use eyre::{bail, eyre, Result};
use rexpect::{process::signal, session::PtySession};
use std::{
    collections::HashMap,
    fmt,
    io::{self, Cursor, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, RwLock},
    time::Instant,
};

//

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Part {
    Ssh {
        destination: Str,
        port: u16,
        askpw: bool,
    },
    Docker {
        container: Str,
    },
    Sudo {
        askpw: bool,
    },
    Bash {},
}

impl Part {
    pub fn parse(pool: &mut String, s: &str) -> Result<Self> {
        let mut args = s.split(':');
        let proto_id = args.next().unwrap_or(s);

        match proto_id {
            "ssh" => {
                let mut port = 22;
                let mut askpw = false;

                // 1st arg is the destination
                let destination = args
                    .next()
                    .ok_or_else(|| eyre!("missing ssh destination"))?;
                let destination = Str::new(pool, destination);

                // 2nd arg (optional) is either the port or askpw
                if let Some(a2) = args.next() {
                    if a2 == "askpw" {
                        askpw = true;
                    } else if let Ok(_port) = a2.parse::<u16>() {
                        port = _port;
                    }
                }

                // 3rd arg (optional) is askpw
                if Some("askpw") == args.next() {
                    askpw = true;
                }

                Ok(Self::Ssh {
                    destination,
                    port,
                    askpw,
                })
            }
            "sudo" => {
                let mut askpw = false;

                // 1st arg (optional) is askpw
                if Some("askpw") == args.next() {
                    askpw = true;
                }

                Ok(Self::Sudo { askpw })
            }
            "docker" => {
                let container = args
                    .next()
                    .ok_or_else(|| eyre!("missing docker container ID"))?;
                let container = Str::new(pool, container);

                Ok(Self::Docker { container })
            }
            "bash" => Ok(Self::Bash {}),
            _ => bail!("unknown protocol"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Str {
    start: u32,
    len: u32,
}

impl Str {
    pub fn new(pool: &mut String, s: &str) -> Self {
        if let Some(start) = pool.find(s) {
            Self {
                start: start.try_into().unwrap(),
                len: s.len().try_into().unwrap(),
            }
        } else {
            let start = pool.len();
            pool.push_str(s);
            Self {
                start: start.try_into().unwrap(),
                len: s.len().try_into().unwrap(),
            }
        }
    }

    pub fn as_str(self, pool: &str) -> &str {
        &pool[self.start as usize..][..self.len as usize]
    }
}

/// a single threaded connection instance
#[must_use]
pub struct Connection {
    remote: Arc<[Part]>,
    shell: PtySession,
}

impl Connection {
    /// jump over ssh
    pub fn hop_ssh(&mut self, destination: &str, port: u16, askpw: bool) -> Result<()> {
        // FIXME: sanitation
        _ = askpw;
        self.run_cmd_checked(format_args!(
            "ssh -p {port} -t -t '{destination}' env PS1=__sh_prompt TERM=dumb sh"
        ))?;
        Ok(())
    }

    /// elevate privileges
    pub fn hop_sudo(&mut self, askpw: bool) -> Result<()> {
        // FIXME: sanitation
        _ = askpw;
        self.run_cmd_checked(format_args!(
            "sudo -S -p '__sudo_askpw' env PS1=__sh_prompt TERM=dumb sh"
        ))?;
        Ok(())
    }

    pub fn hop_docker(&mut self, container: &str) -> Result<()> {
        // FIXME: sanitation
        self.run_cmd_checked(format_args!(
            "docker exec -it '{container}' env PS1=__sh_prompt TERM=dumb sh"
        ))?;
        self.run_cmd(format_args!("stty -echoctl"))?;
        self.run_cmd(format_args!("stty -echo"))?;
        self.wait()?;
        self.wait()?;
        Ok(())
    }

    /// run a command and test the exit code
    pub fn run_cmd_checked(&mut self, cmd: fmt::Arguments) -> Result<String> {
        let now = Instant::now();
        self.run_cmd(cmd)?;
        self.run_cmd(format_args!("echo $?"))?;

        let result = self.wait()?;
        let exit_code = self.wait()?;
        tracing::debug!("checked command complete");
        let exit_code = exit_code.trim();
        if exit_code != "0" {
            bail!(
                "command failed with: '{}' exit code '{exit_code:?}'",
                result.trim()
            );
        }

        tracing::debug!("cmd took {:?}", now.elapsed());
        Ok(result)
    }

    /// just run one command and log it
    pub fn run_cmd(&mut self, cmd: fmt::Arguments) -> Result<()> {
        tracing::trace!("running '{cmd}'");
        self.shell.writer.write_fmt(cmd)?;
        self.shell.writer.write_all(b"\n")?;
        self.shell.writer.flush()?;
        Ok(())
    }

    /// wait for the prompt indicator
    pub fn wait(&mut self) -> Result<String> {
        tracing::trace!("waiting for __sh_prompt");
        for _ in 0..30_000 {
            match self.shell.exp_string("__sh_prompt") {
                Err(rexpect::error::Error::Timeout { .. }) => {}
                other => return Ok(other?),
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }

        bail!("Expected \"__sh_prompt\" but got \"\" (after waiting for 30000 ms)");
    }

    pub fn canonicalize(&mut self, path: &Path) -> Result<PathBuf> {
        // FIXME: sanitation
        tracing::trace!("running 'realpath {path:?}'");
        self.shell.writer.write_all(b"realpath ")?;
        self.shell
            .writer
            .write_all(path.as_os_str().as_encoded_bytes())?;
        let mut result = self.run_cmd_checked(format_args!(""))?;
        if result.ends_with('\n') {
            result.pop();
        }
        if result.ends_with('\r') {
            result.pop();
        }
        Ok(PathBuf::from(result))
    }

    pub fn list_files(&mut self, path: &Path) -> Result<String> {
        tracing::trace!("running 'ls -al {path:?}'");
        self.shell.writer.write_all(b"ls -al ")?;
        self.shell
            .writer
            .write_all(path.as_os_str().as_encoded_bytes())?;
        self.run_cmd_checked(format_args!(""))
    }

    pub fn read_file(&mut self, filename: &str) -> Result<impl io::Read> {
        let read = self.run_cmd_checked(format_args!("base64 -w 0 {filename}"))?;

        Ok(base64::read::DecoderReader::new(
            Cursor::new(read.into_bytes()),
            &base64::engine::general_purpose::STANDARD,
        ))
    }

    pub fn write_file(&mut self, filename: &str) -> Result<impl io::Write + '_> {
        // FIXME: what if the base64 command failed somehow
        // and now the base64 garbage runs as a command

        // starts reading the base64 data from stdin
        if false {
            self.run_cmd(format_args!("stty -echoctl"))?;
            self.run_cmd(format_args!("base64 -d - > {filename}"))?;
        } else {
            self.shell.writer.write_all(b"echo '")?;
        }

        // Ok(base64::write::EncoderWriter::new(
        //     std::io::stdout().lock(),
        //     &base64::engine::general_purpose::STANDARD,
        // ))

        Ok(base64::write::EncoderWriter::new(
            &mut self.shell.writer,
            &base64::engine::general_purpose::STANDARD,
        ))
    }

    pub fn finish_write_file(&mut self, filename: &str) -> Result<()> {
        if false {
            self.shell.send_control('d')?;
        } else {
            tracing::trace!("running 'echo '...");
            self.run_cmd_checked(format_args!("' | base64 -d - > {filename}"))?;
        }
        Ok(())
    }

    pub fn remote(&self) -> Arc<[Part]> {
        self.remote.clone()
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        // self.shell.process.kill(sig)
        // while self.shell.send_line("exit").is_ok() {
        //     _ = self.shell.read_line();
        //     // self.shell.process.kill(sig)
        // }
        self.shell.process.set_kill_timeout(Some(0));
        _ = self.shell.process.kill(signal::SIGTERM);
    }
}

// pub struct Destination {
//     connections: Vec<Connection>,
//     file_cache: HashMap<Box<str>, File>,
// }

/// a cache for connections
pub struct ConnectionPool {
    string_pool: RwLock<String>,
    connections: Mutex<HashMap<Arc<[Part]>, Vec<Connection>>>,
}

impl ConnectionPool {
    pub fn new() -> Self {
        Self {
            string_pool: RwLock::new(String::new()),
            connections: Mutex::new(HashMap::new()),
        }
    }

    pub fn path_of(&self, remote: &[Part], path: &str) -> String {
        let string_pool = self
            .string_pool
            .read()
            .unwrap_or_else(|err| err.into_inner());

        let mut buf = String::new();
        use std::fmt::Write;

        for part in remote {
            match part {
                Part::Ssh {
                    destination,
                    port,
                    askpw,
                } => {
                    _ = write!(
                        &mut buf,
                        "ssh:{}:{port}{}",
                        destination.as_str(&string_pool),
                        if *askpw { ":askpw" } else { "" }
                    );
                }
                Part::Docker { container } => {
                    _ = write!(&mut buf, "ssh:{}", container.as_str(&string_pool),);
                }
                Part::Sudo { askpw } => {
                    _ = write!(&mut buf, "sudo:{}", if *askpw { ":askpw" } else { "" });
                }
                Part::Bash {} => {
                    _ = write!(&mut buf, "bash");
                }
            }
        }

        buf.push(':');
        buf.push_str(path);

        buf
    }

    pub fn connect(&self, remote: &str) -> Result<Connection> {
        let mut string_pool = self
            .string_pool
            .write()
            .unwrap_or_else(|err| err.into_inner());

        let remote = remote
            .split('|')
            .map(|part| Part::parse(&mut string_pool, part))
            .collect::<Result<Arc<[Part]>>>()?;

        drop(string_pool);

        self.connect_to(remote)
    }

    pub fn connect_to(&self, remote: Arc<[Part]>) -> Result<Connection> {
        let mut connections = self
            .connections
            .lock()
            .unwrap_or_else(|err| err.into_inner());
        if let Some(cache) = connections.get_mut(&remote) {
            if let Some(conn) = cache.pop() {
                return Ok(conn);
            }
        }

        let mut conn = Connection {
            remote,
            shell: rexpect::spawn("env PS1=__sh_prompt TERM=dumb sh", Some(0))?,
        };
        conn.wait()?;

        let string_pool = self
            .string_pool
            .read()
            .unwrap_or_else(|err| err.into_inner());

        for part in conn.remote.clone().iter() {
            tracing::trace!("hop: {part:?}");

            match part {
                Part::Ssh {
                    destination,
                    port,
                    askpw,
                } => {
                    let destination =
                        &string_pool[destination.start as usize..][..destination.len as usize];

                    conn.hop_ssh(destination, *port, *askpw)?;
                }
                Part::Sudo { askpw } => {
                    conn.hop_sudo(*askpw)?;
                }
                Part::Docker { container } => {
                    let container =
                        &string_pool[container.start as usize..][..container.len as usize];
                    conn.hop_docker(container)?;
                }
                Part::Bash {} => {}
            }
        }

        tracing::trace!("connected");

        Ok(conn)
    }

    pub fn recycle(&self, conn: Connection) {
        let mut connections = self
            .connections
            .lock()
            .unwrap_or_else(|err| err.into_inner());

        let cache = connections.entry(conn.remote.clone()).or_default();

        cache.push(conn);
    }
}

impl Default for ConnectionPool {
    fn default() -> Self {
        Self::new()
    }
}
