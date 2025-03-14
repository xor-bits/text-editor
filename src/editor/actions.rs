use std::{env, path::PathBuf, sync::Arc};

use crossterm::event::{KeyCode, KeyModifiers};

use crate::{
    buffer::{Buffer, BufferContents, BufferInner},
    editor::{
        keymap::{Code, Entry, Layer},
        popup::Popup,
    },
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
        //
        ClearLog::arc(),
        RefreshSuggestions::arc(),
        NextSuggestion::arc(),
        PrevSuggestion::arc(),
        //
        Open::arc(),
        BufferClose::arc(),
        BufferNext::arc(),
        BufferPrev::arc(),
        //
        FileExplorer::arc(),
        BufferPicker::arc(),
        //
        WhichKey::arc(),
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
            let cur = editor.current_mut();
            cur.view.cursor = cur.view.cursor.saturating_sub(1);
        }
        editor.mode = Mode::Normal;
        editor.command.clear();
        editor.command_suggestions.clear();
        editor.command_suggestion_index = None;
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
        editor.current_mut().jump_cursor(-1, 0);
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
        editor.current_mut().jump_cursor(1, 0);
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
        editor.current_mut().jump_cursor(0, -1);
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
        editor.current_mut().jump_cursor(0, 1);
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
        let size = editor.size;
        editor.current_mut().jump_cursor(0, -(size.1 as isize - 1))
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
        let size = editor.size;
        editor.current_mut().jump_cursor(0, size.1 as isize - 3)
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
        editor.current_mut().jump_line_beg()
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
        editor.current_mut().jump_line_end()
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
        editor.current_mut().jump_beg();
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
        editor.current_mut().jump_end();
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
        let cur = editor.current_mut();

        match cur.buffer.contents {
            BufferContents::Text(ref rope) => {
                if cur.view.cursor + 1 >= rope.len_chars() {
                    return;
                }

                cur.view.cursor += 1;
                cur.view.cursor += cur.find_boundary(cur.view.cursor);
                cur.view.cursor += cur.count_matching(cur.view.cursor + 1, |ch| ch.is_whitespace());
            }
        }
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
        let cur = editor.current_mut();

        match cur.buffer.contents {
            BufferContents::Text(ref rope) => {
                if cur.view.cursor + 1 >= rope.len_chars() {
                    return;
                }

                cur.view.cursor += 1;
                cur.view.cursor += cur.find_boundary(cur.view.cursor);
            }
        }
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
        let cur = editor.current_mut();

        if cur.view.cursor == 0 {
            return;
        }

        cur.view.cursor -= 1;
        cur.view.cursor -= cur.rfind_boundary(cur.view.cursor);
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
        editor.current_mut().jump_line_beg();
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
        editor.current_mut().jump_cursor(1, 0);
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
        editor.current_mut().jump_line_end();
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
        "switch to command mode"
    }

    fn run(&self, editor: &mut Editor) {
        editor.mode = Mode::Command;
        editor.command.clear();
        editor.command.push(':');
        RefreshSuggestions.run(editor);
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
        let mut cur = editor.current_mut();
        cur.jump_line_end();

        match cur.buffer.contents {
            BufferContents::Text(ref mut rope) => {
                rope.insert_char(cur.view.cursor, '\n');
                cur.buffer.modified = true;
                cur.jump_cursor(1, 0);
            }
        }
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
        let mut cur = editor.current_mut();
        cur.jump_line_beg();

        match cur.buffer.contents {
            BufferContents::Text(ref mut rope) => {
                rope.insert_char(cur.view.cursor, '\n');
                cur.buffer.modified = true;
            }
        }
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

    fn entries(&self) -> Arc<[(Code, Entry)]> {
        <_>::default()
    }

    fn run(&self, keycode: Code, editor: &mut Editor) -> bool {
        let KeyCode::Char(ch) = keycode.keycode else {
            return false;
        };

        let cur = editor.current_mut();
        cur.view.cursor += cur
            .find(cur.view.cursor + 1, |cur_ch| cur_ch == ch)
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

    fn entries(&self) -> Arc<[(Code, Entry)]> {
        <_>::default()
    }

    fn wildcard(&self) -> Option<&dyn Layer> {
        Some(self)
    }

    fn run(&self, keycode: Code, editor: &mut Editor) -> bool {
        let KeyCode::Char(ch) = keycode.keycode else {
            return false;
        };

        let cur = editor.current_mut();
        cur.view.cursor += cur
            .find(cur.view.cursor + 2, |cur_ch| cur_ch == ch)
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

    fn entries(&self) -> Arc<[(Code, Entry)]> {
        <_>::default()
    }

    fn wildcard(&self) -> Option<&dyn Layer> {
        Some(self)
    }

    fn run(&self, keycode: Code, editor: &mut Editor) -> bool {
        let KeyCode::Char(ch) = keycode.keycode else {
            return false;
        };

        let cur = editor.current_mut();

        if cur.view.cursor == 0 {
            editor.mode = Mode::Normal;
            return false;
        }
        cur.view.cursor -= cur
            .rfind(cur.view.cursor - 1, |cur_ch| cur_ch == ch)
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

    fn entries(&self) -> Arc<[(Code, Entry)]> {
        <_>::default()
    }

    fn wildcard(&self) -> Option<&dyn Layer> {
        Some(self)
    }

    fn run(&self, keycode: Code, editor: &mut Editor) -> bool {
        let KeyCode::Char(ch) = keycode.keycode else {
            return false;
        };

        let cur = editor.current_mut();

        if cur.view.cursor <= 1 {
            editor.mode = Mode::Normal;
            return false;
        }
        cur.view.cursor -= cur
            .rfind(cur.view.cursor - 2, |cur_ch| cur_ch == ch)
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
        let cur = editor.current_mut();
        if cur.view.cursor == 0 {
            return;
        }

        match cur.buffer.contents {
            BufferContents::Text(ref mut rope) => {
                if rope
                    .try_remove(cur.view.cursor..cur.view.cursor + 1)
                    .is_ok()
                {
                    cur.buffer.modified = true;
                }
            }
        }
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
                let mut cur = editor.current_mut();
                if cur.view.cursor == 0 {
                    return;
                }

                match cur.buffer.contents {
                    BufferContents::Text(ref mut rope) => {
                        rope.remove(cur.view.cursor - 1..cur.view.cursor);
                        cur.buffer.modified = true;
                        cur.jump_cursor(-1, 0);
                    }
                }
            }
            Mode::Command => {
                if editor.command.len() >= 2 {
                    _ = editor.command.pop();
                    RefreshSuggestions.run(editor);
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

    fn description(&self) -> &str {
        "type character *"
    }

    fn get(&self, _: Code) -> Option<Entry> {
        None
    }

    fn entries(&self) -> Arc<[(Code, Entry)]> {
        <_>::default()
    }

    fn wildcard(&self) -> Option<&dyn Layer> {
        Some(self)
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
                let mut cur = editor.current_mut();

                match cur.buffer.contents {
                    BufferContents::Text(ref mut rope) => {
                        rope.insert_char(cur.view.cursor, ch);
                        cur.buffer.modified = true;
                        cur.jump_cursor(1, 0);
                    }
                }
            }
            Mode::Command => {
                if ch == '\n' {
                    editor.mode = Mode::Normal;

                    let command_name = editor.command.as_str().trim_start_matches(':');
                    // remove arguments
                    let command_name = command_name
                        .split_whitespace()
                        .next()
                        .unwrap_or(command_name);

                    let Some(act) = DEFAULT_ACTIONS.get(command_name) else {
                        editor.command.clear();
                        editor.command.push_str("invalid command");
                        return true;
                    };

                    act.act.run(editor);
                    editor.command.clear();
                    editor.command_suggestions.clear();
                    editor.command_suggestion_index = None;
                } else {
                    editor.command.push(ch);
                    RefreshSuggestions.run(editor);
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

    fn description(&self) -> &str {
        "quit without saving"
    }

    fn run(&self, editor: &mut Editor) {
        if editor.current().buffer.modified {
            editor.status_is_error = true;
            editor.status.clear();
            editor
                .status
                .push_str("unsaved changes, type :q! to quit without saving");
            return;
        }

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

    fn description(&self) -> &str {
        "force quit without saving"
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

    fn description(&self) -> &str {
        "save"
    }

    fn run(&self, editor: &mut Editor) {
        if let Err(err) = editor.current_mut().buffer.write() {
            editor.status_is_error = true;
            editor.status.clear();
            use std::fmt::Write;
            _ = write!(&mut editor.status, "{err}");
        }
    }
}

//

#[derive(Debug, Default)]
pub struct WriteQuit;

impl Action for WriteQuit {
    fn name(&self) -> &str {
        "x"
    }

    fn description(&self) -> &str {
        "save and quit"
    }

    fn run(&self, editor: &mut Editor) {
        if !editor.current().buffer.modified {
            editor.should_close = true;
            return;
        }

        if let Err(err) = editor.current_mut().buffer.write() {
            editor.status_is_error = true;
            editor.status.clear();
            use std::fmt::Write;
            _ = write!(&mut editor.status, "{err}");
            return;
        }

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

    fn description(&self) -> &str {
        "save and force quit"
    }

    fn run(&self, editor: &mut Editor) {
        if let Err(err) = editor.current_mut().buffer.write() {
            editor.status_is_error = true;
            editor.status.clear();
            use std::fmt::Write;
            _ = write!(&mut editor.status, "{err}");
            return;
        }

        editor.should_close = true;
    }
}

//

#[derive(Debug, Default)]
pub struct ClearLog;

impl Action for ClearLog {
    fn name(&self) -> &str {
        "clear-log"
    }

    fn description(&self) -> &str {
        "clear the log file"
    }

    fn run(&self, _: &mut Editor) {
        if let Some(log_file) = crate::LOG_FILE.get() {
            log_file.set_len(0).unwrap();
        }
    }
}

//

#[derive(Debug, Default)]
pub struct RefreshSuggestions;

impl Action for RefreshSuggestions {
    fn name(&self) -> &str {
        "refresh-suggestions"
    }

    fn run(&self, editor: &mut Editor) {
        editor.command_suggestions.clear();
        editor.command_suggestion_index = None;

        let cmd = editor
            .command
            .strip_prefix(":")
            .unwrap_or(editor.command.as_str());
        editor.command_suggestions.extend(
            DEFAULT_ACTIONS
                .iter()
                .filter(|act| act.act.name().contains(cmd))
                .cloned(),
        );
    }
}

//

#[derive(Debug, Default)]
pub struct NextSuggestion;

impl Action for NextSuggestion {
    fn name(&self) -> &str {
        "next-suggestion"
    }

    fn run(&self, editor: &mut Editor) {
        if editor.command_suggestions.is_empty() {
            return;
        }

        if let Some(index) = editor.command_suggestion_index.as_mut() {
            *index += 1;
            *index = (*index).min(editor.command_suggestions.len() - 1);
        };

        let index = *editor.command_suggestion_index.get_or_insert(0);

        editor.command.clear();
        editor.command.push(':');
        editor
            .command
            .push_str(editor.command_suggestions[index].act.name());
    }
}

//

#[derive(Debug, Default)]
pub struct PrevSuggestion;

impl Action for PrevSuggestion {
    fn name(&self) -> &str {
        "prev-suggestion"
    }

    fn run(&self, editor: &mut Editor) {
        tracing::debug!("running action {}", self.name());

        if editor.command_suggestions.is_empty() {
            return;
        }
        if let Some(index) = editor.command_suggestion_index.as_mut() {
            *index = (*index).saturating_sub(1);
        }

        let index = *editor.command_suggestion_index.get_or_insert(0);

        editor.command.clear();
        editor.command.push(':');
        editor
            .command
            .push_str(editor.command_suggestions[index].act.name());
    }
}

//

#[derive(Debug, Default)]
pub struct Open;

impl Action for Open {
    fn name(&self) -> &str {
        "open"
    }

    fn description(&self) -> &str {
        "open a file, takes a path argument"
    }

    fn run(&self, editor: &mut Editor) {
        let Some(path) = editor.command.split_whitespace().nth(1) else {
            tracing::error!("`open` is missing path argument");
            return;
        };

        editor.open(path.to_string().as_str()); // FIXME: lifetime error
    }
}

//

#[derive(Debug, Default)]
pub struct BufferClose;

impl Action for BufferClose {
    fn name(&self) -> &str {
        "buffer-close"
    }

    fn description(&self) -> &str {
        "close the current buffer"
    }

    fn run(&self, editor: &mut Editor) {
        if editor.current().buffer.modified {
            editor.status_is_error = true;
            editor.status.clear();
            editor.status.push_str("unsaved changes");
            return;
        }

        if editor.buffers.len() == 1 {
            editor.buffers.clear();
            editor.buffers.push(Buffer::new());
            editor.view.buffer_index = 0;
            return;
        }

        editor.buffers.pop();
        editor.view.buffer_index = editor.view.buffer_index.min(editor.buffers.len() - 1);
    }
}

//

#[derive(Debug, Default)]
pub struct BufferNext;

impl Action for BufferNext {
    fn name(&self) -> &str {
        "buffer-next"
    }

    fn description(&self) -> &str {
        "go to the next buffer"
    }

    fn run(&self, editor: &mut Editor) {
        editor.view.buffer_index += 1;
        editor.view.buffer_index %= editor.buffers.len();
    }
}

//

#[derive(Debug, Default)]
pub struct BufferPrev;

impl Action for BufferPrev {
    fn name(&self) -> &str {
        "buffer-prev"
    }

    fn description(&self) -> &str {
        "go to the previous buffer"
    }

    fn run(&self, editor: &mut Editor) {
        editor.view.buffer_index += 1;
        editor.view.buffer_index %= editor.buffers.len();
    }
}

//

#[derive(Debug, Default)]
pub struct FileExplorer;

impl Action for FileExplorer {
    fn name(&self) -> &str {
        "file-explorer"
    }

    fn run(&self, editor: &mut Editor) {
        let buf = editor.current().buffer;
        let (at, remote) = match &buf.inner {
            BufferInner::File { .. } => {
                let mut path = PathBuf::from(buf.name.to_string()).canonicalize().unwrap();
                path.pop();
                (path, None)
            }
            BufferInner::NewFile { inner } => {
                let mut path = inner.clone();
                path.pop();
                (path, None)
            }
            BufferInner::Remote { remote } => {
                let mut path = PathBuf::from(
                    buf.name
                        .rsplit_once(':')
                        .map(|(_, path)| path)
                        .unwrap_or(buf.name.as_ref())
                        .to_string(),
                );
                path.pop();
                (path, Some(remote.clone()))
            }
            BufferInner::Scratch { .. } => (env::current_dir().unwrap(), None),
        };

        match Popup::file_explorer(remote, at) {
            Ok(popup) => {
                editor.popup = popup;
            }
            Err(err) => {
                tracing::error!("failed to open file explorer: {err}");
            }
        }
    }
}

//

#[derive(Debug, Default)]
pub struct BufferPicker;

impl Action for BufferPicker {
    fn name(&self) -> &str {
        "buffer-picker"
    }

    fn run(&self, editor: &mut Editor) {
        editor.popup = Popup::buffer_picker(editor.view.buffer_index);
    }
}

//

#[derive(Debug, Default)]
pub struct WhichKey;

impl Action for WhichKey {
    fn name(&self) -> &str {
        "which-key"
    }

    fn run(&self, editor: &mut Editor) {
        editor.force_whichkey ^= true;
    }
}
