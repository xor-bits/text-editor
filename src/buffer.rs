use std::{
    borrow::Cow,
    fs,
    io::{self, BufRead, BufReader, Read, Seek, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
};

use ropey::Rope;

//

pub struct Buffer {
    pub contents: Rope,
    pub lossy_name: Cow<'static, str>,
    /// where the buffer is stored, if it even is
    pub inner: BufferInner,
}

pub enum BufferInner {
    File { inner: fs::File, readonly: bool },
    NewFile { inner: PathBuf },
    // Remote { inner: PathBuf, ctx: Child },
    Scratch,
}

impl Buffer {
    pub fn new() -> Self {
        Self {
            contents: Rope::new(),
            lossy_name: Cow::Borrowed("[scratch]"),
            inner: BufferInner::Scratch,
        }
    }

    pub fn open(path: &str) -> io::Result<Self> {
        if let Some((parts, file)) = path.rsplit_once(':') {
            Self::open_remote(parts, file.as_ref())
        } else {
            Self::open_local(path.as_ref())
        }
    }

    pub fn open_remote(parts: &str, path: &Path) -> io::Result<Self> {
        let mut cmd = Command::new("sh")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let mut stdin = cmd.stdin.take().unwrap();
        let mut stdout = BufReader::new(cmd.stdout.take().unwrap());
        let mut stderr = BufReader::new(cmd.stderr.take().unwrap());

        for hop in parts.split('|') {
            let mut part = hop.split(':');
            let hop_type = part.next().unwrap_or(hop);

            match hop_type {
                "ssh" => {
                    let target = part.next().ok_or_else(|| {
                        io::Error::new(io::ErrorKind::InvalidInput, "ssh hop missing destination")
                    })?;

                    // FIXME: sanitation
                    writeln!(stdin, "ssh \'{target}\' \'sh\'")?;

                    let Some("askpw") = part.next() else {
                        continue;
                    };
                }
                "sudo" => {
                    // FIXME: sanitation
                    writeln!(stdin, "sudo -S -p '<??>' sh")?;

                    let Some("askpw") = part.next() else {
                        continue;
                    };

                    // let mut buf = [0u8; 4];
                    // stderr.read_exact(&mut buf)?;
                    // if &buf != b"<??>" {
                    //     continue;
                    // }

                    // write!(stdin, "{}");
                }
                other => {
                    return Err(io::Error::new(
                        io::ErrorKind::Unsupported,
                        format!("unsupported hop type: {other}").as_str(),
                    ))
                }
            }
        }

        cmd.wait().unwrap();

        todo!()
    }

    pub fn open_local(path: &Path) -> io::Result<Self> {
        let lossy_name = path.to_string_lossy().to_string().into();

        // first try opening in RW mode
        match fs::OpenOptions::new()
            .write(true)
            .read(true)
            .create(false)
            .open(path)
        {
            Err(e) if e.kind() == io::ErrorKind::PermissionDenied => {}
            Err(other) => return Err(other),
            Ok(file) => {
                return Ok(Self {
                    contents: Rope::from_reader(&file)?,
                    lossy_name,
                    inner: BufferInner::File {
                        inner: file,
                        readonly: false,
                    },
                })
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
            Err(other) => return Err(other),
            Ok(file) => {
                return Ok(Self {
                    contents: Rope::from_reader(&file)?,
                    lossy_name,
                    inner: BufferInner::File {
                        inner: file,
                        readonly: true,
                    },
                })
            }
        };

        // finally open it as a new file, without creating the file yet
        Ok(Self {
            contents: Rope::new(),
            lossy_name,
            inner: BufferInner::NewFile { inner: path.into() },
        })
    }

    pub fn write(&mut self) -> io::Result<()> {
        match self.inner {
            BufferInner::File {
                ref mut inner,
                readonly,
            } => {
                if readonly {
                    return Err(io::Error::new(io::ErrorKind::PermissionDenied, "readonly"));
                }

                inner.seek(io::SeekFrom::Start(0))?;

                self.contents.write_to(inner)?;
            }
            BufferInner::NewFile { ref inner } => {
                let new_file = fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(inner)?;

                self.contents.write_to(new_file)?;
            }
            BufferInner::Scratch => {}
        };

        Ok(())
    }
}

impl Default for Buffer {
    fn default() -> Self {
        Self::new()
    }
}
