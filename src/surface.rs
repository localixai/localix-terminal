//! What a terminal shows and what it is told, as plain data: a frame of rows of cells with their
//! styles and colours, the cursor, the selection and the matches of a find, and the events a view
//! sends back (`surface/input.rs`). The window draws a terminal from this alone and the terminal
//! implements it ([`Surface`]), so neither knows the other: the core's own types never leave
//! `terminal/core*`, and the window holds no terminal.

mod input;

pub use input::{
    Button, Count, Edge, FindStep, Geometry, Key, KeyAction, KeyCode, Mods, Mouse, MouseAction,
    OptionMeta, Routed, ScrollTo,
};

/// A colour, 8 bits a channel.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    /// The colour written `0xrrggbb`.
    #[must_use]
    pub const fn hex(value: u32) -> Self {
        let [_, r, g, b] = value.to_be_bytes();
        Self { r, g, b }
    }
}

/// A cell's text or underline colour as the program set it: the terminal's own, an entry of the
/// palette (so a bold one can be drawn bright), or a colour of its own.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Color {
    #[default]
    Default,
    Palette(u8),
    Rgb(Rgb),
}

/// How a cell is underlined (SGR 4 and its kinds).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Underline {
    #[default]
    None,
    Single,
    Double,
    Curly,
    Dotted,
    Dashed,
}

/// A cell's on/off attributes, a bit each.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Flags(pub u8);

impl Flags {
    pub const BOLD: u8 = 1 << 0;
    pub const ITALIC: u8 = 1 << 1;
    pub const FAINT: u8 = 1 << 2;
    pub const BLINK: u8 = 1 << 3;
    pub const INVERSE: u8 = 1 << 4;
    pub const INVISIBLE: u8 = 1 << 5;
    pub const STRIKE: u8 = 1 << 6;
    pub const OVERLINE: u8 = 1 << 7;

    /// Whether `flag` is set.
    #[must_use]
    pub const fn has(self, flag: u8) -> bool {
        self.0 & flag != 0
    }
}

/// How a cell is drawn: its text colour, its ground (none: the terminal's, which is clear), its
/// attributes and its underline.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Style {
    pub fg: Color,
    pub bg: Option<Rgb>,
    pub flags: Flags,
    pub underline: Underline,
    pub underline_color: Color,
}

/// How wide a cell's character is: one column, the first of two, the second of two (drawn by the
/// first), or the empty last column of a row whose wide character went on to the next.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Wide {
    #[default]
    Narrow,
    Wide,
    Tail,
    Head,
}

/// One cell: where its text is in its row's `text`, how wide it is, and its style.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Cell {
    pub from: u32,
    pub len: u16,
    pub wide: Wide,
    pub style: Style,
}

/// A row's id: it stays with the row as the view scrolls and is never used again, so what was
/// drawn of a row can be kept by it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RowId(pub u128);

/// One row in view. A clean row of a partial frame says only its id and place; a dirty one, and
/// every row of a full frame, has its cells too.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct Row {
    pub id: RowId,
    /// Its place in view, from 0 at the top.
    pub y: u16,
    pub dirty: bool,
    /// It goes on to the next row (the line wrapped there).
    pub wrapped: bool,
    /// Every cell's text, one after the other.
    pub text: String,
    pub cells: Vec<Cell>,
    /// The columns selected, first and last, both in.
    pub selected: Option<(u16, u16)>,
}

impl Row {
    /// What `cell` shows: nothing for a blank one.
    #[must_use]
    pub fn text_of(&self, cell: &Cell) -> &str {
        let from = cell.from as usize;
        self.text
            .get(from..from + usize::from(cell.len))
            .unwrap_or("")
    }
}

/// The cursor's look, as the program asked for it (DECSCUSR).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Shape {
    #[default]
    Block,
    Bar,
    Underline,
    Hollow,
}

/// Where the cursor is in view (none when it is out of view or hidden), and how it looks.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Cursor {
    /// Column and row, from 0.
    pub at: Option<(u16, u16)>,
    pub shape: Shape,
    pub blinking: bool,
    /// It stands on the second half of a wide character.
    pub wide_tail: bool,
}

/// The terminal's colours as the frame is drawn with them: the theme it was given, and what a
/// program set over it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Colors {
    pub foreground: Rgb,
    pub background: Rgb,
    /// The cursor's own colour, when one was set.
    pub cursor: Option<Rgb>,
    pub palette: [Rgb; 256],
}

impl Default for Colors {
    fn default() -> Self {
        Self {
            foreground: Rgb::default(),
            background: Rgb::default(),
            cursor: None,
            palette: [Rgb::default(); 256],
        }
    }
}

/// The colours a terminal is given (the window's theme): its text, ground and cursor, and the
/// sixteen colours programs name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    pub foreground: Rgb,
    pub background: Rgb,
    pub cursor: Rgb,
    pub ansi: [Rgb; 16],
}

/// How much there is to scroll: all rows, the first in view, and how many are in view.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Scrollbar {
    pub total: u64,
    pub offset: u64,
    pub len: u64,
}

/// Columns `from` to `to` (both in) of the row at `y` in view: a match of a find.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Span {
    pub y: u16,
    pub from: u16,
    pub to: u16,
}

/// What a cell links to, and every stretch of the rows in view the link covers: an address a
/// program gave its text (OSC 8), or one written in the line it is part of.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct Link {
    pub uri: String,
    pub spans: Vec<Span>,
}

/// What a view draws: the grid's size, whether every row is new, the rows in view, and what lies
/// over them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Frame {
    pub columns: u16,
    pub lines: u16,
    /// Every row has its cells: draw them all.
    pub full: bool,
    pub rows: Vec<Row>,
    pub cursor: Cursor,
    pub colors: Colors,
    pub scroll: Scrollbar,
    /// A program draws on the other screen (a full-screen one).
    pub alternate: bool,
    /// The program takes the mouse: a click is its, unless Option is held.
    pub mouse_taken: bool,
    /// The matches of a find in view, and the one it is at.
    pub matches: Vec<Span>,
    pub current: Option<Span>,
    /// The link the pointer is on with Command held, underlined: the view's own, which the
    /// terminal never fills.
    pub link: Vec<Span>,
    /// How the program ended, once it has ("(the program exited with 0)"): the view says so
    /// under what it left.
    pub ended: Option<String>,
}

/// A cell's size in the screen's pixels, as the view draws it: what a program that asks the
/// terminal's size in pixels is told. Zero where no view has drawn the terminal.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct CellPixels {
    pub width: u32,
    pub height: u32,
}

/// A terminal as a view draws it and acts on it. Every call is short: what takes the terminal's
/// lock holds it only to read or tell the core.
pub trait Surface: Send + Sync {
    /// Fills `into` with what changed since the last frame (every row when `full`); `false`
    /// when there is nothing new to draw, as while a program holds its frame back.
    fn frame(&self, into: &mut Frame, full: bool) -> bool;
    /// The view's grid is `columns` by `lines`, each cell `cell` in pixels.
    fn resize(&self, columns: u16, lines: u16, cell: CellPixels);
    /// The view went out of sight: what it drew from is let go of, and the next frame is full.
    fn hidden(&self);
    /// The theme the terminal is drawn in: what a program asks of its colours is answered from it.
    fn colors(&self, palette: &Palette);
    /// Which Option key is Alt from now on.
    fn option_meta(&self, meta: OptionMeta);
    /// A key pressed: what it sends the program; `false` when it sends nothing.
    fn key(&self, key: &Key) -> bool;
    /// Text the person typed whole (an input method's, a character the layout made).
    fn text(&self, committed: &str);
    /// A pointer event in the view: to the program when it takes the mouse, else a selection.
    fn mouse(&self, mouse: &Mouse, geometry: &Geometry) -> Routed;
    /// The wheel turned by `lines` (up toward history is more than 0) where the pointer is.
    fn wheel(&self, lines: i32, mouse: &Mouse, geometry: &Geometry);
    fn scroll_to(&self, to: ScrollTo);
    /// The view took the keys or let them go.
    fn focus(&self, on: bool);
    fn paste(&self, text: &str);
    /// The selection's text, its wrapped lines joined.
    fn copy(&self) -> Option<String>;
    fn select_all(&self);
    /// History and screen cleared, the shell asked to draw its prompt again: only on the main
    /// screen, since a full-screen program draws all of its own.
    fn clear(&self);
    /// A step of a find: where it is and how many it found.
    fn find(&self, step: &FindStep) -> Count;
    /// What the cell at `column` of the row `y` in view links to.
    fn link_at(&self, column: u16, y: u16) -> Option<Link>;
    /// What the program asked to put on the pasteboard since this was last asked (OSC 52),
    /// plain text of at most a mebibyte: the window decides whether it goes there.
    fn copied(&self) -> Option<String>;
    /// Whether the program rang the bell since this was last asked.
    fn rang(&self) -> bool;
    /// The screen as text, for a reader.
    fn reader_text(&self) -> String;
    /// The program reads what it does not show (a password).
    fn hides_input(&self) -> bool;
}
