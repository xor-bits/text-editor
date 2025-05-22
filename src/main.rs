use std::{
    env,
    fs::{self, File, OpenOptions},
    path::PathBuf,
    sync::OnceLock,
};

use self::{args::Args, buffer::Buffer, editor::Editor};
use clap::Parser;
use eyre::Result;
use tree_sitter::LogType;

//

pub mod args;
pub mod buffer;
pub mod editor;
pub mod mode;
pub mod tramp;

//

fn main() -> Result<()> {
    color_eyre::install()?;
    logger_init()?;
    let args: Args = Args::parse();

    let buffer = args
        .file
        .map(|filename| Buffer::open(filename.as_str()))
        .unwrap_or_else(|| Ok(Buffer::new_welcome()))
        .expect("FIXME: failed to open a file");

    let (_guard, terminal) = AlternativeScreenGuard::enter();

    let mut editor = Editor::new(buffer);
    editor.run(terminal);

    Ok(())
}

fn ts_logger() -> Option<Box<dyn FnMut(LogType, &str)>> {
    Some(Box::new(|ty, msg| {
        _ = (ty, msg);
        // tracing::debug!("tree-sitter ({ty:?}): {msg}");
    }))
}

fn tmpdir() -> PathBuf {
    if let Some(xdg_runtime_dir) = env::var_os("XDG_RUNTIME_DIR") {
        PathBuf::from(xdg_runtime_dir).join("text-editor")
    } else {
        PathBuf::from("/tmp/text-editor")
    }
}

/* fn tmpfile(name_hint: &str) -> Result<File> {
    loop {
        let mut filename = String::with_capacity(name_hint.len() + 9);
        filename.push_str(name_hint);
        filename.push('-');
        for _ in 0..8 {
            filename.push(random_alphanum() as char);
        }

        let mut path = crate::tmpdir();
        path.push(filename);

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        match OpenOptions::new().create_new(true).open(path) {
            Ok(file) => return Ok(file),
            Err(e) if e.kind() == ErrorKind::AlreadyExists => {}
            Err(other) => Err(other)?,
        }
    }
}

fn random_alphanum() -> u8 {
    ALPHANUM_TABLE[rand::random_range(0u8..64u8) as usize]
}

const ALPHANUM_TABLE: [u8; 64] = {
    let mut alpha_table: [u8; 64] = [0u8; 64];
    let mut i: usize = 0;

    let mut ch = b'0';
    while ch <= b'9' {
        ch += 1;
        i += 1;
        alpha_table[i] = ch;
    }

    ch = b'a';
    while ch <= b'z' {
        ch += 1;
        i += 1;
        alpha_table[i] = ch;
    }

    ch = b'A';
    while ch <= b'Z' {
        ch += 1;
        i += 1;
        alpha_table[i] = ch;
    }

    alpha_table
}; */

fn logger_init() -> Result<()> {
    let mut log_file_path = tmpdir();
    log_file_path.push("log");
    log_file_path.push("latest.log");

    if let Some(parent) = log_file_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_file_path)?;

    _ = LOG_FILE.set(log_file.try_clone()?);

    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(log_file)
        .init();

    tracing::debug!("logger init");

    Ok(())
}

static LOG_FILE: OnceLock<File> = OnceLock::new();

struct AlternativeScreenGuard;

impl AlternativeScreenGuard {
    pub fn enter() -> (Self, ratatui::DefaultTerminal) {
        (Self, ratatui::init())
    }
}

impl Drop for AlternativeScreenGuard {
    fn drop(&mut self) {
        ratatui::restore();
    }
}
