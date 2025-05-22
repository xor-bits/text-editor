use std::{
    borrow::Cow,
    fs,
    io::{self, Seek},
    ops::Range,
    path::{Path, PathBuf},
    sync::{Arc, LazyLock},
};

use eyre::{bail, Result};
use ropey::{Rope, RopeSlice};
use tree_sitter::{InputEdit, Language, Parser, Point, Tree};

use crate::tramp::{ConnectionPool, Part};

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
}

impl Lang {
    pub fn ts_language(self) -> Language {
        match self {
            Self::Rust => tree_sitter_rust::LANGUAGE.into(),
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
            _ => Err(UnknownLanguage),
        }
    }
}

//

pub struct Buffer {
    pub contents: Rope,
    pub name: Cow<'static, str>,
    /// where the buffer is stored, if it even is
    pub inner: BufferInner,
    pub modified: bool,
    pub syntax: Option<Syntax>,
}

pub enum BufferInner {
    File { inner: fs::File, readonly: bool },
    NewFile { inner: PathBuf },
    Remote { remote: Arc<[Part]> },
    Scratch { show_welcome: bool },
}

impl Buffer {
    pub fn new() -> Self {
        Self {
            contents: Rope::new(),
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
        let name = name.to_string().into();

        let mut conn = CONN_POOL.connect(parts)?;
        let file = conn.read_file(path)?;
        let contents = Rope::from_reader(file)?;
        let remote = conn.remote();
        CONN_POOL.recycle(conn);

        let syntax = Syntax::try_from_ext(path, contents.slice(..));

        Ok(Self {
            contents,
            name,
            inner: BufferInner::Remote { remote },
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
            Ok(file) => {
                let contents = Rope::from_reader(&file)?;
                let syntax = Syntax::try_from_ext(path, contents.slice(..));

                return Ok(Self {
                    contents,
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
            Err(e) if e.kind() == io::ErrorKind::PermissionDenied => {}
            Err(other) => bail!(other),
            Ok(file) => {
                let contents = Rope::from_reader(&file)?;
                let syntax = Syntax::try_from_ext(path, contents.slice(..));

                return Ok(Self {
                    contents,
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

        let contents = Rope::new();
        let syntax = Syntax::try_from_ext(path, contents.slice(..));

        // finally open it as a new file, without creating the file yet
        Ok(Self {
            contents,
            name,
            inner: BufferInner::NewFile { inner: path.into() },
            modified: false,
            syntax,
        })
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

                inner.seek(io::SeekFrom::Start(0))?;
                inner.set_len(self.contents.len_bytes() as u64)?;

                self.contents.write_to(inner)?;
            }
            BufferInner::NewFile { ref inner } => {
                let new_file = fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(inner)?;

                self.contents.write_to(new_file)?;
            }
            BufferInner::Remote { ref remote } => {
                let (_, filename) = self.name.rsplit_once(':').unwrap();

                let mut conn = CONN_POOL.connect_to(remote.clone())?;
                let writer = conn.write_file(filename)?;
                self.contents.write_to(writer)?;
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
        if cursor.start >= self.contents.len_chars() {
            tracing::warn!("replace_text_at cursor.start out of bounds");
            return;
        }
        if cursor.end >= self.contents.len_chars() {
            cursor.end = self.contents.len_chars() - 1;
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
