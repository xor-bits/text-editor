use std::cmp::Ordering;

use crossterm::{
    cursor::SetCursorStyle,
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute, terminal,
};
use ratatui::{
    layout::{Constraint, Layout, Position, Rect},
    style::Style,
    text::Line,
    widgets::{Block, Paragraph, Widget},
    DefaultTerminal, Frame,
};

use crate::{
    buffer::Buffer,
    mode::{Mode, ModeSubset},
};

use self::keymap::{Code, Keymap};

//

pub mod actions;
pub mod keymap;
pub mod theme;

//

struct BufferWidget<'a> {
    buffer: &'a Buffer,
    line: usize,
}

impl Widget for BufferWidget<'_> {
    fn render(self, area: Rect, buf: &mut ratatui::prelude::Buffer) {
        for (y, line) in self
            .buffer
            .contents
            .get_lines_at(self.line)
            .into_iter()
            .flatten()
            .take(area.height as usize)
            .enumerate()
        {
            for (x, ch) in line
                .chars()
                .take(area.width as usize)
                .filter(|ch| *ch != '\n' && *ch != '\r')
                .enumerate()
            {
                buf[(area.x + x as u16, area.y + y as u16)].set_char(ch);
            }
        }
    }
}

pub struct Cursor<'a> {
    line: usize,
    row: usize,
    col: usize,
    mode: &'a Mode,
}

impl Widget for Cursor<'_> {
    fn render(self, area: Rect, buf: &mut ratatui::prelude::Buffer) {
        if self.row - self.line > area.height as usize || self.col > area.width as usize {
            return;
        }
        let row = area.top() + (self.row - self.line) as u16;
        let col = area.left() + self.col as u16;

        // highlight the current row
        for x in area.left()..area.right() {
            buf[(x, row)].set_bg(theme::CURSOR_LINE);
        }
        // highlight the current column
        for y in area.top()..area.bottom() {
            buf[(col, y)].set_bg(theme::CURSOR_LINE);
        }
        // highlight the 80th char column
        if 80 <= area.right() {
            for y in area.top()..area.bottom() {
                buf[(80, y)].set_bg(theme::CURSOR_LINE);
            }
        }
        // highlight the cursor itself
        if !self.mode.is_insert() {
            buf[(col, row)]
                .set_bg(theme::CURSOR)
                .set_fg(theme::BACKGROUND);
        }
    }
}

pub struct LineNumbers {
    line: usize,
    row: usize,
    lines: usize,
}

impl Widget for LineNumbers {
    fn render(self, area: Rect, buf: &mut ratatui::prelude::Buffer) {
        use std::fmt::Write;

        let mut text = String::with_capacity(area.width as usize * area.height as usize); // TODO: cache this memory

        for y in 0..area.height {
            match (y as usize + self.line + 1).cmp(&self.lines) {
                Ordering::Equal => {
                    _ = writeln!(&mut text, "{:>width$}", "~", width = area.width as usize);
                }
                Ordering::Less => {
                    let num = self.line + y as usize;
                    let num = if num == self.row {
                        num + 1
                    } else {
                        // relative numbering
                        num.abs_diff(self.row)
                    };

                    _ = writeln!(&mut text, "{:>width$}", num, width = area.width as usize);
                }
                _ => {}
            }
        }

        Paragraph::new(text).render(area, buf);

        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                if y as usize != self.row - self.line {
                    buf[(x, y)].set_fg(theme::INACTIVE);
                }
            }
        }
    }
}

pub struct Editor {
    pub should_close: bool,

    pub buffer: Buffer,

    pub size: (u16, u16),
    pub cursor: usize,
    pub view_line: usize,
    pub command: String,

    pub mode: Mode,

    pub keymap: Keymap,
}

impl Editor {
    pub fn new(buffer: Buffer) -> Self {
        Self {
            should_close: false,
            buffer,
            size: terminal::size().unwrap(),
            cursor: 0,
            view_line: 0,
            command: String::new(),

            mode: Mode::Normal,

            keymap: Keymap::load(),
        }
    }

    pub fn run(&mut self, mut terminal: DefaultTerminal) {
        loop {
            let area = terminal
                .draw(|frame| {
                    self.render(frame);
                })
                .unwrap()
                .area;

            if self.mode.is_command() {
                execute!(terminal.backend_mut(), SetCursorStyle::SteadyBlock).unwrap();
                terminal.show_cursor().unwrap();
                terminal
                    .set_cursor_position(Position {
                        x: self.command.len() as u16,
                        y: area.height.saturating_sub(1),
                    })
                    .unwrap();
            } else if self.mode.is_insert() {
                let row = self.buffer.contents.char_to_line(self.cursor);
                let col = self.cursor - self.buffer.contents.line_to_char(row);
                execute!(terminal.backend_mut(), SetCursorStyle::SteadyBar).unwrap();
                terminal.show_cursor().unwrap();
                terminal
                    .set_cursor_position(Position {
                        x: col as u16 + self.buffer.contents.len_lines().ilog10() as u16 + 5,
                        y: row.saturating_sub(self.view_line) as u16,
                    })
                    .unwrap();
            } else {
                terminal.hide_cursor().unwrap();
            }

            self.event(event::read().unwrap());

            if self.should_close {
                break;
            }
        }
    }

    pub fn render(&mut self, frame: &mut Frame) {
        frame.render_widget(
            Block::new().style(Style::new().bg(theme::BACKGROUND)),
            frame.area(),
        );

        let [buffer_area, info_area, cmd_area] = Layout::vertical([
            Constraint::Min(0),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .areas(frame.area());

        let lines = self.buffer.contents.len_lines();

        let [_, line_numbers_area, _, buffer_area] = Layout::horizontal([
            Constraint::Length(2),
            Constraint::Length(lines.ilog10() as u16 + 1),
            Constraint::Length(2),
            Constraint::Min(0),
        ])
        .areas(buffer_area);

        let row = self.buffer.contents.char_to_line(self.cursor);
        let col = self.cursor - self.buffer.contents.line_to_char(row);

        // keep the cursor within view
        if row < self.view_line {
            self.view_line = row;
        }
        if row + 3 > self.view_line + buffer_area.height as usize {
            self.view_line = row + 3 - buffer_area.height as usize;
        }

        // render line numbers
        let line_numbers = LineNumbers {
            line: self.view_line,
            row,
            lines,
        };
        frame.render_widget(line_numbers, line_numbers_area);

        // render the text buffer
        let buffer = BufferWidget {
            buffer: &self.buffer,
            line: self.view_line,
        };
        frame.render_widget(buffer, buffer_area);

        // render the cursor and cursor crosshair
        let cursor = Cursor {
            line: self.view_line,
            row,
            col,
            mode: &self.mode,
        };
        frame.render_widget(cursor, buffer_area);

        let cursor_pos = format!("{row}:{col}");
        let left = Line::from_iter([
            " ",
            self.mode.as_str(),
            "   ",
            self.buffer.lossy_name.as_ref(),
        ]);
        let right = Line::from_iter([cursor_pos.as_str(), " "]);
        let info = Block::new()
            .title(left.left_aligned())
            .title(right.right_aligned())
            .style(Style::new().bg(theme::BUFFER_LINE));
        frame.render_widget(info, info_area);

        // let bg = Block::new().style(Style::new().bg(Color::Black));
        // frame.render_widget(bg, cmd_area);
        // frame.render_widget(self.command.as_str(), cmd_area);

        let cmd = Block::new()
            // .style(Style::new().bg(Color::Black))
            .title(self.command.as_str());
        frame.render_widget(cmd, cmd_area);

        // frame.render_widget(Block::new().title("cmd area"), cmd_area);
    }

    pub fn event(&mut self, event: Event) {
        match event {
            Event::Key(KeyEvent {
                code: KeyCode::Esc,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                ..
            }) => {
                if let Mode::Insert { append: true } = self.mode {
                    self.cursor -= 1;
                }
                self.mode = Mode::Normal;
                self.command.clear();
            }
            /* Event::Key(KeyEvent {
                code: KeyCode::Char('c'),
                modifiers: KeyModifiers::CONTROL,
                kind: KeyEventKind::Press,
                ..
            }) => {
                self.should_close = true;
                return;
            } */
            Event::Resize(w, h) => {
                self.size = (w, h);
            }
            Event::Key(KeyEvent {
                code,
                modifiers,
                kind: KeyEventKind::Press,
                ..
            }) => {
                let (layer, prev) = match self.mode {
                    Mode::Normal => (self.keymap.normal(), ModeSubset::Normal),
                    Mode::Insert { append } => {
                        (self.keymap.insert(), ModeSubset::Insert { append })
                    }
                    Mode::Command => (self.keymap.command(), ModeSubset::Command),
                    Mode::Action { ref layer, prev } => (layer.clone(), prev),
                };

                if layer.run(
                    Code {
                        keycode: code,
                        modifiers,
                    },
                    self,
                ) {
                    return;
                }

                self.mode = prev.mode();
            }
            _ => {}
        }
    }

    /// count matching characters starting and including `from`
    fn count_matching(&self, from: usize, mut pred: impl FnMut(char) -> bool) -> usize {
        self.buffer
            .contents
            .get_chars_at(from)
            .into_iter()
            .flatten()
            .take_while(|ch| pred(*ch))
            .count()
    }

    /// find the next matching `pred` starting and including `from`
    fn find(&self, from: usize, pred: impl FnMut(char) -> bool) -> Option<usize> {
        self.buffer
            .contents
            .get_chars_at(from)
            .into_iter()
            .flatten()
            .position(pred)
    }

    /// reverse find the next matching `pred` starting and including `from`
    fn rfind(&self, from: usize, pred: impl FnMut(char) -> bool) -> Option<usize> {
        self.buffer
            .contents
            .get_chars_at(from + 1)
            .map(|s| s.reversed())
            .into_iter()
            .flatten()
            .position(pred)
    }

    /// find the next word boundary starting and including `from`
    fn find_boundary(&self, from: usize) -> usize {
        self.buffer
            .contents
            .chars_at(from)
            .scan(None, |first, ch| {
                let ty = ch.is_alphanumeric();
                (*first.get_or_insert(ty) == ty).then_some(())
            })
            .skip(1)
            .count()
    }

    /// reverse find the next word boundary starting and including `from`
    fn rfind_boundary(&self, from: usize) -> usize {
        self.buffer
            .contents
            .chars_at(from + 1)
            .reversed()
            .scan(None, |first, ch| {
                let ty = ch.is_alphanumeric();
                (*first.get_or_insert(ty) == ty).then_some(())
            })
            .skip(1)
            .count()
    }

    fn jump_cursor(&mut self, delta_x: isize, delta_y: isize) {
        if self.buffer.contents.len_chars() == 0 {
            // cant move if the buffer has nothing
            self.cursor = 0;
            return;
        }

        if delta_x != 0 {
            // delta X can wrap
            self.cursor = self
                .cursor
                .saturating_add_signed(delta_x)
                .min(self.buffer.contents.len_chars() - 1);
        }

        // delta Y from now on
        if delta_y == 0 || self.buffer.contents.len_lines() == 0 {
            return;
        }

        // figure out what X position the cursor is moved to
        let cursor_line = self.buffer.contents.char_to_line(self.cursor);
        let line_start = self.buffer.contents.line_to_char(cursor_line);
        let cursor_x = self.cursor - line_start;

        let target_line = cursor_line
            .saturating_add_signed(delta_y)
            .min(self.buffer.contents.len_lines() - 1);
        let target_line_len = self.buffer.contents.line(target_line).len_chars();

        // place the cursor on the same X position or on the last char on the line
        let target_line_start = self.buffer.contents.line_to_char(target_line);
        self.cursor = target_line_start + target_line_len.min(cursor_x);
    }

    fn jump_line_beg(&mut self) {
        self.cursor = self
            .buffer
            .contents
            .line_to_char(self.buffer.contents.char_to_line(self.cursor));
    }

    fn jump_line_end(&mut self) {
        let line = self.buffer.contents.char_to_line(self.cursor);
        let line_len = self
            .buffer
            .contents
            .line(line)
            .len_chars()
            .saturating_sub(1);
        self.cursor = self
            .buffer
            .contents
            .len_chars()
            .min(self.buffer.contents.line_to_char(line) + line_len);
    }

    fn jump_beg(&mut self) {
        self.cursor = 0;
    }

    fn jump_end(&mut self) {
        self.cursor = self.buffer.contents.len_chars().saturating_sub(1);
    }
}
