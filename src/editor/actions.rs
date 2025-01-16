use std::sync::Arc;

use crossterm::event::{KeyCode, KeyModifiers};

use crate::{
    editor::keymap::{Code, Entry, Layer},
    mode::Mode,
};

use super::{
    keymap::{Action, ActionExt, DEFAULT_ACTIONS},
    Editor,
};

//

pub fn all_actions() -> impl IntoIterator<Item = Arc<dyn Action>> {
    [
        Escape::arc(),
        //
        MoveLeft::arc(),
        MoveRight::arc(),
        MoveUp::arc(),
        MoveDown::arc(),
        MovePageUp::arc(),
        MovePageDown::arc(),
        MoveLineBeg::arc(),
        MoveLineEnd::arc(),
        MoveBufferBeg::arc(),
        MoveBufferEnd::arc(),
        //
        NextWordBeg::arc(),
        NextWordEnd::arc(),
        PrevWordBeg::arc(),
        //
        SwitchToInsert::arc(),
        SwitchToInsertLineBeg::arc(),
        SwitchToAppend::arc(),
        SwitchToAppendLineEnd::arc(),
        SwitchToCommand::arc(),
        InsertLineAbove::arc(),
        InsertLineBelow::arc(),
        //
        Delete::arc(),
        Backspace::arc(),
        //
        Quit::arc(),
        QuitForce::arc(),
        Write::arc(),
        WriteQuit::arc(),
        WriteQuitForce::arc(),
    ]
}

//

#[derive(Debug, Default)]
pub struct Escape;

impl Action for Escape {
    fn name(&self) -> &str {
        "escape"
    }

    fn description(&self) -> &str {
        "escape"
    }

    fn run(&self, editor: &mut Editor) {
        if let Mode::Insert { append: true } = editor.mode {
            editor.cursor -= 1;
        }
        editor.mode = Mode::Normal;
        editor.command.clear();
    }
}

//

#[derive(Debug, Default)]
pub struct MoveLeft;

impl Action for MoveLeft {
    fn name(&self) -> &str {
        "move-left"
    }

    fn description(&self) -> &str {
        "move left"
    }

    fn run(&self, editor: &mut Editor) {
        editor.jump_cursor(-1, 0);
    }
}

//

#[derive(Debug, Default)]
pub struct MoveRight;

impl Action for MoveRight {
    fn name(&self) -> &str {
        "move-right"
    }

    fn description(&self) -> &str {
        "move right"
    }

    fn run(&self, editor: &mut Editor) {
        editor.jump_cursor(1, 0);
    }
}

//

#[derive(Debug, Default)]
pub struct MoveUp;

impl Action for MoveUp {
    fn name(&self) -> &str {
        "move-up"
    }

    fn description(&self) -> &str {
        "move up"
    }

    fn run(&self, editor: &mut Editor) {
        editor.jump_cursor(0, -1);
    }
}

//

#[derive(Debug, Default)]
pub struct MoveDown;

impl Action for MoveDown {
    fn name(&self) -> &str {
        "move-down"
    }

    fn description(&self) -> &str {
        "move down"
    }

    fn run(&self, editor: &mut Editor) {
        editor.jump_cursor(0, 1);
    }
}

//

#[derive(Debug, Default)]
pub struct MovePageUp;

impl Action for MovePageUp {
    fn name(&self) -> &str {
        "move-page-up"
    }

    fn description(&self) -> &str {
        "move one page up"
    }

    fn run(&self, editor: &mut Editor) {
        editor.jump_cursor(0, -(editor.size.1 as isize - 1))
    }
}

//

#[derive(Debug, Default)]
pub struct MovePageDown;

impl Action for MovePageDown {
    fn name(&self) -> &str {
        "move-page-down"
    }

    fn description(&self) -> &str {
        "move one page down"
    }

    fn run(&self, editor: &mut Editor) {
        editor.jump_cursor(0, editor.size.1 as isize - 3)
    }
}

//

#[derive(Debug, Default)]
pub struct MoveLineBeg;

impl Action for MoveLineBeg {
    fn name(&self) -> &str {
        "move-line-beg"
    }

    fn description(&self) -> &str {
        "move to current line beginning"
    }

    fn run(&self, editor: &mut Editor) {
        editor.jump_line_beg()
    }
}

//

#[derive(Debug, Default)]
pub struct MoveLineEnd;

impl Action for MoveLineEnd {
    fn name(&self) -> &str {
        "move-line-end"
    }

    fn description(&self) -> &str {
        "move to current line end"
    }

    fn run(&self, editor: &mut Editor) {
        editor.jump_line_end()
    }
}

//

#[derive(Debug, Default)]
pub struct MoveBufferBeg;

impl Action for MoveBufferBeg {
    fn name(&self) -> &str {
        "move-buffer-beg"
    }

    fn description(&self) -> &str {
        "move to the beginning"
    }

    fn run(&self, editor: &mut Editor) {
        editor.jump_beg();
    }
}

//

#[derive(Debug, Default)]
pub struct MoveBufferEnd;

impl Action for MoveBufferEnd {
    fn name(&self) -> &str {
        "move-buffer-end"
    }

    fn description(&self) -> &str {
        "move to the end of buffer"
    }

    fn run(&self, editor: &mut Editor) {
        editor.jump_end();
    }
}

//

#[derive(Debug, Default)]
pub struct NextWordBeg;

impl Action for NextWordBeg {
    fn name(&self) -> &str {
        "next-word-beg"
    }

    fn description(&self) -> &str {
        "move to the start of next word"
    }

    fn run(&self, editor: &mut Editor) {
        if editor.cursor + 1 >= editor.buffer.contents.len_chars() {
            return;
        }

        editor.cursor += 1;
        editor.cursor += editor.find_boundary(editor.cursor);
        editor.cursor += editor.count_matching(editor.cursor + 1, |ch| ch.is_whitespace());
    }
}

//

#[derive(Debug, Default)]
pub struct NextWordEnd;

impl Action for NextWordEnd {
    fn name(&self) -> &str {
        "next-word-end"
    }

    fn description(&self) -> &str {
        "move to the start of next word"
    }

    fn run(&self, editor: &mut Editor) {
        if editor.cursor + 1 >= editor.buffer.contents.len_chars() {
            return;
        }

        editor.cursor += 1;
        editor.cursor += editor.find_boundary(editor.cursor);
    }
}

//

#[derive(Debug, Default)]
pub struct PrevWordBeg;

impl Action for PrevWordBeg {
    fn name(&self) -> &str {
        "prev-word-beg"
    }

    fn description(&self) -> &str {
        "move to the start of previous word"
    }

    fn run(&self, editor: &mut Editor) {
        if editor.cursor == 0 {
            return;
        }

        editor.cursor -= 1;
        editor.cursor -= editor.rfind_boundary(editor.cursor);
    }
}

//

#[derive(Debug, Default)]
pub struct SwitchToInsert;

impl Action for SwitchToInsert {
    fn name(&self) -> &str {
        "switch-to-insert"
    }

    fn description(&self) -> &str {
        "switch to insert mode"
    }

    fn run(&self, editor: &mut Editor) {
        editor.mode = Mode::Insert { append: false };
    }
}

//

#[derive(Debug, Default)]
pub struct SwitchToInsertLineBeg;

impl Action for SwitchToInsertLineBeg {
    fn name(&self) -> &str {
        "switch-to-insert-line-beg"
    }

    fn description(&self) -> &str {
        "switch to insert mode at the start of current line"
    }

    fn run(&self, editor: &mut Editor) {
        editor.mode = Mode::Insert { append: false };
        editor.jump_line_beg();
    }
}

//

#[derive(Debug, Default)]
pub struct SwitchToAppend;

impl Action for SwitchToAppend {
    fn name(&self) -> &str {
        "switch-to-append"
    }

    fn description(&self) -> &str {
        "switch to append mode"
    }

    fn run(&self, editor: &mut Editor) {
        editor.mode = Mode::Insert { append: true };
        editor.jump_cursor(1, 0);
    }
}

//

#[derive(Debug, Default)]
pub struct SwitchToAppendLineEnd;

impl Action for SwitchToAppendLineEnd {
    fn name(&self) -> &str {
        "switch-to-append-line-end"
    }

    fn description(&self) -> &str {
        "switch to append mode at the end of current line"
    }

    fn run(&self, editor: &mut Editor) {
        editor.mode = Mode::Insert { append: true };
        editor.jump_line_end();
    }
}

//

#[derive(Debug, Default)]
pub struct SwitchToCommand;

impl Action for SwitchToCommand {
    fn name(&self) -> &str {
        "switch-to-command"
    }

    fn description(&self) -> &str {
        "insert a line above and switch to insert mode"
    }

    fn run(&self, editor: &mut Editor) {
        editor.mode = Mode::Command;
        editor.command.clear();
        editor.command.push(':');
    }
}

//

#[derive(Debug, Default)]
pub struct InsertLineBelow;

impl Action for InsertLineBelow {
    fn name(&self) -> &str {
        "insert-line-below"
    }

    fn description(&self) -> &str {
        "insert a line below and switch to insert mode"
    }

    fn run(&self, editor: &mut Editor) {
        editor.mode = Mode::Insert { append: true };
        editor.jump_line_end();
        editor.buffer.contents.insert_char(editor.cursor, '\n');
        editor.jump_cursor(1, 0);
    }
}

//

#[derive(Debug, Default)]
pub struct InsertLineAbove;

impl Action for InsertLineAbove {
    fn name(&self) -> &str {
        "insert-line-above"
    }

    fn description(&self) -> &str {
        "insert a line above and switch to insert mode"
    }

    fn run(&self, editor: &mut Editor) {
        editor.mode = Mode::Insert { append: true };
        editor.jump_line_beg();
        editor.buffer.contents.insert_char(editor.cursor, '\n');
    }
}

//

#[derive(Debug, Default)]
pub struct JumpForwardsTo;

impl Layer for JumpForwardsTo {
    fn name(&self) -> &str {
        "jump-forwards-to"
    }

    fn description(&self) -> &str {
        "jump forwards to the next matching character"
    }

    fn get(&self, _: Code) -> Option<Entry> {
        None
    }

    fn run(&self, keycode: Code, editor: &mut Editor) -> bool {
        let KeyCode::Char(ch) = keycode.keycode else {
            return false;
        };

        editor.cursor += editor
            .find(editor.cursor + 1, |cur_ch| cur_ch == ch)
            .map_or(0, |n| n + 1);
        editor.mode = Mode::Normal;
        true
    }
}

//

#[derive(Debug, Default)]
pub struct JumpForwardsUntil;

impl Layer for JumpForwardsUntil {
    fn name(&self) -> &str {
        "jump-forwards-until"
    }

    fn description(&self) -> &str {
        "jump forwards until the next character is matching"
    }

    fn get(&self, _: Code) -> Option<Entry> {
        None
    }

    fn run(&self, keycode: Code, editor: &mut Editor) -> bool {
        let KeyCode::Char(ch) = keycode.keycode else {
            return false;
        };

        editor.cursor += editor
            .find(editor.cursor + 2, |cur_ch| cur_ch == ch)
            .map_or(0, |n| n + 1);
        editor.mode = Mode::Normal;
        true
    }
}

//

#[derive(Debug, Default)]
pub struct JumpBackwardsTo;

impl Layer for JumpBackwardsTo {
    fn name(&self) -> &str {
        "jump-backwards-to"
    }

    fn description(&self) -> &str {
        "jump backwards to the next matching character"
    }

    fn get(&self, _: Code) -> Option<Entry> {
        None
    }

    fn run(&self, keycode: Code, editor: &mut Editor) -> bool {
        let KeyCode::Char(ch) = keycode.keycode else {
            return false;
        };

        if editor.cursor == 0 {
            editor.mode = Mode::Normal;
            return false;
        }
        editor.cursor -= editor
            .rfind(editor.cursor - 1, |cur_ch| cur_ch == ch)
            .map_or(0, |n| n + 1);
        editor.mode = Mode::Normal;
        true
    }
}

//

#[derive(Debug, Default)]
pub struct JumpBackwardsUntil;

impl Layer for JumpBackwardsUntil {
    fn name(&self) -> &str {
        "jump-backwards-until"
    }

    fn description(&self) -> &str {
        "jump backwards until the next character is matching"
    }

    fn get(&self, _: Code) -> Option<Entry> {
        None
    }

    fn run(&self, keycode: Code, editor: &mut Editor) -> bool {
        let KeyCode::Char(ch) = keycode.keycode else {
            return false;
        };

        if editor.cursor <= 1 {
            editor.mode = Mode::Normal;
            return false;
        }
        editor.cursor -= editor
            .rfind(editor.cursor - 2, |cur_ch| cur_ch == ch)
            .map_or(0, |n| n + 1);
        editor.mode = Mode::Normal;
        true
    }
}

//

#[derive(Debug, Default)]
pub struct Delete;

impl Action for Delete {
    fn name(&self) -> &str {
        "delete"
    }

    fn description(&self) -> &str {
        "delete the current selection"
    }

    fn run(&self, editor: &mut Editor) {
        if editor.cursor == 0 {
            return;
        }

        _ = editor
            .buffer
            .contents
            .try_remove(editor.cursor..editor.cursor + 1);
    }
}

//

#[derive(Debug, Default)]
pub struct Backspace;

impl Action for Backspace {
    fn name(&self) -> &str {
        "backspace"
    }

    fn description(&self) -> &str {
        "delete the current selection, and move backwards"
    }

    fn run(&self, editor: &mut Editor) {
        match editor.mode {
            Mode::Insert { .. } => {
                if editor.cursor == 0 {
                    return;
                }

                editor
                    .buffer
                    .contents
                    .remove(editor.cursor - 1..editor.cursor);
                editor.jump_cursor(-1, 0);
            }
            Mode::Command => {
                if editor.command.len() >= 2 {
                    _ = editor.command.pop();
                }
            }
            _ => {}
        }
    }
}

//

#[derive(Debug, Default)]
pub struct TypeChar;

impl Layer for TypeChar {
    fn name(&self) -> &str {
        "type-char"
    }

    fn get(&self, _: Code) -> Option<Entry> {
        None
    }

    fn run(&self, keycode: Code, editor: &mut Editor) -> bool {
        let ch = match keycode {
            Code {
                keycode: KeyCode::Char(ch),
                ..
            } => ch,
            Code {
                keycode: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
            } => '\n',
            _ => return false,
        };

        match editor.mode {
            Mode::Insert { .. } => {
                editor.buffer.contents.insert_char(editor.cursor, ch);
                editor.jump_cursor(1, 0);
            }
            Mode::Command => {
                if ch == '\n' {
                    editor.mode = Mode::Normal;

                    let Some(act) =
                        DEFAULT_ACTIONS.get(editor.command.as_str().trim_start_matches(':'))
                    else {
                        editor.command.clear();
                        editor.command.push_str("invalid command");
                        return true;
                    };

                    editor.command.clear();
                    act.act.run(editor);
                } else {
                    editor.command.push(ch);
                }
            }
            _ => {}
        }

        true
    }
}

//

#[derive(Debug, Default)]
pub struct Quit;

impl Action for Quit {
    fn name(&self) -> &str {
        "q"
    }

    fn run(&self, editor: &mut Editor) {
        // TODO: dont close if unsaved
        editor.should_close = true;
    }
}

//

#[derive(Debug, Default)]
pub struct QuitForce;

impl Action for QuitForce {
    fn name(&self) -> &str {
        "q!"
    }

    fn run(&self, editor: &mut Editor) {
        editor.should_close = true;
    }
}

//

#[derive(Debug, Default)]
pub struct Write;

impl Action for Write {
    fn name(&self) -> &str {
        "w"
    }

    fn run(&self, editor: &mut Editor) {
        editor.should_close = true;
    }
}

//

#[derive(Debug, Default)]
pub struct WriteQuit;

impl Action for WriteQuit {
    fn name(&self) -> &str {
        "x"
    }

    fn run(&self, editor: &mut Editor) {
        // TODO: dont close if unsaved
        editor.buffer.write().unwrap();
        editor.should_close = true;
    }
}

//

#[derive(Debug, Default)]
pub struct WriteQuitForce;

impl Action for WriteQuitForce {
    fn name(&self) -> &str {
        "x!"
    }

    fn run(&self, editor: &mut Editor) {
        editor.buffer.write().unwrap();
        editor.should_close = true;
    }
}
