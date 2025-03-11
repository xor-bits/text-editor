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
