use std::{
    env,
    fs::{self, OpenOptions},
    path::{Path, PathBuf},
};

use self::{args::Args, buffer::Buffer, editor::Editor};
use clap::Parser;
use eyre::Result;

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
        .unwrap_or_else(|| Ok(Buffer::new()))
        .expect("FIXME: failed to open a file");

    let (_guard, terminal) = AlternativeScreenGuard::enter();

    let mut editor = Editor::new(buffer);
    editor.run(terminal);

    Ok(())
}

fn logger_init() -> Result<()> {
    let mut log_file_path = PathBuf::from("/tmp");
    if let Some(xdg_runtime_dir) = env::var_os("XDG_RUNTIME_DIR") {
        log_file_path = PathBuf::from(xdg_runtime_dir);
    }
    log_file_path = log_file_path.join("text-editor").join("latest.log");

    if let Some(parent) = log_file_path.parent() {
        fs::create_dir_all(parent)?;
    }

    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(
            OpenOptions::new()
                .create(true)
                .append(true)
                .open(log_file_path)?,
        )
        .init();

    tracing::debug!("logger init");

    Ok(())
}

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
