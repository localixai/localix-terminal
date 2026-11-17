//! What a view tells a terminal, as plain data: a key by its place on the keyboard (the W3C
//! `code` names) with its modifiers and the side each was pressed on, a pointer event in the
//! view's pixels with the geometry to read it by, and the steps of a find.

/// Modifiers held, and for each the side it was pressed on.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Mods(pub u16);

impl Mods {
    pub const SHIFT: u16 = 1 << 0;
    pub const CTRL: u16 = 1 << 1;
    pub const ALT: u16 = 1 << 2;
    /// Command on a Mac.
    pub const SUPER: u16 = 1 << 3;
    pub const CAPS_LOCK: u16 = 1 << 4;
    pub const NUM_LOCK: u16 = 1 << 5;
    /// The right one of each, when its modifier is held.
    pub const SHIFT_RIGHT: u16 = 1 << 6;
    pub const CTRL_RIGHT: u16 = 1 << 7;
    pub const ALT_RIGHT: u16 = 1 << 8;
    pub const SUPER_RIGHT: u16 = 1 << 9;

    /// Whether every bit of `mods` is held.
    #[must_use]
    pub const fn has(self, mods: u16) -> bool {
        self.0 & mods == mods
    }
}

/// A key by its place on the keyboard, whatever the layout prints on it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum KeyCode {
    #[default]
    Unidentified,
    /// `a` to `z`.
    Letter(u8),
    /// `0` to `9` of the main block.
    Digit(u8),
    /// F1 to F25.
    F(u8),
    /// `0` to `9` of the keypad.
    Numpad(u8),
    Backquote,
    Backslash,
    BracketLeft,
    BracketRight,
    Comma,
    Equal,
    Minus,
    Period,
    Quote,
    Semicolon,
    Slash,
    IntlBackslash,
    Backspace,
    Tab,
    Enter,
    Space,
    Escape,
    Delete,
    Insert,
    Home,
    End,
    PageUp,
    PageDown,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    NumpadAdd,
    NumpadSubtract,
    NumpadMultiply,
    NumpadDivide,
    NumpadDecimal,
    NumpadEnter,
    NumpadEqual,
    CapsLock,
    ShiftLeft,
    ShiftRight,
    ControlLeft,
    ControlRight,
    AltLeft,
    AltRight,
    MetaLeft,
    MetaRight,
    ContextMenu,
}

/// Which Option key is Alt (Meta) for a terminal: with it, Option and a key send ESC and the
/// key; the other types what the layout prints with Option.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum OptionMeta {
    #[default]
    Off,
    Left,
    Right,
    Both,
}

impl OptionMeta {
    /// Whether the Option (Alt) held in `mods` is the one that is Meta: the right one when its
    /// side's bit says so, else the left.
    #[must_use]
    pub const fn is_meta(self, mods: Mods) -> bool {
        match self {
            Self::Off => false,
            Self::Both => true,
            Self::Left => !mods.has(Mods::ALT_RIGHT),
            Self::Right => mods.has(Mods::ALT_RIGHT),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum KeyAction {
    #[default]
    Press,
    Repeat,
    Release,
}

/// A key event: which key, what was done with it, the modifiers held (and those the layout used
/// to make its text), the text it makes, and the character the key makes with no modifier.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Key {
    pub code: KeyCode,
    pub action: KeyAction,
    pub mods: Mods,
    pub consumed: Mods,
    pub text: String,
    pub unshifted: Option<char>,
    /// An input method is composing with it.
    pub composing: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Button {
    Left,
    Right,
    Middle,
    /// The wheel's four directions, as programs are told them: up, down, left, right.
    WheelUp,
    WheelDown,
    WheelLeft,
    WheelRight,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum MouseAction {
    #[default]
    Press,
    Release,
    Motion,
}

/// A pointer event: where in the view (pixels from its top left), which button, what was done,
/// the modifiers held, and when (nanoseconds of a monotonic clock, which tells a double click).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Mouse {
    pub x: f32,
    pub y: f32,
    pub button: Option<Button>,
    pub action: MouseAction,
    pub mods: Mods,
    pub time_ns: u64,
}

/// The view's size and its cells' in pixels, and the gutter around the grid.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Geometry {
    pub width: u32,
    pub height: u32,
    pub cell_width: u32,
    pub cell_height: u32,
    pub pad_top: u32,
    pub pad_bottom: u32,
    pub pad_left: u32,
    pub pad_right: u32,
}

/// Where a pointer event went: to the program, to a selection (which wants the view to scroll
/// on while it is dragged past an edge), or nowhere.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Routed {
    Program,
    Selection { scroll: Option<Edge> },
    Nothing,
}

/// The edge a selection is dragged past.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Edge {
    Top,
    Bottom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScrollTo {
    Top,
    Bottom,
    /// The row of history at the top of the view, from 0.
    Row(u64),
}

/// A step of a find.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FindStep {
    /// Look for this (nothing ends the find).
    Needle(String),
    /// The next match up, toward older output; and back down.
    Next,
    Previous,
    /// The same needle counted again over the output as it is now (it grew while the row was
    /// open): the match shown stays, nothing scrolls.
    Again,
    /// The find row closed.
    Close,
}

/// Where a find is: the match it is at (from 1, counted from the top; 0 for none) and how many
/// there are.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Count {
    pub at: usize,
    pub total: usize,
}
