use std::{borrow::Cow, fs, path::PathBuf, sync::Arc};

use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind};
use eyre::Result;
use ratatui::{
    layout::{Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Style},
    text::Line,
    widgets::{Block, Clear},
    Frame,
};

use crate::{
    buffer::{Buffer, CONN_POOL},
    tramp::Part,
};

use super::{theme, view::BufferView, Editor};

//

#[derive(Default)]
pub enum Popup {
    FileExplorer {
        files: Vec<(Cow<'static, str>, bool)>,
        remote: Option<Arc<[Part]>>,
        cwd: PathBuf,
        selected: usize,
    },
    BufferPicker {
        selected: usize,
    },
    #[default]
    None,
}

impl Popup {
    pub fn file_explorer(remote: Option<Arc<[Part]>>, mut cwd: PathBuf) -> Result<Self> {
        let mut files: Vec<(Cow<'static, str>, bool)>;

        if let Some(remote) = remote.clone() {
            let mut conn = CONN_POOL.connect_to(remote)?;
            cwd = conn.canonicalize(&cwd)?;
            let read_dir = conn.list_files(&cwd)?;

            files = [(Cow::Borrowed(".."), true)]
                .into_iter()
                .chain(
                    read_dir
                        .lines()
                        .skip(1) // skip the total: 5329835903590
                        .filter_map(|line| {
                            let is_dir = line.starts_with('d');
                            let name = line.split_whitespace().nth(8)?;

                            if name == "." || name == ".." {
                                return None;
                            }

                            Some((name.to_string().into(), is_dir))
                        }),
                )
                .collect();
        } else {
            cwd = cwd.canonicalize()?;
            let read_dir = fs::read_dir(&cwd)?;

            files = [Ok((Cow::Borrowed(".."), true))]
                .into_iter()
                .chain(read_dir.map(|entry| {
                    let entry = entry?;
                    let name: Cow<'_, str> =
                        entry.file_name().to_string_lossy().into_owned().into();
                    let is_dir = entry.file_type()?.is_dir();

                    Ok((name, is_dir))
                }))
                .collect::<Result<_>>()?;
        }

        files.sort_by(|a, b| (!a.1, a.0.as_ref()).cmp(&(!b.1, b.0.as_ref())));

        Ok(Self::FileExplorer {
            files,
            remote,
            cwd,
            selected: 0,
        })
    }

    pub fn buffer_picker(current: usize) -> Self {
        Self::BufferPicker { selected: current }
    }

    pub fn render(&mut self, buffers: &[Buffer], area: Rect, frame: &mut Frame) {
        match self {
            Popup::FileExplorer {
                files,
                selected,
                cwd: at,
                ..
            } => {
                let block = Block::bordered()
                    .title("File explorer")
                    .style(Style::new().bg(theme::BACKGROUND));
                frame.render_widget(Clear, area);
                frame.render_widget(block, area);

                let area = area.inner(Margin {
                    horizontal: 1,
                    vertical: 1,
                });

                let [area, pwd_area] = Layout::new(
                    Direction::Vertical,
                    [Constraint::Min(1), Constraint::Max(1)],
                )
                .areas(area);

                let pwd = Line::from_iter([at.to_string_lossy()])
                    .style(Style::new().fg(Color::LightGreen));
                frame.render_widget(pwd, pwd_area);

                let chunk_start = (*selected)
                    .checked_div(area.height as usize)
                    .unwrap_or(0)
                    .checked_mul(area.height as usize)
                    .unwrap_or(0);
                let chunk_len = area.height as usize;

                for ((i, (filename, is_dir)), area) in files
                    .iter()
                    .enumerate()
                    .skip(chunk_start)
                    .take(chunk_len)
                    .zip(area.rows())
                {
                    let mut bg = theme::BACKGROUND;
                    let mut fg = if *is_dir {
                        Color::LightBlue
                    } else {
                        theme::CURSOR
                    };

                    if *selected == i {
                        (fg, bg) = (bg, fg);
                    }

                    if *is_dir {
                        let entry = Line::from_iter([filename.as_ref(), "/"])
                            .style(Style::new().fg(fg).bg(bg));
                        frame.render_widget(entry, area);
                    } else {
                        let entry =
                            Line::from_iter([filename.as_ref()]).style(Style::new().fg(fg).bg(bg));
                        frame.render_widget(entry, area);
                    }
                }
            }
            Popup::BufferPicker { selected } => {
                let block = Block::bordered()
                    .title("Buffer picker")
                    .style(Style::new().bg(theme::BACKGROUND));
                frame.render_widget(Clear, area);
                frame.render_widget(block, area);

                let area = area.inner(Margin {
                    horizontal: 1,
                    vertical: 1,
                });

                let chunk_start = (*selected)
                    .checked_div(area.height as usize)
                    .unwrap_or(0)
                    .checked_mul(area.height as usize)
                    .unwrap_or(0);
                let chunk_len = area.height as usize;

                for ((i, buffer), area) in buffers
                    .iter()
                    .enumerate()
                    .skip(chunk_start)
                    .take(chunk_len)
                    .zip(area.rows())
                {
                    let mut bg = theme::BACKGROUND;
                    let mut fg = theme::CURSOR;

                    if *selected == i {
                        (fg, bg) = (bg, fg);
                    }

                    let entry =
                        Line::from_iter([buffer.name.as_ref()]).style(Style::new().fg(fg).bg(bg));
                    frame.render_widget(entry, area);
                }
            }
            Popup::None => {}
        }
    }

    pub fn event(mut self, editor: &mut Editor, event: &Event) -> Self {
        match &mut self {
            Popup::FileExplorer {
                cwd,
                selected,
                remote,
                files,
            } => match event {
                Event::Key(KeyEvent {
                    code: KeyCode::Esc,
                    kind: KeyEventKind::Press,
                    ..
                }) => Popup::None,
                Event::Key(KeyEvent {
                    code: KeyCode::Up,
                    kind: KeyEventKind::Press,
                    ..
                }) => {
                    *selected = (*selected + files.len() - 1) % files.len();
                    self
                }
                Event::Key(KeyEvent {
                    code: KeyCode::Down,
                    kind: KeyEventKind::Press,
                    ..
                }) => {
                    *selected = (*selected + 1) % files.len();
                    self
                }
                Event::Key(KeyEvent {
                    code: KeyCode::Left,
                    kind: KeyEventKind::Press,
                    ..
                }) => {
                    cwd.pop();
                    match Popup::file_explorer(remote.clone(), cwd.clone()) {
                        Ok(v) => v,
                        Err(err) => {
                            tracing::error!("failed to travel directories: {err}");
                            Popup::None
                        }
                    }
                }
                Event::Key(KeyEvent {
                    code: KeyCode::Right | KeyCode::Enter,
                    kind: KeyEventKind::Press,
                    ..
                }) => {
                    let Some((filename, is_dir)) = files.get(*selected) else {
                        return self;
                    };

                    cwd.push(filename.as_ref());
                    if *is_dir {
                        match Popup::file_explorer(remote.clone(), cwd.clone()) {
                            Ok(v) => v,
                            Err(err) => {
                                tracing::error!("failed to travel directories: {err}");
                                self
                            }
                        }
                    } else {
                        match cwd.as_os_str().to_str() {
                            Some(path) => {
                                editor.open(path);
                                Popup::None
                            }
                            None => {
                                tracing::error!("invalid path: '{cwd:?}'");
                                self
                            }
                        }
                    }
                }
                _ => self,
            },
            Popup::BufferPicker { selected } => match event {
                Event::Key(KeyEvent {
                    code: KeyCode::Up,
                    kind: KeyEventKind::Press,
                    ..
                }) => {
                    *selected = (*selected + editor.buffers.len() - 1) % editor.buffers.len();
                    self
                }
                Event::Key(KeyEvent {
                    code: KeyCode::Down,
                    kind: KeyEventKind::Press,
                    ..
                }) => {
                    *selected = (*selected + 1) % editor.buffers.len();
                    self
                }
                Event::Key(KeyEvent {
                    code: KeyCode::Left | KeyCode::Esc,
                    kind: KeyEventKind::Press,
                    ..
                }) => Popup::None,
                Event::Key(KeyEvent {
                    code: KeyCode::Right | KeyCode::Enter,
                    kind: KeyEventKind::Press,
                    ..
                }) => {
                    editor.view = BufferView::new(*selected);
                    Popup::None
                }
                _ => self,
            },
            Popup::None => self,
        }
    }
}
