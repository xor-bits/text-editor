use std::sync::Arc;

use crossterm::cursor::SetCursorStyle;

use crate::editor::keymap::Layer;

//

#[derive(Clone)]
pub enum Mode {
    Normal,
    Insert {
        append: bool,
    },
    Command,
    Action {
        layer: Arc<dyn Layer>,
        prev: ModeSubset,
    },
}

impl Mode {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Mode::Normal => "NOR",
            Mode::Insert { .. } => "INS",
            Mode::Command => "CMD",
            Mode::Action { .. } => "ACT",
        }
    }

    pub const fn prev(&self) -> ModeSubset {
        match self {
            Mode::Normal => ModeSubset::Normal,
            Mode::Insert { append } => ModeSubset::Insert { append: *append },
            Mode::Command => ModeSubset::Command,
            Mode::Action { prev, .. } => *prev,
        }
    }

    pub const fn cursor_style(&self) -> SetCursorStyle {
        match self {
            Mode::Normal => SetCursorStyle::SteadyBlock,
            Mode::Insert { .. } => SetCursorStyle::SteadyBar,
            Mode::Command => SetCursorStyle::SteadyBar,
            Mode::Action { .. } => SetCursorStyle::SteadyBlock,
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

    /// Returns `true` if the mode is [`Action`].
    ///
    /// [`Action`]: Mode::Action
    #[must_use]
    pub fn is_action(&self) -> bool {
        matches!(self, Self::Action { .. })
    }
}

//

#[derive(Clone, Copy)]
pub enum ModeSubset {
    Normal,
    Insert { append: bool },
    Command,
}

impl ModeSubset {
    pub const fn mode(self) -> Mode {
        match self {
            ModeSubset::Normal => Mode::Normal,
            ModeSubset::Insert { append } => Mode::Insert { append },
            ModeSubset::Command => Mode::Command,
        }
    }
}
