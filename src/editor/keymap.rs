use core::panic;
use std::{
    borrow::Borrow,
    cmp::Ordering,
    collections::{BTreeSet, HashMap, HashSet},
    hash::Hash,
    sync::{Arc, LazyLock},
    thread,
};

use arc_swap::ArcSwap;
use crossterm::event::{KeyCode, KeyModifiers};

use crate::mode::Mode;

use super::{
    actions::{self as act},
    Editor,
};

//

pub struct Keymap {
    inner: Arc<ArcSwap<KeymapInner>>,
}

impl Keymap {
    pub fn load() -> Self {
        let inner = Arc::new(ArcSwap::new(<_>::default()));

        let inner2 = inner.clone();
        thread::spawn(move || {
            // TODO: auto reload
            _ = inner2;
        });

        Self { inner }
    }

    pub fn normal(&self) -> Arc<dyn Layer> {
        self.inner.load().normal.clone()
    }

    pub fn insert(&self) -> Arc<dyn Layer> {
        self.inner.load().insert.clone()
    }

    pub fn command(&self) -> Arc<dyn Layer> {
        self.inner.load().command.clone()
    }
}

//

#[derive(Clone)]
pub enum Entry {
    Layer(Arc<dyn Layer>),
    Action(Arc<dyn Action>),
}

impl Entry {
    pub fn new_action(action: impl Action + 'static) -> Self {
        Self::Action(Arc::new(action) as _)
    }

    pub fn from_action_name(name: &str) -> Option<Self> {
        DEFAULT_ACTIONS
            .get(name)
            .map(|entry| Self::Action(entry.act.clone()))
    }

    pub fn new_layer(layer: impl Layer + 'static) -> Self {
        Self::Layer(Arc::new(layer) as _)
    }
}

impl From<Arc<dyn Layer>> for Entry {
    fn from(value: Arc<dyn Layer>) -> Self {
        Self::Layer(value)
    }
}

impl From<Arc<dyn Action>> for Entry {
    fn from(value: Arc<dyn Action>) -> Self {
        Self::Action(value)
    }
}

impl From<HashMap<Code, Entry>> for Entry {
    fn from(value: HashMap<Code, Entry>) -> Self {
        Self::Layer(Arc::new(value) as _)
    }
}

//

pub trait Action: Sync + Send {
    fn name(&self) -> &str;

    fn description(&self) -> &str {
        self.name()
    }

    fn run(&self, editor: &mut Editor);
}

pub trait ActionExt: Action {
    fn arc() -> Arc<dyn Action>;
}

impl<T: Action + Default + 'static> ActionExt for T {
    fn arc() -> Arc<dyn Action> {
        Arc::new(T::default()) as _
    }
}

//

pub trait Layer: Sync + Send {
    fn name(&self) -> &str;

    fn description(&self) -> &str {
        self.name()
    }

    fn get(&self, keycode: Code) -> Option<Entry>;

    fn run(&self, keycode: Code, editor: &mut Editor) -> bool {
        let Some(next) = self.get(keycode) else {
            return false;
        };

        match next {
            Entry::Layer(layer) => {
                editor.mode = Mode::Action {
                    layer,
                    prev: editor.mode.prev(),
                };
            }
            Entry::Action(action) => {
                action.run(editor);
                editor.mode = editor.mode.prev().mode();
            }
        };

        true
    }
}

pub trait LayerExt: Layer {
    fn arc() -> Arc<dyn Layer>;
}

impl<T: Layer + Default + 'static> LayerExt for T {
    fn arc() -> Arc<dyn Layer> {
        Arc::new(T::default()) as _
    }
}

impl Layer for HashMap<Code, Entry> {
    fn name(&self) -> &str {
        "layer"
    }

    fn get(&self, keycode: Code) -> Option<Entry> {
        self.get(&keycode).cloned()
    }
}

//

pub struct Global(HashMap<Code, Entry>);

impl Layer for Global {
    fn name(&self) -> &str {
        "global"
    }

    fn get(&self, keycode: Code) -> Option<Entry> {
        Layer::get(&self.0, keycode)
    }
}

//

pub struct Normal(HashMap<Code, Entry>);

impl Layer for Normal {
    fn name(&self) -> &str {
        "normal"
    }

    fn get(&self, keycode: Code) -> Option<Entry> {
        Layer::get(&self.0, keycode)
    }
}

//

pub struct Insert(HashMap<Code, Entry>);

impl Layer for Insert {
    fn name(&self) -> &str {
        "insert"
    }

    fn get(&self, keycode: Code) -> Option<Entry> {
        Layer::get(&self.0, keycode)
    }

    fn run(&self, keycode: Code, editor: &mut Editor) -> bool {
        let Some(next) = self.get(keycode) else {
            return act::TypeChar.run(keycode, editor);
        };

        match next {
            Entry::Layer(layer) => {
                editor.mode = Mode::Action {
                    layer,
                    prev: editor.mode.prev(),
                };
            }
            Entry::Action(action) => {
                action.run(editor);
                editor.mode = editor.mode.prev().mode();
            }
        };

        true
    }
}

//

pub struct Command(HashMap<Code, Entry>);

impl Layer for Command {
    fn name(&self) -> &str {
        "command"
    }

    fn get(&self, keycode: Code) -> Option<Entry> {
        Layer::get(&self.0, keycode)
    }

    fn run(&self, keycode: Code, editor: &mut Editor) -> bool {
        let Some(next) = self.get(keycode) else {
            tracing::debug!("running action {}", act::TypeChar.name());
            return act::TypeChar.run(keycode, editor);
        };

        match next {
            Entry::Layer(layer) => {
                editor.mode = Mode::Action {
                    layer,
                    prev: editor.mode.prev(),
                };
            }
            Entry::Action(action) => {
                tracing::debug!("running action {}", action.name());
                action.run(editor);
                editor.mode = editor.mode.prev().mode();
            }
        };

        true
    }
}

//

pub static DEFAULT_ACTIONS: LazyLock<BTreeSet<ActionEntry>> = LazyLock::new(|| {
    BTreeSet::from_iter(
        act::all_actions()
            .into_iter()
            .map(|act| ActionEntry { act }),
    )
});

#[derive(Clone)]
pub struct ActionEntry {
    pub act: Arc<dyn Action>,
}

impl PartialEq for ActionEntry {
    fn eq(&self, other: &Self) -> bool {
        self.act.name().eq(other.act.name())
    }
}

impl Eq for ActionEntry {}

impl Hash for ActionEntry {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.act.name().hash(state);
    }
}

impl Borrow<str> for ActionEntry {
    fn borrow(&self) -> &str {
        self.act.name()
    }
}

impl PartialOrd for ActionEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ActionEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        self.act.name().cmp(other.act.name())
    }
}

//

/* trait IterKeymap<'a> {
    fn keymap(self) -> impl Iterator<Item = (Code, Entry)> + 'a;
}

impl<'a, I: Iterator<Item = (&'a str, &'a str)> + 'a> IterKeymap<'a> for I {
    fn keymap(self) -> impl Iterator<Item = (Code, Entry)> + 'a {
        self.map(|(key, act)| {
            (
                Code::try_from_str(key)
                    .unwrap_or_else(|| panic!("cannot find default keycode: `{key}`")),
                Entry::from_action_name(act)
                    .unwrap_or_else(|| panic!("cannot find default action: `{act}` ")),
            )
        })
    }
} */

macro_rules! map {
    ($($key:literal: $act:expr,)*) => {{
        HashMap::from_iter([
            $((const { Code::from_str($key) }, Entry::from($act)),)*
        ])
    }};

    ($base:ident, $($key:literal: $act:expr,)*) => {{
        $base.extend([
            $((const { Code::from_str($key) }, Entry::from($act)),)*
        ]);
    }};
}

static DEFAULT_GLOBAL: LazyLock<HashMap<Code, Entry>> = LazyLock::new(|| {
    map! {
        "esc": act::Escape::arc(),
    }
});

static DEFAULT_NORMAL: LazyLock<Arc<dyn Layer>> = LazyLock::new(|| {
    let mut normal = DEFAULT_GLOBAL.clone();
    map! {
        normal,
        "left":      act::MoveLeft::arc(),
        "right":     act::MoveRight::arc(),
        "up":        act::MoveUp::arc(),
        "down":      act::MoveDown::arc(),
        "h":         act::MoveLeft::arc(),
        "l":         act::MoveRight::arc(),
        "k":         act::MoveUp::arc(),
        "j":         act::MoveDown::arc(),
        "pageup":    act::MovePageUp::arc(),
        "pagedown":  act::MovePageDown::arc(),
        "home":      act::MoveLineBeg::arc(),
        "end":       act::MoveLineEnd::arc(),
        "w":         act::NextWordBeg::arc(),
        "e":         act::NextWordEnd::arc(),
        "b":         act::PrevWordBeg::arc(),
        "i":         act::SwitchToInsert::arc(),
        "I":         act::SwitchToInsertLineBeg::arc(),
        "a":         act::SwitchToAppend::arc(),
        "A":         act::SwitchToAppendLineEnd::arc(),
        "a":         act::SwitchToAppend::arc(),
        ":":         act::SwitchToCommand::arc(),
        "o":         act::InsertLineBelow::arc(),
        "S-O":       act::InsertLineAbove::arc(),
        "f":         act::JumpForwardsTo::arc(),
        "t":         act::JumpForwardsUntil::arc(),
        "S-F":       act::JumpBackwardsTo::arc(),
        "S-T":       act::JumpBackwardsUntil::arc(),
        "d":         act::Delete::arc(),
        "g":         map! {
            "g":         act::MoveBufferBeg::arc(),
            "e":         act::MoveBufferEnd::arc(),
        },
    }
    Arc::new(Normal(normal)) as _
});

static DEFAULT_INSERT: LazyLock<Arc<dyn Layer>> = LazyLock::new(|| {
    let mut insert = DEFAULT_GLOBAL.clone();
    map! {
        insert,
        "left":      act::MoveLeft::arc(),
        "right":     act::MoveRight::arc(),
        "up":        act::MoveUp::arc(),
        "down":      act::MoveDown::arc(),
        "pageup":    act::MovePageUp::arc(),
        "pagedown":  act::MovePageDown::arc(),
        "home":      act::MoveLineBeg::arc(),
        "end":       act::MoveLineEnd::arc(),
        "backspace": act::Backspace::arc(),
    }
    Arc::new(Insert(insert)) as _
});

static DEFAULT_COMMAND: LazyLock<Arc<dyn Layer>> = LazyLock::new(|| {
    let mut command = DEFAULT_GLOBAL.clone();
    map! {
        command,
        "backspace": act::Backspace::arc(),
        "tab":       act::NextSuggestion::arc(),
        "S-tab":     act::PrevSuggestion::arc(),
    };
    Arc::new(Command(command)) as _
});

//

pub struct KeymapInner {
    normal: Arc<dyn Layer>,
    insert: Arc<dyn Layer>,
    command: Arc<dyn Layer>,
}

impl Default for KeymapInner {
    fn default() -> Self {
        Self {
            normal: DEFAULT_NORMAL.clone(),
            insert: DEFAULT_INSERT.clone(),
            command: DEFAULT_COMMAND.clone(),
        }
    }
}

//

#[derive(Debug, PartialEq, Eq, PartialOrd, Hash, Clone, Copy)]
pub struct Code {
    pub keycode: KeyCode,
    pub modifiers: KeyModifiers,
}

impl Code {
    pub const fn from_event(mut keycode: KeyCode, mut modifiers: KeyModifiers) -> Self {
        if matches!(keycode, KeyCode::BackTab) {
            keycode = KeyCode::Tab;
            modifiers =
                KeyModifiers::from_bits_truncate(modifiers.bits() | KeyModifiers::SHIFT.bits());
        }
        Self { keycode, modifiers }
    }

    pub const fn from_str(s: &str) -> Self {
        if let Some(some) = Self::try_from_str(s) {
            some
        } else {
            // const_format_args!("{s}");
            // panic!("failed to parse keycode: `{s}`");
            panic!("{}", s);
        }
    }

    pub const fn try_from_str(s: &str) -> Option<Self> {
        Self::try_from_bytes(s.as_bytes())
    }

    pub const fn from_bytes(b: &[u8]) -> Self {
        if let Some(some) = Self::try_from_bytes(b) {
            some
        } else {
            panic!("failed to parse keycode");
        }
    }

    pub const fn try_from_bytes(b: &[u8]) -> Option<Self> {
        let (key, mods) = match b {
            [b'C', b'-', b'A', b'-', b'S', b'-', c @ ..] => (
                c,
                KeyModifiers::from_bits_truncate(
                    KeyModifiers::CONTROL.bits()
                        | KeyModifiers::ALT.bits()
                        | KeyModifiers::SHIFT.bits(),
                ),
            ),
            [b'A', b'-', b'S', b'-', c @ ..] => (
                c,
                KeyModifiers::from_bits_truncate(
                    KeyModifiers::ALT.bits() | KeyModifiers::SHIFT.bits(),
                ),
            ),
            [b'C', b'-', b'S', b'-', c @ ..] => (
                c,
                KeyModifiers::from_bits_truncate(
                    KeyModifiers::CONTROL.bits() | KeyModifiers::SHIFT.bits(),
                ),
            ),
            [b'C', b'-', b'A', b'-', c @ ..] => (
                c,
                KeyModifiers::from_bits_truncate(
                    KeyModifiers::CONTROL.bits() | KeyModifiers::ALT.bits(),
                ),
            ),
            [b'C', b'-', c @ ..] => (c, KeyModifiers::CONTROL),
            [b'A', b'-', c @ ..] => (c, KeyModifiers::ALT),
            [b'S', b'-', c @ ..] => (c, KeyModifiers::SHIFT),
            c => (c, KeyModifiers::NONE),
        };

        let key = match key {
            [b'f', num @ ..] if !num.is_empty() => {
                let Ok(num) = std::str::from_utf8(num) else {
                    return None;
                };

                let Ok(num) = u8::from_str_radix(num, 10) else {
                    return None;
                };

                if num > 24 || num == 0 {
                    return None;
                }

                KeyCode::F(num)
            }
            b"esc" => KeyCode::Esc,
            b"space" => KeyCode::Char(' '),
            b"backspace" => KeyCode::Backspace,
            b"left" => KeyCode::Left,
            b"right" => KeyCode::Right,
            b"up" => KeyCode::Up,
            b"down" => KeyCode::Down,
            b"pageup" => KeyCode::PageUp,
            b"pagedown" => KeyCode::PageDown,
            b"home" => KeyCode::Home,
            b"end" => KeyCode::End,
            b"tab" => KeyCode::Tab,
            [c] => KeyCode::Char(*c as char),
            _ => return None,
        };

        Some(Self {
            keycode: key,
            modifiers: mods,
        })
    }
}
