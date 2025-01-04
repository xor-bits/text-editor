use crossterm::cursor::SetCursorStyle;

//

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Insert { append: bool },
    Command,
}

impl Mode {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Mode::Normal => "NOR",
            Mode::Insert { .. } => "INS",
            Mode::Command => "CMD",
        }
    }

    pub const fn cursor_style(&self) -> SetCursorStyle {
        match self {
            Mode::Normal => SetCursorStyle::SteadyBlock,
            Mode::Insert { .. } => SetCursorStyle::SteadyBar,
            Mode::Command => SetCursorStyle::SteadyBar,
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
}
