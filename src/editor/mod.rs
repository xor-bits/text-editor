use std::{borrow::Cow, mem};

use crossterm::{
    cursor::SetCursorStyle,
    event::{self, Event, KeyEvent, KeyEventKind},
    execute, terminal,
};
use ratatui::{
    layout::{Constraint, Layout, Margin, Position, Rect},
    style::{Color, Style, Stylize},
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

    pub status: String,
    pub status_is_error: bool,

    pub mode: Mode,
    pub force_whichkey: bool,

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

            status: String::new(),
            status_is_error: false,

            mode: Mode::Normal,
            force_whichkey: false,

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

        // render a keymapping layer helper
        self.render_whichkey(buffer_area, frame);
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
        if !self.command.is_empty() {
            let cmd = Block::new()
                // .style(Style::new().bg(Color::Black))
                .title(self.command.as_str());
            frame.render_widget(cmd, area);
            // frame.render_widget(Block::new().title("cmd area"), cmd_area);
            return;
        }

        if !self.status.is_empty() {
            let status = Block::new()
                // .style(Style::new().bg(Color::Black))
                .title(self.status.as_str())
                .style(Style::new().fg(if self.status_is_error {
                    Color::Red
                } else {
                    Color::Reset
                }));
            frame.render_widget(status, area);
        }
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

    fn render_whichkey(&mut self, area: Rect, frame: &mut Frame) {
        let layer = if let Mode::Action { layer, .. } = &self.mode {
            layer.clone()
        } else if self.force_whichkey {
            match self.mode {
                Mode::Normal => self.keymap.normal(),
                Mode::Insert { .. } => self.keymap.insert(),
                Mode::Command => self.keymap.command(),
                Mode::Action { ref layer, .. } => layer.clone(),
            }
        } else {
            return;
        };

        let entries = layer.entries();
        let wildcard = layer.wildcard();

        let width = entries
            .iter()
            .map(|(code, entry)| {
                code.as_str(&mut [const { 0 }; 16]).len() + 1 + entry.description().len()
            })
            .max()
            .unwrap_or(0)
            .max(wildcard.map_or(0, |desc| 2 + desc.description().len()));
        // .max(layer.description().len());

        let height = entries.len() + wildcard.is_some() as usize /* + 1 */;

        let [_, area] =
            Layout::horizontal([Constraint::Percentage(100), Constraint::Min(width as _)])
                .areas(area);
        let [_, area, _] = Layout::vertical([
            Constraint::Percentage(100),
            Constraint::Min(height as _),
            Constraint::Length(1),
        ])
        .areas(area);

        frame.render_widget(Clear, area);
        frame.render_widget(
            Block::new()
                // .title(layer.description())
                .style(Style::new().bg(theme::BACKGROUND_LIGHT)),
            area,
        );

        for ((key_name, action_name), area) in wildcard
            .into_iter()
            .map(|wildcard_description| (Cow::Borrowed("*"), wildcard_description.description()))
            .chain(entries.iter().map(|(code, entry)| {
                let key = code.as_str(&mut [const { 0 }; 16]).to_string().into();
                let entry = entry.description();
                (key, entry)
            }))
            .zip(area.rows() /*.skip(1)*/)
        {
            let action_name = Line::from_iter([action_name])
                .right_aligned()
                .style(Style::new().fg(theme::ACCENT));
            let key_name = Line::from_iter([key_name]).left_aligned().fg(Color::Reset);
            let line = Block::new().title(action_name).title(key_name);
            frame.render_widget(line, area);
        }
    }

    pub fn event(&mut self, event: Event) {
        self.force_whichkey = false;
        self.status.clear();

        // tracing::debug!("ev {event:?}");
        if tracing::enabled!(tracing::Level::DEBUG) {
            if let Event::Key(KeyEvent {
                code,
                modifiers,
                kind: KeyEventKind::Press,
                ..
            }) = event
            {
                let mut buf = [const { 0 }; 16];
                let key_name = Code::from_event(code, modifiers).as_str(&mut buf);
                tracing::debug!("pressed '{key_name}'");
            }
        }

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

    pub fn find_opened(&self, path: &str) -> Option<usize> {
        // look for existing open buffers
        for (i, existing) in self.buffers.iter().enumerate() {
            if existing.name.as_ref() == path {
                return Some(i);
            }
        }
        None
    }

    pub fn switch_to(&mut self, i: usize) {
        std::debug_assert!(i < self.buffers.len());
        self.view = BufferView::new(i);
    }

    pub fn open(&mut self, path: &str) {
        if let Some(i) = self.find_opened(path) {
            self.switch_to(i);
            return;
        }

        match Buffer::open(path) {
            Ok(buf) => self.open_from(buf),
            Err(err) => tracing::error!("failed to open `{path}`: {err}"),
        }
    }

    pub fn open_from(&mut self, buf: Buffer) {
        let idx = self.buffers.len();
        self.buffers.push(buf);
        self.view = BufferView::new(idx);
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
