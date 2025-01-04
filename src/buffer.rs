use std::{
    borrow::Cow,
    fs,
    io::{self, Seek},
    path::{Path, PathBuf},
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

    pub fn open(path: &Path) -> io::Result<Self> {
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
