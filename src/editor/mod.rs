use std::cmp::Ordering;

use crossterm::{
    cursor::SetCursorStyle,
    event::{self, Event, KeyEvent, KeyEventKind},
    execute, terminal,
};
use ratatui::{
    layout::{Constraint, Layout, Position, Rect},
    style::{Style, Stylize},
    text::Line,
    widgets::{Block, Paragraph, Widget},
    DefaultTerminal, Frame,
};

use crate::{
    buffer::Buffer,
    mode::{Mode, ModeSubset},
};

use self::{
    keymap::{ActionEntry, Code, Keymap},
    view::BufferView,
};

//

pub mod actions;
pub mod keymap;
pub mod theme;
pub mod view;

//

pub struct Editor {
    pub should_close: bool,
    pub size: (u16, u16),
    pub real_cursor: (usize, usize),

    pub buffers: Vec<Buffer>,
    pub view: BufferView,

    pub command: String,
    pub command_suggestions: Vec<ActionEntry>,
    pub command_suggestion_index: Option<usize>,

    pub mode: Mode,

    pub keymap: Keymap,
}

impl Editor {
    pub fn new(buffer: Buffer) -> Self {
        Self {
            should_close: false,
            size: terminal::size().unwrap(),
            real_cursor: (0, 0),

            buffers: vec![buffer],
            view: BufferView::new(0),

            command: String::new(),
            command_suggestions: Vec::new(),
            command_suggestion_index: None,

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
                execute!(terminal.backend_mut(), SetCursorStyle::SteadyBar).unwrap();
                terminal.show_cursor().unwrap();
                terminal
                    .set_cursor_position(Position {
                        x: self.real_cursor.0 as u16,
                        y: self.real_cursor.1 as u16,
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

        let [buffer_area, suggestion_area] = Layout::vertical([
            Constraint::Min(1),
            Constraint::Length((self.command_suggestions.len() as u16).min(10)),
        ])
        .areas(buffer_area);

        let ((col, row), buf_name, _real_cursor) =
            self.view
                .render(&self.buffers, &self.mode, buffer_area, frame);
        self.real_cursor = _real_cursor;

        let cursor_pos = format!("{row}:{col}");
        let left = Line::from_iter([" ", self.mode.as_str(), "   ", buf_name]);
        let right = Line::from_iter([cursor_pos.as_str(), " "]);
        let info = Block::new()
            .title(left.left_aligned())
            .title(right.right_aligned())
            .style(Style::new().bg(theme::BUFFER_LINE));
        frame.render_widget(info, info_area);
        // let bg = Block::new().style(Style::new().bg(Color::Black));
        // frame.render_widget(bg, cmd_area);
        // frame.render_widget(self.command.as_str(), cmd_area);

        let suggestion_list_chunk_start = self
            .command_suggestion_index
            .unwrap_or(0)
            .checked_div(suggestion_area.height as usize)
            .unwrap_or(0)
            .checked_mul(suggestion_area.height as usize)
            .unwrap_or(0);

        let suggestion_bg = Block::new().style(Style::new().bg(theme::BACKGROUND_LIGHT));
        frame.render_widget(suggestion_bg, suggestion_area);
        for (i, act) in self
            .command_suggestions
            .iter()
            .enumerate()
            .skip(suggestion_list_chunk_start)
            .take(suggestion_area.height as usize)
        {
            let (fg, bg) = if Some(i) == self.command_suggestion_index {
                (theme::BACKGROUND_LIGHT, theme::CURSOR)
            } else {
                (theme::CURSOR, theme::BACKGROUND_LIGHT)
            };

            let area = Rect {
                x: suggestion_area.x,
                y: suggestion_area.y + (i - suggestion_list_chunk_start) as u16,
                width: suggestion_area.width,
                height: 1,
            };

            let suggestion = Block::new()
                .title(
                    Line::from_iter([act.act.description()])
                        .right_aligned()
                        .fg(theme::ACCENT),
                )
                .title(
                    Line::from_iter([act.act.name()])
                        .left_aligned()
                        .fg(fg)
                        .bg(bg),
                );
            frame.render_widget(suggestion, area);
        }

        let cmd = Block::new()
            // .style(Style::new().bg(Color::Black))
            .title(self.command.as_str());
        frame.render_widget(cmd, cmd_area);

        // frame.render_widget(Block::new().title("cmd area"), cmd_area);
    }

    pub fn event(&mut self, event: Event) {
        match event {
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

                if layer.run(Code::from_event(code, modifiers), self) {
                    return;
                }

                self.mode = prev.mode();
            }
            _ => {}
        }
    }

    fn current(&self) -> (&Buffer, &BufferView) {
        (&self.buffers[self.view.buffer_index], &self.view)
    }

    fn current_mut(&mut self) -> (&mut Buffer, &mut BufferView) {
        (&mut self.buffers[self.view.buffer_index], &mut self.view)
    }

    /// count matching characters starting and including `from`
    fn count_matching(&self, from: usize, mut pred: impl FnMut(char) -> bool) -> usize {
        let (buffer, _) = self.current();

        buffer
            .contents
            .get_chars_at(from)
            .into_iter()
            .flatten()
            .take_while(|ch| pred(*ch))
            .count()
    }

    /// find the next matching `pred` starting and including `from`
    fn find(&self, from: usize, pred: impl FnMut(char) -> bool) -> Option<usize> {
        let (buffer, _) = self.current();

        buffer
            .contents
            .get_chars_at(from)
            .into_iter()
            .flatten()
            .position(pred)
    }

    /// reverse find the next matching `pred` starting and including `from`
    fn rfind(&self, from: usize, pred: impl FnMut(char) -> bool) -> Option<usize> {
        let (buffer, _) = self.current();

        buffer
            .contents
            .get_chars_at(from + 1)
            .map(|s| s.reversed())
            .into_iter()
            .flatten()
            .position(pred)
    }

    /// find the next word boundary starting and including `from`
    fn find_boundary(&self, from: usize) -> usize {
        let (buffer, _) = self.current();

        buffer
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
        let (buffer, _) = self.current();

        buffer
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
        let (buffer, view) = self.current_mut();

        if buffer.contents.len_chars() == 0 {
            // cant move if the buffer has nothing
            view.cursor = 0;
            return;
        }

        if delta_x != 0 {
            // delta X can wrap
            view.cursor = view
                .cursor
                .saturating_add_signed(delta_x)
                .min(buffer.contents.len_chars());
        }

        // delta Y from now on
        if delta_y == 0 || buffer.contents.len_lines() == 0 {
            return;
        }

        // figure out what X position the cursor is moved to
        let cursor_line = buffer.contents.char_to_line(view.cursor);
        let line_start = buffer.contents.line_to_char(cursor_line);
        let cursor_x = view.cursor - line_start;

        let target_line = cursor_line
            .saturating_add_signed(delta_y)
            .min(buffer.contents.len_lines() - 1);
        let target_line_len = buffer.contents.line(target_line).len_chars();

        // place the cursor on the same X position or on the last char on the line
        let target_line_start = buffer.contents.line_to_char(target_line);
        view.cursor = target_line_start + target_line_len.min(cursor_x);
    }

    fn jump_line_beg(&mut self) {
        let (buffer, view) = self.current_mut();
        view.cursor = buffer
            .contents
            .line_to_char(buffer.contents.char_to_line(view.cursor));
    }

    fn jump_line_end(&mut self) {
        let (buffer, view) = self.current_mut();
        let line = buffer.contents.char_to_line(view.cursor);
        let line_len = buffer.contents.line(line).len_chars();
        view.cursor = buffer
            .contents
            .len_chars()
            .min((buffer.contents.line_to_char(line) + line_len).saturating_sub(2));
    }

    fn jump_beg(&mut self) {
        let (_, view) = self.current_mut();
        view.cursor = 0;
    }

    fn jump_end(&mut self) {
        let (buffer, view) = self.current_mut();
        view.cursor = buffer.contents.len_chars().saturating_sub(1);
    }
}
