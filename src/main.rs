use std::{fs, io::stdout, thread, time::Duration};

use clap::Parser;
use crossterm::{
    cursor::{self, MoveDown, MoveLeft, MoveRight, MoveUp},
    event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute, queue,
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

fn main() {
    let args = Args::parse();

    let a = AlternativeScreenGuard::enter();

    // println!("{args:?}");
    let buffer = fs::read_to_string(args.file)
        .unwrap()
        .split('\n')
        .map(ToString::to_string)
        .collect::<Vec<String>>();

    let mut size = terminal::size().unwrap();
    let mut cursor = (0u16, 0u16);
    let mut view_line = 0usize;

    redraw(&buffer[view_line..], size);
    execute!(stdout(), MoveLeft(u16::MAX), MoveUp(u16::MAX)).unwrap();

    loop {
        let ev = crossterm::event::read().unwrap();
        let mut cursor_delta = (0i16, 0i16);

        match ev {
            Event::Key(KeyEvent {
                code: KeyCode::Char('c'),
                modifiers: KeyModifiers::CONTROL,
                kind: KeyEventKind::Press,
                ..
            })
            | Event::Key(KeyEvent {
                code: KeyCode::Esc,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                ..
            }) => break,
            Event::Resize(w, h) => {
                size = (w, h);
                redraw(&buffer[view_line..], size);
            }
            Event::Key(KeyEvent {
                code: KeyCode::Tab,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                ..
            }) => {
                redraw(&buffer[view_line..], size);
            }
            Event::Key(KeyEvent {
                code: KeyCode::Up,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                ..
            }) => cursor_delta.1 -= 1,
            Event::Key(KeyEvent {
                code: KeyCode::Down,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                ..
            }) => cursor_delta.1 += 1,
            Event::Key(KeyEvent {
                code: KeyCode::Left,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                ..
            }) => cursor_delta.0 -= 1,
            Event::Key(KeyEvent {
                code: KeyCode::Right,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                ..
            }) => cursor_delta.0 += 1,
            Event::Key(KeyEvent {
                code: KeyCode::PageUp,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                ..
            }) => cursor_delta.1 -= size.1 as i16 - 1,
            Event::Key(KeyEvent {
                code: KeyCode::PageDown,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                ..
            }) => cursor_delta.1 += size.1 as i16 - 1,
            Event::Key(KeyEvent {
                code: KeyCode::Home,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                ..
            }) => cursor_delta.0 -= i16::MAX,
            Event::Key(KeyEvent {
                code: KeyCode::End,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                ..
            }) => cursor_delta.0 += i16::MAX,
            _ => {}
        }

        let old_cursor = cursor;
        cursor.0 = cursor.0.saturating_add_signed(cursor_delta.0);
        cursor.1 = cursor.1.saturating_add_signed(cursor_delta.1);

        cursor.0 = buffer
            .get(cursor.1 as usize + view_line)
            .map_or(0, |line| line.len().min(u16::MAX as usize) as u16)
            .min(cursor.0)
            .min(size.0 - 1) as _;
        cursor.1 = cursor.1.min(size.1 - 1);

        cursor_delta.0 += old_cursor.0 as i16 - cursor.0 as i16;
        cursor_delta.1 += old_cursor.1 as i16 - cursor.1 as i16;
        // println!("{cursor_delta:?}");

        let view_old = view_line;
        view_line = view_line
            .saturating_add_signed(cursor_delta.1 as isize)
            .min(buffer.len().saturating_sub(size.1 as usize));
        if view_old != view_line {
            redraw(&buffer[view_line..], size);
        }

        // if cursor.1 == 0 {
        //     if view_line != 0 {
        //         view_line -= 1;
        //         redraw(&buffer[view_line..], size);
        //     }
        // } else {
        //     cursor.1 -= 1;
        // }

        // println!("{ev:?}");
        // match ev {
        //     Event::FocusGained => todo!(),
        //     Event::FocusLost => todo!(),
        //     Event::Key(_) => todo!(),
        //     Event::Mouse(_) => todo!(),
        //     Event::Paste(_) => todo!(),
        //     Event::Resize(_, _) => todo!(),
        // }

        // println!("{cursor:?}");
        queue!(stdout(), MoveUp(u16::MAX), MoveLeft(u16::MAX)).unwrap();
        if cursor.1 != 0 {
            queue!(stdout(), MoveDown(cursor.1)).unwrap();
        }
        if cursor.0 != 0 {
            queue!(stdout(), MoveRight(cursor.0)).unwrap();
        }
        execute!(stdout()).unwrap();
    }

    _ = a;
}

fn redraw(buffer: &[String], size: (u16, u16)) {
    queue!(
        stdout(),
        MoveLeft(u16::MAX),
        MoveUp(u16::MAX),
        Clear(ClearType::All)
    )
    .unwrap();
    let mut first = true;
    for line in buffer.iter().take(size.1 as usize) {
        let line = line.get(..size.0 as usize).unwrap_or(line);
        if !first {
            print!("\n\r");
        }
        first = false;
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
