use std::cmp::Ordering;

use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::Style,
    text::Line,
    widgets::{Block, Paragraph, Widget},
    Frame,
};

use crate::{buffer::Buffer, mode::Mode};

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

        let (row, col) = self.render_buffer(buffer, buffer_area, frame, mode.is_insert());

        // render the buffer line
        self.render_bufferline(buffer, bufferline_area, frame, mode.as_str(), col, row);

        let real_cursor_row = row - self.view_line + buffer_area.y as usize;
        let real_cursor_col =
            self.cursor - buffer.contents.line_to_char(row) + buffer_area.x as usize;

        (real_cursor_row, real_cursor_col)
    }

    fn render_buffer(
        &mut self,
        buffer: &Buffer,
        area: Rect,
        frame: &mut Frame,
        is_insert_mode: bool,
    ) -> (usize, usize) {
        let lines = buffer.contents.len_lines();

        let [_, line_numbers_area, _, buffer_area] = Layout::horizontal([
            Constraint::Length(2),
            Constraint::Length(lines.ilog10() as u16 + 1),
            Constraint::Length(2),
            Constraint::Min(0),
        ])
        .areas(area);

        let row = buffer.contents.char_to_line(self.cursor);
        let col = self.cursor - buffer.contents.line_to_char(row);

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
        let buffer_widget = BufferWidget {
            buffer,
            line: self.view_line,
        };
        frame.render_widget(buffer_widget, buffer_area);

        // render the cursor and cursor crosshair
        let cursor = Cursor {
            line: self.view_line,
            row,
            col,
            is_insert_mode,
        };
        frame.render_widget(cursor, buffer_area);

        (row, col)
    }

    fn render_bufferline(
        &mut self,
        buffer: &Buffer,
        area: Rect,
        frame: &mut Frame,
        mode: &str,
        col: usize,
        row: usize,
    ) {
        let cursor_pos = format!("{row}:{col}");
        let left = Line::from_iter([" ", mode, "   ", buffer.name.as_ref()]);
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
        buffer
            .contents
            .get_chars_at(from)
            .into_iter()
            .flatten()
            .take_while(|ch| pred(*ch))
            .count()
    }

    /// find the next matching `pred` starting and including `from`
    pub fn find(
        &self,
        buffer: &Buffer,
        from: usize,
        pred: impl FnMut(char) -> bool,
    ) -> Option<usize> {
        buffer
            .contents
            .get_chars_at(from)
            .into_iter()
            .flatten()
            .position(pred)
    }

    /// reverse find the next matching `pred` starting and including `from`
    pub fn rfind(
        &self,
        buffer: &Buffer,
        from: usize,
        pred: impl FnMut(char) -> bool,
    ) -> Option<usize> {
        buffer
            .contents
            .get_chars_at(from + 1)
            .map(|s| s.reversed())
            .into_iter()
            .flatten()
            .position(pred)
    }

    /// find the next word boundary starting and including `from`
    pub fn find_boundary(&self, buffer: &Buffer, from: usize) -> usize {
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
    pub fn rfind_boundary(&self, buffer: &Buffer, from: usize) -> usize {
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

    pub fn jump_cursor(&mut self, buffer: &Buffer, delta_x: isize, delta_y: isize) {
        if buffer.contents.len_chars() == 0 {
            // cant move if the buffer has nothing
            self.cursor = 0;
            return;
        }

        if delta_x != 0 {
            // delta X can wrap
            self.cursor = self
                .cursor
                .saturating_add_signed(delta_x)
                .min(buffer.contents.len_chars());
        }

        // delta Y from now on
        if delta_y == 0 || buffer.contents.len_lines() == 0 {
            return;
        }

        // figure out what X position the cursor is moved to
        let cursor_line = buffer.contents.char_to_line(self.cursor);
        let line_start = buffer.contents.line_to_char(cursor_line);
        let cursor_x = self.cursor - line_start;

        let target_line = cursor_line
            .saturating_add_signed(delta_y)
            .min(buffer.contents.len_lines() - 1);
        let target_line_len = buffer
            .contents
            .line(target_line)
            .len_chars()
            .saturating_sub(1);

        // place the cursor on the same X position or on the last char on the line
        let target_line_start = buffer.contents.line_to_char(target_line);
        self.cursor = target_line_start + target_line_len.min(cursor_x);
    }

    pub fn jump_line_beg(&mut self, buffer: &Buffer) {
        self.cursor = buffer
            .contents
            .line_to_char(buffer.contents.char_to_line(self.cursor));
    }

    pub fn jump_line_end(&mut self, buffer: &Buffer) {
        let line = buffer.contents.char_to_line(self.cursor);
        let line_len = buffer.contents.line(line).len_chars();
        self.cursor = buffer
            .contents
            .len_chars()
            .min((buffer.contents.line_to_char(line) + line_len).saturating_sub(2));
    }

    pub fn jump_beg(&mut self) {
        self.cursor = 0;
    }

    pub fn jump_end(&mut self, buffer: &Buffer) {
        self.cursor = buffer.contents.len_chars().saturating_sub(1);
    }
}

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
