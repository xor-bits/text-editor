use std::mem;

use crossterm::{
    cursor::SetCursorStyle,
    event::{self, Event, KeyEvent, KeyEventKind},
    execute, terminal,
};
use ratatui::{
    layout::{Constraint, Layout, Margin, Position, Rect},
    style::{Style, Stylize},
    text::Line,
    widgets::{Block, Clear},
    DefaultTerminal, Frame,
};

use crate::{
    buffer::Buffer,
    mode::{Mode, ModeSubset},
};

use self::{
    keymap::{ActionEntry, Code, Keymap},
    popup::Popup,
    view::BufferView,
};

//

pub mod actions;
pub mod keymap;
pub mod popup;
pub mod theme;
pub mod view;

//

pub struct Editor {
    pub should_close: bool,
    pub size: (u16, u16),
    pub real_cursor: (usize, usize),

    pub buffers: Vec<Buffer>,
    pub view: BufferView,

    pub popup: Popup,

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

            popup: <_>::default(),

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
                        x: self.real_cursor.1 as u16,
                        y: self.real_cursor.0 as u16,
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

        let [buffer_area, cmd_area] =
            Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(frame.area());

        // render the main buffer view
        self.real_cursor = BufferViewMut::new(&mut self.view, &mut self.buffers).render(
            &self.mode,
            buffer_area,
            frame,
        );

        // render the command suggestion box
        self.render_cmd_suggestions(buffer_area, frame);

        // render the command line
        self.render_cmdline(cmd_area, frame);

        // render popups like the file explorer or buffer picker
        self.render_popups(buffer_area, frame);
    }

    fn render_cmd_suggestions(&mut self, area: Rect, frame: &mut Frame) {
        // render suggestions as a popup over the buffer area
        let [_, area] = Layout::vertical([
            Constraint::Min(1),
            Constraint::Length((self.command_suggestions.len() as u16).min(10)),
        ])
        .areas(area);

        let suggestion_list_chunk_start = self
            .command_suggestion_index
            .unwrap_or(0)
            .checked_div(area.height as usize)
            .unwrap_or(0)
            .checked_mul(area.height as usize)
            .unwrap_or(0);
        let suggestion_bg = Block::new().style(Style::new().bg(theme::BACKGROUND_LIGHT));
        frame.render_widget(Clear, area);
        frame.render_widget(suggestion_bg, area);
        for (i, act) in self
            .command_suggestions
            .iter()
            .enumerate()
            .skip(suggestion_list_chunk_start)
            .take(area.height as usize)
        {
            let (fg, bg) = if Some(i) == self.command_suggestion_index {
                (theme::BACKGROUND_LIGHT, theme::CURSOR)
            } else {
                (theme::CURSOR, theme::BACKGROUND_LIGHT)
            };

            let area = Rect {
                x: area.x,
                y: area.y + (i - suggestion_list_chunk_start) as u16,
                width: area.width,
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
    }

    fn render_cmdline(&mut self, area: Rect, frame: &mut Frame) {
        let cmd = Block::new()
            // .style(Style::new().bg(Color::Black))
            .title(self.command.as_str());
        frame.render_widget(cmd, area);
        // frame.render_widget(Block::new().title("cmd area"), cmd_area);
    }

    fn render_popups(&mut self, area: Rect, frame: &mut Frame) {
        _ = area;
        let area = frame.area();
        let popup_area = area.inner(Margin {
            horizontal: (area.width as f32 * 0.1) as u16,
            vertical: (area.height as f32 * 0.1) as u16,
        });
        self.popup.render(&self.buffers, popup_area, frame);
    }

    pub fn event(&mut self, event: Event) {
        if !matches!(self.popup, Popup::None) {
            self.popup = mem::take(&mut self.popup).event(self, &event);
            return;
        }

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

    pub fn current(&self) -> BufferViewRef<'_> {
        BufferViewRef::new(&self.view, &self.buffers)
    }

    pub fn current_mut(&mut self) -> BufferViewMut<'_> {
        BufferViewMut::new(&mut self.view, &mut self.buffers)
    }

    pub fn open(&mut self, path: &str) {
        // look for existing open buffers
        for (i, existing) in self.buffers.iter().enumerate() {
            if existing.name.as_ref() == path {
                self.view = BufferView::new(i);
                return;
            }
        }

        match Buffer::open(path) {
            Ok(buf) => {
                let idx = self.buffers.len();
                self.buffers.push(buf);
                self.view = BufferView::new(idx);
            }
            Err(err) => tracing::error!("failed to open `{path}`: {err}"),
        }
    }
}

//

pub struct BufferViewRef<'a> {
    view: &'a BufferView,
    buffer: &'a Buffer,
}

impl<'a> BufferViewRef<'a> {
    pub const fn new(view: &'a BufferView, buffers: &'a [Buffer]) -> Self {
        BufferViewRef {
            buffer: &buffers[view.buffer_index],
            view,
        }
    }

    /// count matching characters starting and including `from`
    pub fn count_matching(&self, from: usize, pred: impl FnMut(char) -> bool) -> usize {
        self.view.count_matching(self.buffer, from, pred)
    }

    /// find the next matching `pred` starting and including `from`
    pub fn find(&self, from: usize, pred: impl FnMut(char) -> bool) -> Option<usize> {
        self.view.find(self.buffer, from, pred)
    }

    /// reverse find the next matching `pred` starting and including `from`
    pub fn rfind(&self, from: usize, pred: impl FnMut(char) -> bool) -> Option<usize> {
        self.view.rfind(self.buffer, from, pred)
    }

    /// find the next word boundary starting and including `from`
    pub fn find_boundary(&self, from: usize) -> usize {
        self.view.find_boundary(self.buffer, from)
    }

    /// reverse find the next word boundary starting and including `from`
    pub fn rfind_boundary(&self, from: usize) -> usize {
        self.view.rfind_boundary(self.buffer, from)
    }
}

pub struct BufferViewMut<'a> {
    view: &'a mut BufferView,
    buffer: &'a mut Buffer,
}

impl<'a> BufferViewMut<'a> {
    pub const fn new(view: &'a mut BufferView, buffers: &'a mut [Buffer]) -> Self {
        BufferViewMut {
            buffer: &mut buffers[view.buffer_index],
            view,
        }
    }

    pub fn render(
        self,
        mode: &Mode,
        area: Rect,
        frame: &mut ratatui::prelude::Frame,
    ) -> (usize, usize) {
        self.view.render(self.buffer, mode, area, frame)
    }

    /// count matching characters starting and including `from`
    pub fn count_matching(&self, from: usize, pred: impl FnMut(char) -> bool) -> usize {
        self.view.count_matching(self.buffer, from, pred)
    }

    /// find the next matching `pred` starting and including `from`
    pub fn find(&self, from: usize, pred: impl FnMut(char) -> bool) -> Option<usize> {
        self.view.find(self.buffer, from, pred)
    }

    /// reverse find the next matching `pred` starting and including `from`
    pub fn rfind(&self, from: usize, pred: impl FnMut(char) -> bool) -> Option<usize> {
        self.view.rfind(self.buffer, from, pred)
    }

    /// find the next word boundary starting and including `from`
    pub fn find_boundary(&self, from: usize) -> usize {
        self.view.find_boundary(self.buffer, from)
    }

    /// reverse find the next word boundary starting and including `from`
    pub fn rfind_boundary(&self, from: usize) -> usize {
        self.view.rfind_boundary(self.buffer, from)
    }

    pub fn jump_cursor(&mut self, delta_x: isize, delta_y: isize) {
        self.view.jump_cursor(self.buffer, delta_x, delta_y);
    }

    pub fn jump_line_beg(&mut self) {
        self.view.jump_line_beg(self.buffer);
    }

    pub fn jump_line_end(&mut self) {
        self.view.jump_line_end(self.buffer)
    }

    pub fn jump_beg(&mut self) {
        self.view.jump_beg()
    }

    pub fn jump_end(&mut self) {
        self.view.jump_end(self.buffer)
    }
}
