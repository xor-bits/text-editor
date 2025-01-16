use std::io::stdout;

use self::{args::Args, buffer::Buffer, editor::Editor, mode::Mode};
use clap::Parser;
use crossterm::{
    cursor::{MoveDown, MoveLeft, MoveUp},
    execute, queue,
    style::{Color, SetBackgroundColor},
    terminal::{Clear, ClearType},
};

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

    /* // let mut old_mode = None;

    loop {
        let row = buffer.contents.char_to_line(cursor);
        let col = cursor - buffer.contents.line_to_char(row);

        // keep the cursor within view
        let view_old = view_line;
        if row < view_line {
            view_line = row;
        }
        if row + 3 > view_line + size.1 as usize {
            view_line = row + 3 - size.1 as usize;
        }
        if view_old != view_line {
            redraw(&buffer, size, mode, view_line);
        }

        // if Some(mode) != old_mode {
        redraw_footer(size, mode, &buffer.lossy_name, row, col);
        // }
        // old_mode = Some(mode);

        let screen_row = (row - view_line).min(size.1 as usize) as u16;
        let screen_col = col.min(size.0 as usize) as u16;

        queue!(
            stdout(),
            MoveDown(u16::MAX),
            MoveLeft(u16::MAX),
            Clear(ClearType::CurrentLine)
        )
        .unwrap();
        print!("{command}");
        execute!(stdout(), mode.cursor_style()).unwrap();
        if !mode.is_command() {
            // println!("{cursor:?}");
            queue!(stdout(), MoveUp(u16::MAX), MoveLeft(u16::MAX)).unwrap();
            if screen_row != 0 {
                queue!(stdout(), MoveDown(screen_row)).unwrap();
            }
            if screen_col != 0 {
                queue!(stdout(), MoveRight(screen_col)).unwrap();
            }
            execute!(stdout(), mode.cursor_style()).unwrap();
        }

        match crossterm::event::read().unwrap() {
            Event::Key(KeyEvent {
                code: KeyCode::Char('c'),
                modifiers: KeyModifiers::CONTROL,
                kind: KeyEventKind::Press,
                ..
            }) => break,
            Event::Key(KeyEvent {
                code: KeyCode::Esc,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                ..
            }) => {
                if let Mode::Insert { append: true } = mode {
                    cursor -= 1;
                }
                mode = Mode::Normal;
                command.clear();
            }
            Event::Resize(w, h) => {
                size = (w, h);
                redraw(&buffer, size, mode, view_line);
            }
            Event::Key(KeyEvent {
                code: KeyCode::Tab,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                ..
            }) => {
                redraw(&buffer, size, mode, view_line);
            }
            Event::Key(KeyEvent {
                code: KeyCode::Left,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                ..
            }) if !mode.is_command() => jump_cursor(&buffer, &mut cursor, -1, 0),
            Event::Key(KeyEvent {
                code: KeyCode::Down,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                ..
            }) if !mode.is_command() => jump_cursor(&buffer, &mut cursor, 0, 1),
            Event::Key(KeyEvent {
                code: KeyCode::Up,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                ..
            }) if !mode.is_command() => jump_cursor(&buffer, &mut cursor, 0, -1),
            Event::Key(KeyEvent {
                code: KeyCode::Right,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                ..
            }) if !mode.is_command() => jump_cursor(&buffer, &mut cursor, 1, 0),
            Event::Key(KeyEvent {
                code: KeyCode::Char('h'),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                ..
            }) if mode.is_normal() => jump_cursor(&buffer, &mut cursor, -1, 0),
            Event::Key(KeyEvent {
                code: KeyCode::Char('j'),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                ..
            }) if mode.is_normal() => jump_cursor(&buffer, &mut cursor, 0, 1),
            Event::Key(KeyEvent {
                code: KeyCode::Char('k'),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                ..
            }) if mode.is_normal() => jump_cursor(&buffer, &mut cursor, 0, -1),
            Event::Key(KeyEvent {
                code: KeyCode::Char('l'),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                ..
            }) if mode.is_normal() => jump_cursor(&buffer, &mut cursor, 1, 0),
            Event::Key(KeyEvent {
                code: KeyCode::PageUp,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                ..
            }) if !mode.is_command() => {
                jump_cursor(&buffer, &mut cursor, 0, -(size.1 as isize - 1))
            }
            Event::Key(KeyEvent {
                code: KeyCode::PageDown,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                ..
            }) if !mode.is_command() => jump_cursor(&buffer, &mut cursor, 0, size.1 as isize - 3),
            Event::Key(KeyEvent {
                code: KeyCode::Home,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                ..
            }) if !mode.is_command() => jump_line_beg(&buffer, &mut cursor),
            Event::Key(KeyEvent {
                code: KeyCode::End,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                ..
            }) if !mode.is_command() => jump_line_end(&buffer, &mut cursor),
            Event::Key(KeyEvent {
                code: KeyCode::Char('i'),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                ..
            }) if mode.is_normal() => mode = Mode::Insert { append: false },
            Event::Key(KeyEvent {
                code: KeyCode::Char('I'),
                modifiers: KeyModifiers::SHIFT,
                kind: KeyEventKind::Press,
                ..
            }) if mode.is_normal() => {
                mode = Mode::Insert { append: false };
                jump_line_beg(&buffer, &mut cursor);
            }
            Event::Key(KeyEvent {
                code: KeyCode::Char('a'),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                ..
            }) if mode.is_normal() => {
                mode = Mode::Insert { append: true };
                jump_cursor(&buffer, &mut cursor, 1, 0);
            }
            Event::Key(KeyEvent {
                code: KeyCode::Char('A'),
                modifiers: KeyModifiers::SHIFT,
                kind: KeyEventKind::Press,
                ..
            }) if mode.is_normal() => {
                mode = Mode::Insert { append: true };
                jump_line_end(&buffer, &mut cursor);
            }
            Event::Key(KeyEvent {
                code: KeyCode::Char(':'),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                ..
            }) if mode.is_normal() => {
                mode = Mode::Command;
                command.clear();
                command.push(':');
            }
            Event::Key(KeyEvent {
                code: KeyCode::Backspace,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                ..
            }) if mode.is_insert() => {
                if cursor == 0 {
                    continue;
                }

                buffer.contents.remove(cursor - 1..cursor);
                jump_cursor(&buffer, &mut cursor, -1, 0);
                redraw(&buffer, size, mode, view_line);
            }
            Event::Key(KeyEvent {
                code: KeyCode::Char(ch),
                kind: KeyEventKind::Press,
                ..
            }) if mode.is_insert() => {
                buffer.contents.insert_char(cursor, ch);
                jump_cursor(&buffer, &mut cursor, 1, 0);
                redraw(&buffer, size, mode, view_line);
            }
            Event::Key(KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                ..
            }) if mode.is_insert() => {
                buffer.contents.insert_char(cursor, '\n');
                jump_cursor(&buffer, &mut cursor, 1, 0);
                redraw(&buffer, size, mode, view_line);
            }
            Event::Key(KeyEvent {
                code: KeyCode::Backspace,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                ..
            }) if mode.is_command() => {
                _ = command.pop();
                if command.is_empty() {
                    mode = Mode::Normal;
                    queue!(
                        stdout(),
                        MoveDown(u16::MAX),
                        MoveLeft(u16::MAX),
                        Clear(ClearType::CurrentLine)
                    )
                    .unwrap();
                }
            }
            Event::Key(KeyEvent {
                code: KeyCode::Char(ch),
                kind: KeyEventKind::Press,
                ..
            }) if mode.is_command() => command.push(ch),
            Event::Key(KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                ..
            }) if mode.is_command() => {
                queue!(
                    stdout(),
                    MoveDown(u16::MAX),
                    MoveLeft(u16::MAX),
                    Clear(ClearType::CurrentLine)
                )
                .unwrap();
                mode = Mode::Normal;
                match command.as_str() {
                    ":q" | ":q!" => return,
                    ":wq" | ":x" => {
                        buffer.write().unwrap();
                        // fs::write(&args.file, buffer.join("\n")).unwrap();
                        return;
                    }
                    ":w" => {
                        // fs::write(&args.file, buffer.join("\n")).unwrap();
                    }
                    _ => {
                        command.push_str("invalid command");
                    }
                }
                command.clear();
            }
            _ => {}
        }
    }

    _ = a; */
}

fn redraw(buffer: &Buffer, size: (u16, u16), mode: Mode, line: usize) {
    queue!(
        stdout(),
        MoveLeft(u16::MAX),
        MoveUp(u16::MAX),
        Clear(ClearType::All)
    )
    .unwrap();
    for line in buffer
        .contents
        .get_lines_at(line)
        .into_iter()
        .flatten()
        .take(size.1 as usize - 2)
    {
        let line = line.get_slice(..size.0 as usize).unwrap_or(line);
        print!("{line}\r");
    }

    // redraw_footer_at(mode, &buffer.lossy_name);
}

fn redraw_footer(size: (u16, u16), mode: Mode, file: &str, row: usize, col: usize) {
    queue!(stdout(), MoveLeft(u16::MAX), MoveUp(u16::MAX)).unwrap();
    if size.1 - 2 != 0 {
        queue!(
            stdout(),
            MoveDown(size.1 - 2),
            Clear(ClearType::CurrentLine)
        )
        .unwrap();
    }
    redraw_footer_at(mode, file, row, col);
}

fn redraw_footer_at(mode: Mode, file: &str, row: usize, col: usize) {
    queue!(stdout(), SetBackgroundColor(Color::Black)).unwrap();
    println!(" {} {}", mode.as_str(), file);
    execute!(stdout(), SetBackgroundColor(Color::Reset)).unwrap();
}

fn redraw_line(buffer: &[String], line: u16, size: (u16, u16)) {
    queue!(stdout(), MoveLeft(u16::MAX), MoveUp(u16::MAX),).unwrap();
    if line != 0 {
        queue!(stdout(), MoveDown(line)).unwrap();
    }
    queue!(stdout(), Clear(ClearType::CurrentLine)).unwrap();
    if let Some(line) = buffer.iter().take(size.1 as usize - 2).nth(line as usize) {
        let line = line.get(..size.0 as usize).unwrap_or(line);
        print!("{line}");
    }
}

struct AlternativeScreenGuard;

impl AlternativeScreenGuard {
    pub fn enter() -> (Self, ratatui::DefaultTerminal) {
        // std::panic::set_hook(Box::new(move |i| {
        //     execute!(stdout(), LeaveAlternateScreen).unwrap();
        //     disable_raw_mode().unwrap();
        //     eprintln!("{i}");
        // }));
        (Self, ratatui::init())
    }
}

impl Drop for AlternativeScreenGuard {
    fn drop(&mut self) {
        ratatui::restore();
    }
}
