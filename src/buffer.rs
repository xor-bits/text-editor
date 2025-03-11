use std::{
    borrow::Cow,
    fs,
    io::{self, Seek},
    path::PathBuf,
    sync::{Arc, LazyLock},
};

use eyre::{bail, Result};
use ropey::Rope;

use crate::tramp::{ConnectionPool, Part};

//

pub struct Buffer {
    pub contents: Rope,
    pub name: Cow<'static, str>,
    /// where the buffer is stored, if it even is
    pub inner: BufferInner,
}

pub enum BufferInner {
    File { inner: fs::File, readonly: bool },
    NewFile { inner: PathBuf },
    Remote { remote: Arc<[Part]> },
    Scratch,
}

impl Buffer {
    pub fn new() -> Self {
        Self {
            contents: Rope::new(),
            name: Cow::Borrowed("[scratch]"),
            inner: BufferInner::Scratch,
        }
    }

    pub fn open(path: &str) -> Result<Self> {
        if let Some((parts, file)) = path.rsplit_once(':') {
            Ok(Self::open_remote(parts, file)?)
        } else {
            Ok(Self::open_local(path)?)
        }
    }

    pub fn open_remote(parts: &str, path: &str) -> Result<Self> {
        let name = path.to_string().into();

        let mut conn = CONN_POOL.connect(parts)?;
        let file = conn.read_file(path)?;
        let contents = Rope::from_reader(file)?;
        let remote = conn.remote();
        CONN_POOL.recycle(conn);

        Ok(Self {
            contents,
            name,
            inner: BufferInner::Remote { remote },
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
                return Ok(Self {
                    contents: Rope::from_reader(&file)?,
                    name,
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
            Err(other) => bail!(other),
            Ok(file) => {
                return Ok(Self {
                    contents: Rope::from_reader(&file)?,
                    name,
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
            name,
            inner: BufferInner::NewFile { inner: path.into() },
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
                let mut conn = CONN_POOL.connect_to(remote.clone())?;
                let writer = conn.write_file(&self.name)?;
                self.contents.write_to(writer)?;
                conn.finish_write_file(&self.name)?;
                CONN_POOL.recycle(conn);
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

//

static CONN_POOL: LazyLock<ConnectionPool> = LazyLock::new(ConnectionPool::new);
