use std::{cell::RefCell, cmp::Ordering, rc::Rc};

use ratatui::{
    layout::{Constraint, Layout, Rect},
    widgets::{Paragraph, Widget},
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

    pub fn render<'a>(
        &mut self,
        buffers: &'a [Buffer],
        mode: &Mode,
        area: Rect,
        frame: &mut ratatui::prelude::Frame,
    ) -> ((usize, usize), &'a str, (usize, usize)) {
        let buffer = &buffers[self.buffer_index];

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
            mode,
        };
        frame.render_widget(cursor, buffer_area);

        let real_cursor_row = row - self.view_line + buffer_area.y as usize;
        let real_cursor_col =
            self.cursor - buffer.contents.line_to_char(row) + buffer_area.x as usize;

        (
            (col, row),
            buffer.lossy_name.as_ref(),
            (real_cursor_col, real_cursor_row),
        )
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
