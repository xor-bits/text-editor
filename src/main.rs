use self::{args::Args, buffer::Buffer, editor::Editor};
use clap::Parser;

//

pub mod args;
pub mod buffer;
pub mod editor;
pub mod mode;

//

fn main() {
    let args: Args = Args::parse();

    let (_guard, terminal) = AlternativeScreenGuard::enter();

    let buffer = args
        .file
        .map(|filename| Buffer::open(filename.as_str().as_ref()))
        .unwrap_or_else(|| Ok(Buffer::new()))
        .expect("FIXME: failed to open a file");

    let mut editor = Editor::new(buffer);
    editor.run(terminal);
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
