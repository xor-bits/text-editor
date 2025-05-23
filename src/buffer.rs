use std::{
    borrow::Cow,
    fs,
    io::{self, BufWriter, Read, Seek, Write},
    ops::Range,
    path::{Path, PathBuf},
    sync::{Arc, LazyLock},
};

use eyre::{bail, Result};
use ropey::{Rope, RopeSlice};
use tree_sitter::{InputEdit, Language, Parser, Point, Tree};

use crate::tramp::{Connection, ConnectionPool, Part};

//

pub struct Syntax {
    pub parser: Parser,
    pub tree: Tree,
    pub lang: Lang,
}

impl Syntax {
    pub fn try_from_ext(path: &str, rope: RopeSlice) -> Option<Syntax> {
        Path::extension(path.as_ref())
            .and_then(|s| s.to_str())
            .and_then(|s| Lang::try_from(s).ok())
            .map(|lang| {
                let mut parser = Parser::new();
                parser.set_logger(crate::ts_logger());
                parser.set_language(&lang.ts_language()).unwrap();

                let tree = Self::parse(&mut parser, rope, None);

                Syntax { parser, tree, lang }
            })
    }

    pub fn update(&mut self, rope: RopeSlice) {
        self.tree = Self::parse(&mut self.parser, rope, Some(&self.tree));
    }

    fn parse(parser: &mut Parser, rope: RopeSlice, old_tree: Option<&Tree>) -> Tree {
        parser
            .parse_with_options(
                &mut |byte: usize, _position: Point| -> &[u8] {
                    let Ok((chunk, chunk_byte_idx, _, _)) = rope.try_chunk_at_byte(byte) else {
                        return &[];
                    };

                    let offs = byte - chunk_byte_idx;
                    tracing::info!(
                        "byte={byte} chunk_byte_idx={chunk_byte_idx} offs={offs} chunklen={}",
                        chunk.len()
                    );
                    tracing::info!("_position={_position}");
                    &chunk.as_bytes()[offs..]
                },
                old_tree,
                None,
            )
            .unwrap()
    }
}

//

#[derive(Debug, Clone, Copy)]
pub enum Lang {
    Rust,
    Zig,
}

impl Lang {
    pub fn ts_language(self) -> Language {
        match self {
            Self::Rust => tree_sitter_rust::LANGUAGE.into(),
            Self::Zig => tree_sitter_zig::LANGUAGE.into(),
        }
    }
}

#[derive(Debug)]
pub struct UnknownLanguage;

impl TryFrom<&str> for Lang {
    type Error = UnknownLanguage;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "rs" => Ok(Self::Rust),
            "zig" => Ok(Self::Zig),
            _ => Err(UnknownLanguage),
        }
    }
}

//

pub struct Buffer {
    pub contents: Rope,
    pub name: Cow<'static, str>,
    pub ty: ContentTransform,
    /// where the buffer is stored, if it even is
    pub inner: BufferInner,
    pub modified: bool,
    pub syntax: Option<Syntax>,
}

#[derive(Debug, Clone, Copy)]
pub enum ContentTransform {
    Utf8,
    Hex,
    Nbt,
}

pub enum BufferInner {
    File { inner: fs::File, readonly: bool },
    NewFile { inner: PathBuf },
    Remote { remote: Arc<[Part]>, readonly: bool },
    Scratch { show_welcome: bool },
}

impl Buffer {
    pub fn new() -> Self {
        Self {
            contents: Rope::new(),
            ty: ContentTransform::Utf8,
            name: Cow::Borrowed("[scratch]"),
            inner: BufferInner::Scratch {
                show_welcome: false,
            },
            modified: false,
            syntax: None,
        }
    }

    pub fn new_welcome() -> Self {
        Self {
            contents: Rope::new(),
            ty: ContentTransform::Utf8,
            name: Cow::Borrowed("[scratch]"),
            inner: BufferInner::Scratch { show_welcome: true },
            modified: false,
            syntax: None,
        }
    }

    pub fn open(path: &str) -> Result<Self> {
        if let Some((parts, file)) = path.rsplit_once(':') {
            Ok(Self::open_remote(parts, file, path)?)
        } else {
            Ok(Self::open_local(path)?)
        }
    }

    pub fn open_remote(parts: &str, path: &str, name: &str) -> Result<Self> {
        let mut conn = CONN_POOL.connect(parts)?;
        let res = Self::open_remote_with(&mut conn, path, name);
        CONN_POOL.recycle(conn);
        res
    }

    pub fn open_remote_with(conn: &mut Connection, path: &str, name: &str) -> Result<Self> {
        let name = name.to_string().into();

        let mut file = conn.read_file(path)?;

        let mut contents = Vec::new();
        file.read_to_end(&mut contents)?;

        tracing::debug!("remote returned {} bytes", contents.len());

        let remote = conn.remote();

        let (contents, syntax, ty) = Self::read_from(&contents, path);

        Ok(Self {
            contents,
            ty,
            name,
            inner: BufferInner::Remote {
                remote,
                readonly: false,
            },
            modified: false,
            syntax,
        })
    }

    pub fn open_local(path: &str) -> Result<Self> {
        let name = path.to_string().into();

        // first try opening in RW mode
        match fs::OpenOptions::new()
            .write(true)
            .read(true)
            .create(false)
            .open(path)
        {
            Err(e) if e.kind() == io::ErrorKind::PermissionDenied => {}
            Err(other) => bail!(other),
            Ok(mut file) => {
                let mut contents = Vec::new();
                file.read_to_end(&mut contents)?;

                let (contents, syntax, ty) = Self::read_from(&contents, path);

                return Ok(Self {
                    contents,
                    ty,
                    name,
                    inner: BufferInner::File {
                        inner: file,
                        readonly: false,
                    },
                    modified: false,
                    syntax,
                });
            }
        };

        // then try opening it in readonly mode
        match fs::OpenOptions::new()
            .write(false)
            .read(true)
            .create(false)
            .open(path)
        {
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(other) => bail!(other),
            Ok(mut file) => {
                let mut contents = Vec::new();
                file.read_to_end(&mut contents)?;

                let (contents, syntax, ty) = Self::read_from(&contents, path);

                return Ok(Self {
                    contents,
                    ty,
                    name,
                    inner: BufferInner::File {
                        inner: file,
                        readonly: true,
                    },
                    modified: false,
                    syntax,
                });
            }
        };

        let (contents, syntax, ty) = Self::read_from(&[], path);

        // finally open it as a new file, without creating the file yet
        Ok(Self {
            contents,
            ty,
            name,
            inner: BufferInner::NewFile { inner: path.into() },
            modified: false,
            syntax,
        })
    }

    fn read_from(contents: &[u8], path: &str) -> (Rope, Option<Syntax>, ContentTransform) {
        if let Some(result) = Self::try_read_utf8(contents, path) {
            return result;
        }

        if let Some(result) = Self::try_read_nbt(contents, path) {
            return result;
        }

        Self::read_hex(contents, path)
    }

    fn try_read_utf8(
        contents: &[u8],
        path: &str,
    ) -> Option<(Rope, Option<Syntax>, ContentTransform)> {
        let Ok(s) = std::str::from_utf8(contents) else {
            return None;
        };

        let contents = Rope::from_str(s);
        let syntax = Syntax::try_from_ext(path, contents.slice(..));

        Some((contents, syntax, ContentTransform::Utf8))
    }

    fn try_read_nbt(
        contents: &[u8],
        _path: &str,
    ) -> Option<(Rope, Option<Syntax>, ContentTransform)> {
        let decoder = flate2::bufread::GzDecoder::new(contents);
        let header = decoder.header()?;
        tracing::debug!("header = {header:?}");

        let Ok(val) = fastnbt::from_reader::<_, fastnbt::Value>(decoder) else {
            return None;
        };
        let contents = fastsnbt::to_string_pretty(&val).expect("failed to recode NBT to json");
        let contents = Rope::from_str(&contents);
        let syntax = Syntax::try_from_ext(".json", contents.slice(..));

        Some((contents, syntax, ContentTransform::Nbt))
    }

    fn read_hex(contents: &[u8], _path: &str) -> (Rope, Option<Syntax>, ContentTransform) {
        struct HexReader<'a> {
            contents: &'a [u8],
            col: usize,
            state: Option<u8>,
        }

        enum Control {
            First,
            Second,
            Space,
            Next,
        }

        #[rustfmt::skip]
        const FORMAT: &[Control] = &[
            Control::First, Control::Second, Control::Space,
            Control::First, Control::Second, Control::Space,
            Control::First, Control::Second, Control::Space,
            Control::First, Control::Second, Control::Space,
            Control::First, Control::Second, Control::Space,
            Control::First, Control::Second, Control::Space,
            Control::First, Control::Second, Control::Space,
            Control::First, Control::Second, Control::Space,

            Control::Space,

            Control::Space, Control::First, Control::Second,
            Control::Space, Control::First, Control::Second,
            Control::Space, Control::First, Control::Second,
            Control::Space, Control::First, Control::Second,
            Control::Space, Control::First, Control::Second,
            Control::Space, Control::First, Control::Second,
            Control::Space, Control::First, Control::Second,
            Control::Space, Control::First, Control::Second,

            Control::Next,
        ];

        impl HexReader<'_> {
            fn get(&mut self) -> Option<u8> {
                if let Some(cur) = self.state {
                    return Some(cur);
                }
                let (byte, left) = self.contents.split_first()?;
                self.contents = left;
                self.state = Some(*byte);
                self.state
            }

            fn advance(&mut self) {
                self.state = None;
            }

            fn hex_to_ascii(hex: u8) -> u8 {
                if hex < 10 {
                    b'0' + hex
                } else if hex < 16 {
                    b'a' + hex - 10
                } else {
                    unreachable!()
                }
            }
        }

        impl std::io::Read for HexReader<'_> {
            fn read(&mut self, mut buf: &mut [u8]) -> io::Result<usize> {
                let mut n = 0;
                while let Some((outb, buf_next)) = buf.split_first_mut() {
                    buf = buf_next;

                    let Some(byte) = self.get() else {
                        return Ok(n);
                    };

                    match FORMAT[self.col] {
                        Control::First => {
                            *outb = Self::hex_to_ascii((byte & 0xF0) >> 4);
                            self.col += 1;
                        }
                        Control::Second => {
                            *outb = Self::hex_to_ascii(byte & 0xF);
                            self.col += 1;
                            self.advance();
                        }
                        Control::Space => {
                            *outb = b' ';
                            self.col += 1;
                        }
                        Control::Next => {
                            *outb = b'\n';
                            self.col = 0;
                        }
                    }
                    n += 1;
                }

                Ok(n)
            }
        }

        let hex_reader = HexReader {
            contents,
            col: 0,
            state: None,
        };

        let contents = Rope::from_reader(hex_reader).unwrap();
        let syntax = Syntax::try_from_ext("", contents.slice(..));

        (contents, syntax, ContentTransform::Hex)
    }

    pub fn write(&mut self) -> Result<()> {
        match self.inner {
            BufferInner::File {
                ref mut inner,
                readonly,
            } => {
                if readonly {
                    bail!("readonly");
                }

                Self::write_to_file(&self.contents, self.ty, &mut self.modified, inner)?;
            }
            BufferInner::NewFile { ref inner } => {
                let mut new_file = fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(inner)?;

                Self::write_to_file(&self.contents, self.ty, &mut self.modified, &mut new_file)?;

                self.inner = BufferInner::File {
                    inner: new_file,
                    readonly: false,
                };
            }
            BufferInner::Remote {
                ref remote,
                readonly,
            } => {
                if readonly {
                    bail!("readonly");
                }

                let (_, filename) = self.name.rsplit_once(':').unwrap();

                let mut conn = CONN_POOL.connect_to(remote.clone())?;
                let writer = conn.write_file(filename)?;

                Self::write_to(&self.contents, self.ty, &mut self.modified, writer)?;

                conn.finish_write_file(filename)?;
                CONN_POOL.recycle(conn);
            }
            BufferInner::Scratch {
                ref mut show_welcome,
            } => {
                *show_welcome = false;
                bail!("no file path set");
            }
        };

        self.modified = false;

        Ok(())
    }

    fn write_to_file(
        contents: &Rope,
        ty: ContentTransform,
        modified: &mut bool,
        output: &mut fs::File,
    ) -> Result<()> {
        output.seek(io::SeekFrom::Start(0))?;
        output.set_len(0)?;

        Self::write_to(contents, ty, modified, output)?;

        Ok(())
    }

    fn write_to(
        contents: &Rope,
        ty: ContentTransform,
        modified: &mut bool,
        mut output: impl Write,
    ) -> Result<()> {
        match ty {
            ContentTransform::Utf8 => {
                contents.write_to(BufWriter::new(output))?;
            }
            ContentTransform::Hex => {
                let mut buf = Vec::new();
                let mut state = None;

                for (i, ch) in contents.chars().enumerate() {
                    if ch.is_whitespace() {
                        continue;
                    }

                    let Some(hexdigit) = ch.to_digit(16) else {
                        let row = contents.char_to_line(i);
                        let col = i - contents.line_to_char(row);
                        bail!("invalid token at {}:{}", row + 1, col + 1);
                    };

                    std::debug_assert!(hexdigit <= 15);
                    let hexdigit = hexdigit as u8;

                    if let Some(state) = state.take() {
                        buf.push((state << 4) | hexdigit);
                    } else {
                        state = Some(hexdigit);
                    }
                }
                if let Some(state) = state.take() {
                    buf.push(state << 4);
                }

                output.write_all(&buf)?;
            }
            ContentTransform::Nbt => {
                /* struct RopeReader<'a> {
                    chunks: ropey::iter::Chunks<'a>,
                    left: &'a [u8],
                }

                impl io::Read for RopeReader<'_> {
                    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
                        if self.left.is_empty() {
                            let Some(chunk) = self.chunks.next() else {
                                return Ok(0);
                            };
                            self.left = chunk.as_bytes();
                        }

                        let len = buf.len().min(self.left.len());
                        let (copying, now_left) = self.left.split_at(len);
                        buf[0..len].copy_from_slice(copying);
                        self.left = now_left;
                        Ok(len)
                    }
                }

                let mut reader = RopeReader {
                    chunks: contents.chunks(),
                    left: &[],
                }; */

                let contents = contents.to_string(); // TODO: implement Read for fastsnbt

                let encoder = flate2::GzBuilder::new()
                    .write(BufWriter::new(output), flate2::Compression::best());

                let val: fastnbt::Value = fastsnbt::from_str(&contents)?;
                fastnbt::to_writer(encoder, &val)?;
            }
        }

        *modified = false;

        Ok(())
    }

    /// insert `ch` and push the old text under it forward
    pub fn insert_char_at(&mut self, cursor: usize, ch: char) {
        let mut buf = [0u8; 4];
        self.insert_text_at(cursor, ch.encode_utf8(&mut buf));
    }

    /// insert `ch` and delete the old text under it
    pub fn overwrite_char_at(&mut self, cursor: usize, ch: char) {
        let mut buf = [0u8; 4];
        self.overwrite_text_at(cursor, ch.encode_utf8(&mut buf));
    }

    /// delete range `cursor` and replace it with `ch`
    pub fn replace_char_at(&mut self, cursor: Range<usize>, ch: char) {
        let mut buf = [0u8; 4];
        self.replace_text_at(cursor, ch.encode_utf8(&mut buf));
    }

    /// insert `text` and push the old text under it forward
    pub fn insert_text_at(&mut self, cursor: usize, text: &str) {
        self.replace_text_at(cursor..cursor, text);
    }

    /// insert `text` and delete the old text under it
    pub fn overwrite_text_at(&mut self, cursor: usize, text: &str) {
        self.replace_text_at(cursor..text.len(), text);
    }

    /// delete range `cursor` and replace it with `text`
    pub fn replace_text_at(&mut self, mut cursor: Range<usize>, text: &str) {
        if cursor.start > self.contents.len_chars() {
            cursor.start = self.contents.len_chars();
            tracing::warn!("replace_text_at cursor.start out of bounds");
            return;
        }
        if cursor.end > self.contents.len_chars() {
            cursor.end = self.contents.len_chars();
            tracing::warn!("replace_text_at cursor.end out of bounds");
            return;
        }

        if !cursor.is_empty() {
            self.contents.remove(cursor.clone());
            self.modified = true;
        }
        if !text.is_empty() {
            self.contents.insert(cursor.start, text);
            self.modified = true;
        }

        // update syntax highlighting
        let Some(syntax) = self.syntax.as_mut() else {
            return;
        };

        let start_byte_idx = self.contents.char_to_byte(cursor.start);
        let end_byte_idx = if cursor.is_empty() {
            start_byte_idx
        } else {
            self.contents.char_to_byte(cursor.end)
        };

        let old_start_line_idx = self.contents.byte_to_line(start_byte_idx);
        let old_start_col_idx = start_byte_idx - self.contents.line_to_byte(old_start_line_idx);
        let old_end_line_idx = if cursor.is_empty() {
            old_start_line_idx
        } else {
            self.contents.byte_to_line(end_byte_idx)
        };
        let old_end_col_idx = if cursor.is_empty() {
            old_start_col_idx
        } else {
            end_byte_idx - self.contents.line_to_byte(old_end_line_idx)
        };

        let new_end = start_byte_idx + text.len();
        let new_end_line_idx = self.contents.byte_to_line(new_end);
        let new_end_col_idx = new_end - self.contents.line_to_byte(old_end_line_idx);

        syntax.tree.edit(&InputEdit {
            start_byte: start_byte_idx,
            old_end_byte: end_byte_idx,

            new_end_byte: new_end,
            start_position: Point::new(old_start_line_idx, old_start_col_idx),
            old_end_position: Point::new(old_end_line_idx, old_end_col_idx),
            new_end_position: Point::new(new_end_line_idx, new_end_col_idx),
        });
        syntax.update(self.contents.slice(..));
    }
}

impl Default for Buffer {
    fn default() -> Self {
        Self::new()
    }
}

//

pub static CONN_POOL: LazyLock<ConnectionPool> = LazyLock::new(ConnectionPool::new);
