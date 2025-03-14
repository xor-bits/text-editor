use std::{cmp::Ordering, env};

use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::Style,
    text::Line,
    widgets::{Block, Paragraph, Widget},
    Frame,
};
use ropey::{Rope, RopeSlice};

use crate::{
    buffer::{Buffer, BufferContents, BufferInner},
    mode::Mode,
};

use super::theme;

//

pub struct BufferView {
    pub buffer_index: usize,
    pub cursor: usize,
    pub view_line: usize,
}

impl BufferView {
    pub const fn new(buffer_index: usize) -> Self {
        Self {
            buffer_index,
            cursor: 0,
            view_line: 0,
        }
    }

    pub fn render(
        &mut self,
        buffer: &Buffer,
        mode: &Mode,
        area: Rect,
        frame: &mut ratatui::prelude::Frame,
    ) -> (usize, usize) {
        let [buffer_area, bufferline_area] =
            Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(area);

        let (cursor, real_cursor) = match &buffer.contents {
            BufferContents::Text(rope) => {
                self.render_text_buffer(buffer, rope, buffer_area, frame, mode.is_insert())
            }
            BufferContents::Hex(vec) => {
                self.render_hex_buffer(buffer, &vec, buffer_area, frame, mode.is_insert())
            }
        };

        // render the buffer line
        self.render_bufferline(buffer, bufferline_area, frame, mode.as_str(), cursor);

        real_cursor
    }

    fn render_text_buffer(
        &mut self,
        buffer: &Buffer,
        contents: &Rope,
        area: Rect,
        frame: &mut Frame,
        is_insert_mode: bool,
    ) -> ((usize, usize), (usize, usize)) {
        let lines = contents.len_lines();

        let [_, line_numbers_area, _, buffer_area] = Layout::horizontal([
            Constraint::Length(2),
            Constraint::Length(lines.ilog10() as u16 + 1),
            Constraint::Length(2),
            Constraint::Min(0),
        ])
        .areas(area);

        let row = contents.char_to_line(self.cursor);
        let col = self.cursor - contents.line_to_char(row);

        // keep the cursor within view
        // tracing::debug!(
        //     "view_line={} row={row} lines={lines} buffer_area.height={}",
        //     self.view_line,
        //     buffer_area.height
        // );
        let min = (row + 3).saturating_sub(buffer_area.height as usize);
        let max = row;
        let (min, max) = (min.min(max), min.max(max));
        self.view_line = self.view_line.clamp(min, max);
        // if row < self.view_line {
        //     self.view_line = row;
        // }
        // if row + 3 > self.view_line + buffer_area.height as usize {
        //     self.view_line = row + 3 - buffer_area.height as usize;
        // }
        // tracing::debug!(
        //     "view_line={} row={row} lines={lines} buffer_area.height={}",
        //     self.view_line,
        //     buffer_area.height
        // );

        // render line numbers
        let line_numbers = LineNumbers {
            line: self.view_line,
            row,
            lines,
        };
        frame.render_widget(line_numbers, line_numbers_area);

        // render the text buffer
        let buffer_widget = TextWidget {
            contents: contents.slice(..),
            line: self.view_line,
        };
        frame.render_widget(buffer_widget, buffer_area);

        if matches!(buffer.inner, BufferInner::Scratch { show_welcome: true }) && !buffer.modified {
            self.render_welcome(buffer_area, frame);
        }

        // render the cursor and cursor crosshair
        let cursor = Cursor {
            line: self.view_line,
            row,
            col,
            is_insert_mode,
        };
        frame.render_widget(cursor, buffer_area);

        let real_cursor_row = row - self.view_line + buffer_area.y as usize;
        let real_cursor_col = col + buffer_area.x as usize;

        ((row, col), (real_cursor_row, real_cursor_col))
    }

    fn render_hex_buffer(
        &mut self,
        buffer: &Buffer,
        contents: &[u8],
        area: Rect,
        frame: &mut Frame,
        is_insert_mode: bool,
    ) -> ((usize, usize), (usize, usize)) {
        let lines = contents.len().div_ceil(16);

        let [_, line_numbers_area, _, buffer_area] = Layout::horizontal([
            Constraint::Length(2),
            Constraint::Length((lines.ilog2() >> 2) as u16 + 1), // ilog2 >> 2 is ilog16
            Constraint::Length(2),
            Constraint::Min(0),
        ])
        .areas(area);

        let row = self.cursor / 16;
        let col = self.cursor % 16;

        let real_cursor_row = row - self.view_line + buffer_area.y as usize;
        let real_cursor_col = col + buffer_area.x as usize;

        _ = (buffer, frame, is_insert_mode, line_numbers_area);

        ((row, col), (real_cursor_row, real_cursor_col))
    }

    fn render_welcome(&mut self, area: Rect, frame: &mut Frame) {
        let [_, area, _] = Layout::vertical([
            Constraint::Percentage(50),
            Constraint::Length(10),
            Constraint::Percentage(50),
        ])
        .areas(area);

        let widget = Paragraph::new(vec![
            Line::from_iter(["text-editor v", env!("CARGO_PKG_VERSION")]),
            Line::from_iter([""]),
            Line::from_iter(["type  :q<Enter>   to exit               "]),
            Line::from_iter([""]),
            Line::from_iter(["press <Alt + />   for keybinds          "]),
            Line::from_iter([""]),
            Line::from_iter(["type  :           for a list of commands"]),
            Line::from_iter(["press <Tab>       to cycle that list    "]),
            Line::from_iter(["type  :open file  to open file          "]),
            Line::from_iter([""]),
            Line::from_iter(["have a nice day"]),
        ])
        .centered()
        .style(Style::new().bg(theme::BACKGROUND).fg(theme::CURSOR));

        frame.render_widget(widget, area);
    }

    fn render_bufferline(
        &mut self,
        buffer: &Buffer,
        area: Rect,
        frame: &mut Frame,
        mode: &str,
        (row, col): (usize, usize),
    ) {
        let cursor_pos = format!("{row}:{col}");
        let left = if buffer.modified {
            Line::from_iter([" ", mode, "   ", buffer.name.as_ref(), " [+]"])
        } else {
            Line::from_iter([" ", mode, "   ", buffer.name.as_ref()])
        };
        let right = Line::from_iter([cursor_pos.as_str(), " "]);
        let info = Block::new()
            .title(left.left_aligned())
            .title(right.right_aligned())
            .style(Style::new().bg(theme::BUFFER_LINE));
        frame.render_widget(info, area);
    }

    /// count matching characters starting and including `from`
    pub fn count_matching(
        &self,
        buffer: &Buffer,
        from: usize,
        mut pred: impl FnMut(char) -> bool,
    ) -> usize {
        match &buffer.contents {
            BufferContents::Text(rope) => rope
                .get_chars_at(from)
                .into_iter()
                .flatten()
                .take_while(|ch| pred(*ch))
                .count(),
            BufferContents::Hex(..) => todo!(),
        }
    }

    /// find the next matching `pred` starting and including `from`
    pub fn find(
        &self,
        buffer: &Buffer,
        from: usize,
        pred: impl FnMut(char) -> bool,
    ) -> Option<usize> {
        match &buffer.contents {
            BufferContents::Text(rope) => {
                rope.get_chars_at(from).into_iter().flatten().position(pred)
            }
            BufferContents::Hex(..) => todo!(),
        }
    }

    /// reverse find the next matching `pred` starting and including `from`
    pub fn rfind(
        &self,
        buffer: &Buffer,
        from: usize,
        pred: impl FnMut(char) -> bool,
    ) -> Option<usize> {
        match &buffer.contents {
            BufferContents::Text(rope) => rope
                .get_chars_at(from + 1)
                .map(|s| s.reversed())
                .into_iter()
                .flatten()
                .position(pred),
            BufferContents::Hex(..) => todo!(),
        }
    }

    /// find the next word boundary starting and including `from`
    pub fn find_boundary(&self, buffer: &Buffer, from: usize) -> usize {
        match &buffer.contents {
            BufferContents::Text(rope) => rope
                .chars_at(from)
                .scan(None, |first, ch| {
                    let ty = ch.is_alphanumeric();
                    (*first.get_or_insert(ty) == ty).then_some(())
                })
                .skip(1)
                .count(),
            BufferContents::Hex(vec) => {
                // literal   machine word boundries, so multiples of size_of::<usize>()
                vec.len()
                    .min(from.div_ceil(size_of::<usize>()) * size_of::<usize>())
                    - from
            }
        }
    }

    /// reverse find the next word boundary starting and including `from`
    pub fn rfind_boundary(&self, buffer: &Buffer, from: usize) -> usize {
        match &buffer.contents {
            BufferContents::Text(rope) => rope
                .chars_at(from + 1)
                .reversed()
                .scan(None, |first, ch| {
                    let ty = ch.is_alphanumeric();
                    (*first.get_or_insert(ty) == ty).then_some(())
                })
                .skip(1)
                .count(),
            BufferContents::Hex(..) => {
                // literal machine word boundries, so multiples of size_of::<usize>()
                from - from / size_of::<usize>() * size_of::<usize>()
            }
        }
    }

    pub fn jump_cursor(&mut self, buffer: &Buffer, delta_x: isize, delta_y: isize) {
        if buffer.contents.is_empty() {
            // cant move if the buffer has nothing
            self.cursor = 0;
            return;
        }

        self.jump_cursor_x(buffer, delta_x, true);
        self.jump_cursor_y(buffer, delta_y);
    }

    pub fn jump_cursor_x(&mut self, buffer: &Buffer, delta_x: isize, wraps: bool) {
        if delta_x == 0 {
            return;
        }

        match &buffer.contents {
            BufferContents::Text(rope) => {
                let limit = if wraps {
                    rope.len_chars()
                } else {
                    let cursor_line = rope.char_to_line(self.cursor);
                    let line_start = rope.line_to_char(cursor_line);
                    let line_length = rope.line(cursor_line).len_chars();
                    line_start + line_length
                };

                // delta X can wrap
                self.cursor = self.cursor.saturating_add_signed(delta_x).min(limit);
            }
            BufferContents::Hex(vec) => {
                let limit = if wraps {
                    vec.len()
                } else {
                    vec.len().min(self.cursor.div_ceil(16) * 16)
                };

                self.cursor = self.cursor.saturating_add_signed(delta_x).min(limit);
            }
        }
    }

    pub fn jump_cursor_y(&mut self, buffer: &Buffer, delta_y: isize) {
        // TODO: remember the X position even when jumping to a line that is shorter than that

        if delta_y == 0 {
            return;
        }

        match &buffer.contents {
            BufferContents::Text(rope) => {
                // figure out what X position the cursor is moved to
                let cursor_line = rope.char_to_line(self.cursor);
                let line_start = rope.line_to_char(cursor_line);
                let cursor_x = self.cursor - line_start;

                let target_line = cursor_line
                    .saturating_add_signed(delta_y)
                    .min(rope.len_lines() - 1);
                let target_line_len = rope.line(target_line).len_chars().saturating_sub(1);

                // place the cursor on the same X position or on the last char on the line
                let target_line_start = rope.line_to_char(target_line);
                self.cursor = target_line_start + target_line_len.min(cursor_x);
            }
            BufferContents::Hex(vec) => {
                self.cursor = self
                    .cursor
                    .saturating_add_signed(16 * delta_y)
                    .min(vec.len());
            }
        }
    }

    /// moves the cursor on top of the first character/byte on the current line
    ///
    /// doesnt not jump to the previous line
    pub fn jump_line_beg(&mut self, buffer: &Buffer) {
        match &buffer.contents {
            BufferContents::Text(rope) => {
                self.cursor = rope.line_to_char(rope.char_to_line(self.cursor));
            }
            BufferContents::Hex(..) => {
                self.cursor = self.cursor / 16 * 16;
            }
        };
    }

    /// moves the cursor on top of the last character/byte (excluding newlines) on the current line
    ///
    /// doesnt not jump to the next line
    pub fn jump_line_end(&mut self, buffer: &Buffer) {
        match &buffer.contents {
            BufferContents::Text(rope) => {
                let line = rope.char_to_line(self.cursor);
                let line_len = rope.line(line).len_chars();
                self.cursor = rope
                    .len_chars()
                    .min((rope.line_to_char(line) + line_len).saturating_sub(2));
            }
            BufferContents::Hex(vec) => {
                self.cursor = (self.cursor.div_ceil(16) * 16).min(vec.len());
            }
        };
    }

    /// moves the cursor on top of the first character/byte
    pub fn jump_beg(&mut self) {
        self.cursor = 0;
    }

    /// moves the cursor on top of the last character/byte (excluding newlines)
    pub fn jump_end(&mut self, buffer: &Buffer) {
        match &buffer.contents {
            BufferContents::Text(rope) => {
                self.cursor = rope.len_chars().saturating_sub(1);
            }
            BufferContents::Hex(vec) => {
                self.cursor = vec.len().saturating_sub(1);
            }
        };
    }
}

//

struct TextWidget<'a> {
    contents: RopeSlice<'a>,
    line: usize,
}

impl Widget for TextWidget<'_> {
    fn render(self, area: Rect, buf: &mut ratatui::prelude::Buffer) {
        for (y, line) in self
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

pub struct Cursor {
    line: usize,
    row: usize,
    col: usize,
    is_insert_mode: bool,
}

impl Widget for Cursor {
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
        if !self.is_insert_mode {
            buf[(col, row)]
                .set_bg(theme::CURSOR)
                .set_fg(theme::BACKGROUND);
        }
    }
}

pub struct LineNumbers {
    /// viewport first line
    line: usize,
    /// viewport line count
    lines: usize,
    /// cursor row
    row: usize,
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
                if y as usize + self.line != self.row {
                    buf[(x, y)].set_fg(theme::INACTIVE);
                }
            }
        }
    }
}
