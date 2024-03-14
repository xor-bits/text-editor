use std::{fs, io::stdout};

use clap::Parser;
use crossterm::{
    cursor::{MoveDown, MoveLeft, MoveRight, MoveUp, SetCursorStyle},
    event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute, queue,
    style::{Color, SetBackgroundColor},
    terminal::{
        self, disable_raw_mode, enable_raw_mode, Clear, ClearType, EnterAlternateScreen,
        LeaveAlternateScreen,
    },
};

//

#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    file: String,
}

//

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Insert { append: bool },
    Command,
}

impl Mode {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Mode::Normal => "NOR",
            Mode::Insert { .. } => "INS",
            Mode::Command => "CMD",
        }
    }

    pub const fn cursor_style(&self) -> SetCursorStyle {
        match self {
            Mode::Normal => SetCursorStyle::SteadyBlock,
            Mode::Insert { .. } => SetCursorStyle::SteadyBar,
            Mode::Command => SetCursorStyle::SteadyBar,
        }
    }

    /// Returns `true` if the mode is [`Normal`].
    ///
    /// [`Normal`]: Mode::Normal
    #[must_use]
    pub fn is_normal(&self) -> bool {
        matches!(self, Self::Normal)
    }

    /// Returns `true` if the mode is [`Insert`].
    ///
    /// [`Insert`]: Mode::Insert
    #[must_use]
    pub fn is_insert(&self) -> bool {
        matches!(self, Self::Insert { .. })
    }

    /// Returns `true` if the mode is [`Command`].
    ///
    /// [`Command`]: Mode::Command
    #[must_use]
    pub fn is_command(&self) -> bool {
        matches!(self, Self::Command)
    }
}

//

fn main() {
    let args = Args::parse();

    let a = AlternativeScreenGuard::enter();

    // println!("{args:?}");
    let mut buffer = fs::read_to_string(args.file.as_str())
        .unwrap()
        .split('\n')
        .map(ToString::to_string)
        .collect::<Vec<String>>();

    let mut size = terminal::size().unwrap();
    let mut cursor = (0u16, 0u16);
    let mut view_line = 0usize;
    let mut mode = Mode::Normal;
    let mut command = String::new();

    redraw(&buffer[view_line..], size, mode, Some(&args.file));
    execute!(
        stdout(),
        MoveLeft(u16::MAX),
        MoveUp(u16::MAX),
        mode.cursor_style()
    )
    .unwrap();

    loop {
        let ev = crossterm::event::read().unwrap();
        let mut cursor_delta = (0i16, 0i16);
        let old_mode = mode;

        // println!("{ev:?}");

        match ev {
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
                    cursor_delta.0 -= 1;
                }
                mode = Mode::Normal
            }
            Event::Resize(w, h) => {
                size = (w, h);
                redraw(&buffer[view_line..], size, mode, Some(&args.file));
            }
            Event::Key(KeyEvent {
                code: KeyCode::Tab,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                ..
            }) => {
                redraw(&buffer[view_line..], size, mode, Some(&args.file));
            }
            Event::Key(KeyEvent {
                code: KeyCode::Up,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                ..
            }) if !mode.is_command() => cursor_delta.1 -= 1,
            Event::Key(KeyEvent {
                code: KeyCode::Down,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                ..
            }) if !mode.is_command() => cursor_delta.1 += 1,
            Event::Key(KeyEvent {
                code: KeyCode::Left,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                ..
            }) if !mode.is_command() => cursor_delta.0 -= 1,
            Event::Key(KeyEvent {
                code: KeyCode::Right,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                ..
            }) if !mode.is_command() => cursor_delta.0 += 1,
            Event::Key(KeyEvent {
                code: KeyCode::PageUp,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                ..
            }) if !mode.is_command() => cursor_delta.1 -= size.1 as i16 - 1,
            Event::Key(KeyEvent {
                code: KeyCode::PageDown,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                ..
            }) if !mode.is_command() => cursor_delta.1 += size.1 as i16 - 3,
            Event::Key(KeyEvent {
                code: KeyCode::Home,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                ..
            }) if !mode.is_command() => cursor_delta.0 -= i16::MAX,
            Event::Key(KeyEvent {
                code: KeyCode::End,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                ..
            }) if !mode.is_command() => cursor_delta.0 += i16::MAX,
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
                cursor_delta.0 -= i16::MAX;
            }
            Event::Key(KeyEvent {
                code: KeyCode::Char('a'),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                ..
            }) if mode.is_normal() => {
                mode = Mode::Insert { append: true };
                cursor_delta.0 += 1;
            }
            Event::Key(KeyEvent {
                code: KeyCode::Char('A'),
                modifiers: KeyModifiers::SHIFT,
                kind: KeyEventKind::Press,
                ..
            }) if mode.is_normal() => {
                mode = Mode::Insert { append: true };
                cursor_delta.0 += i16::MAX;
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
                if cursor.0 != 0 {
                    if let Some(line) = buffer.get_mut(cursor.1 as usize + view_line) {
                        cursor.0 -= 1;
                        line.remove(cursor.0 as usize);
                        redraw_line(&buffer[view_line..], cursor.1, size);
                    }
                }
            }
            Event::Key(KeyEvent {
                code: KeyCode::Char(ch),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                ..
            }) if mode.is_insert() => {
                if let Some(line) = buffer.get_mut(cursor.1 as usize + view_line) {
                    line.insert(cursor.0 as usize, ch);
                    cursor.0 += 1;
                    redraw_line(&buffer[view_line..], cursor.1, size);
                }
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
                modifiers: KeyModifiers::NONE,
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
                        fs::write(&args.file, buffer.join("\n")).unwrap();
                        return;
                    }
                    ":w" => {
                        fs::write(&args.file, buffer.join("\n")).unwrap();
                    }
                    _ => {
                        command.clear();
                        command.push_str("invalid command");
                    }
                }
            }
            _ => {}
        }

        if mode != old_mode {
            redraw_footer(size, mode, Some(&args.file));
        }

        let old_cursor = cursor;
        cursor.0 = cursor.0.saturating_add_signed(cursor_delta.0);
        cursor.1 = cursor.1.saturating_add_signed(cursor_delta.1);

        cursor.0 = buffer
            .get(cursor.1 as usize + view_line)
            .map_or(0, |line| line.len().min(u16::MAX as usize) as u16)
            .min(cursor.0)
            .min(size.0 - 1) as _;
        cursor.1 = cursor.1.min(size.1 - 3);

        cursor_delta.0 += old_cursor.0 as i16 - cursor.0 as i16;
        cursor_delta.1 += old_cursor.1 as i16 - cursor.1 as i16;
        // println!("{cursor_delta:?}");

        let view_old = view_line;
        view_line = view_line
            .saturating_add_signed(cursor_delta.1 as isize)
            .min(buffer.len().saturating_sub(size.1 as usize - 2));
        if view_old != view_line {
            redraw(&buffer[view_line..], size, mode, Some(&args.file));
        }

        queue!(
            stdout(),
            MoveDown(u16::MAX),
            MoveLeft(u16::MAX),
            Clear(ClearType::CurrentLine)
        )
        .unwrap();
        print!("{}", command);
        execute!(stdout(), mode.cursor_style()).unwrap();
        if !mode.is_command() {
            // println!("{cursor:?}");
            queue!(stdout(), MoveUp(u16::MAX), MoveLeft(u16::MAX)).unwrap();
            if cursor.1 != 0 {
                queue!(stdout(), MoveDown(cursor.1)).unwrap();
            }
            if cursor.0 != 0 {
                queue!(stdout(), MoveRight(cursor.0)).unwrap();
            }
            execute!(stdout(), mode.cursor_style()).unwrap();
        }
    }

    _ = a;
}

fn redraw(buffer: &[String], size: (u16, u16), mode: Mode, file: Option<&str>) {
    queue!(
        stdout(),
        MoveLeft(u16::MAX),
        MoveUp(u16::MAX),
        Clear(ClearType::All)
    )
    .unwrap();
    for line in buffer.iter().take(size.1 as usize - 2) {
        let line = line.get(..size.0 as usize).unwrap_or(line);
        print!("{line}\r\n");
    }

    redraw_footer_at(mode, file);
}

fn redraw_footer(size: (u16, u16), mode: Mode, file: Option<&str>) {
    queue!(stdout(), MoveLeft(u16::MAX), MoveUp(u16::MAX),).unwrap();
    if size.1 - 2 != 0 {
        queue!(
            stdout(),
            MoveDown(size.1 - 2),
            Clear(ClearType::CurrentLine)
        )
        .unwrap();
    }
    redraw_footer_at(mode, file);
}

fn redraw_footer_at(mode: Mode, file: Option<&str>) {
    queue!(stdout(), SetBackgroundColor(Color::Black)).unwrap();
    println!(" {} {}", mode.as_str(), file.unwrap_or("[scratch]"));
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

struct AlternativeScreenGuard(());

impl AlternativeScreenGuard {
    pub fn enter() -> Self {
        enable_raw_mode().unwrap();
        execute!(stdout(), EnterAlternateScreen).unwrap();
        Self(())
    }
}

impl Drop for AlternativeScreenGuard {
    fn drop(&mut self) {
        execute!(stdout(), LeaveAlternateScreen).unwrap();
        disable_raw_mode().unwrap();
    }
}
