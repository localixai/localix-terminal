//! The terminal's native view: a frame of the terminal (`surface`) drawn by the window itself. What
//! every platform shares is here: the look (`theme.rs`), the frame laid out in runs and grounds
//! (`layout.rs`), the box characters as lines (`boxes.rs`), the cursor, the line under a program
//! that ended and the scrollbar (`overlay.rs`), the strokes of a row in pixels (`draw.rs`), the
//! rows kept drawn (`rows.rs`, `painter.rs`), what the person's acts mean (`acts.rs`) and what a
//! screen reader is told (`spoken.rs`); each platform has its canvas and its view in the window
//! (macOS: Core Text into Core Graphics bitmaps, an NSView of row layers; Linux: Pango into Cairo
//! surfaces, a GTK drawing area). Elsewhere a terminal's tab holds a stand-in that shows nothing
//! (`blank.rs`).

mod acts;
mod boxes;
mod draw;
mod layout;
mod overlay;
mod rows;
mod spoken;
mod theme;

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
mod blank;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(any(target_os = "macos", target_os = "linux"))]
mod painter;

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub(crate) use blank::TermView;
#[cfg(target_os = "linux")]
pub(crate) use linux::TermView;
#[cfg(target_os = "macos")]
pub(crate) use macos::{Owned, TermView};
#[cfg(any(target_os = "macos", target_os = "linux"))]
pub use painter::{TermPainter, term_picture};
pub use rows::Painted;

use crate::surface::{Palette, Rgb};

/// How a terminal is drawn: dark or light, the code font (a CSS stack of names, as Settings
/// keeps it) and its size in points, the screen's pixels to a point, whether ligatures join,
/// whether the pane has the keys in the key window (`active`) and the window is the key one
/// (`key`), the system's accent, and the person's Accessibility display options: more contrast,
/// and less transparency (`solid`: the terminal's own ground drawn under it, not the glass).
#[expect(
    clippy::struct_excessive_bools,
    reason = "each the system's or the person's own state, as the view is told them"
)]
#[derive(Debug, Clone, PartialEq)]
pub struct TermLook {
    pub dark: bool,
    pub font: String,
    pub size: f64,
    pub scale: f64,
    pub ligatures: bool,
    pub active: bool,
    pub key: bool,
    pub accent: Rgb,
    pub more: bool,
    pub solid: bool,
}

impl TermLook {
    /// The look of a terminal by default: the system's monospace at 13 points, ligatures on.
    #[must_use]
    pub fn new(dark: bool, scale: f64) -> Self {
        Self {
            dark,
            font: "ui-monospace,SFMono-Regular,Menlo,monospace".into(),
            size: 13.0,
            scale,
            ligatures: true,
            active: true,
            key: true,
            accent: theme::ACCENT,
            more: false,
            solid: false,
        }
    }

    /// The colours a terminal drawn in this look is given.
    #[must_use]
    pub fn palette(&self) -> Palette {
        self.theme().palette()
    }

    fn theme(&self) -> theme::Theme {
        theme::Theme {
            active: self.active,
            key: self.key,
            accent: self.accent,
            more: self.more,
            ..theme::Theme::new(self.dark)
        }
    }
}

/// What a check reads of a view: the grid, the rows it drew (top to bottom), the cursor, whether
/// it has the keys, is shown and drawn as the one in use, how many row pictures it holds, an
/// input method's text, and how many frames it drew.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "what a check reads: each a fact of its own about the view"
)]
pub struct TermSeen {
    /// The link lit under the pointer, and how many matches of a find are lit in view.
    pub link: Option<String>,
    pub found: usize,
    pub columns: u16,
    pub lines: u16,
    pub rows: Vec<String>,
    pub cursor: Option<(u16, u16)>,
    pub keys: bool,
    pub shown: bool,
    pub active: bool,
    pub layers: usize,
    pub marked: String,
    pub drawn: u64,
    pub alternate: bool,
    /// The cursor's shape, what is selected (as Copy takes it), the history's rows and the
    /// first of them in view, and the bytes the view's pictures of rows take.
    pub shape: crate::surface::Shape,
    pub selected: Option<String>,
    pub history: u64,
    pub offset: u64,
    pub bytes: usize,
    /// The view is drawn dark; the cursor blinks; the line under a program that ended.
    pub dark: bool,
    pub blinking: bool,
    pub ended: Option<String>,
    /// The system's secure input is on for it (macOS).
    pub secure: bool,
    /// The grids its terminal (and its program) was told; the frames drawn that left a row of
    /// theirs with no picture; the layers moving by an animation now (but the cursor's
    /// blinking); and whether nothing it draws reaches past its bounds.
    pub resized: u64,
    pub blank: u64,
    pub moving: usize,
    pub clips: bool,
    /// How far its rows are raised, in points, to keep the cursor's row in a pane grown
    /// shorter than its grid while the person moves the pane's edge.
    pub lifted: u32,
    /// Where it lies in its window, in points from the top left of the window's content:
    /// across, down, wide and tall (macOS).
    pub rect: [i32; 4],
}

/// For checks: what a check does to a terminal's view, or reads of it, as the system would.
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TermProbe {
    /// What it drew last.
    Seen,
    /// Its layers drawn into a picture.
    Pixels,
    /// A key pressed and let go: its virtual key code, the modifier flags with their sides'
    /// bits, the characters it types and the characters with no modifier.
    Key {
        code: u16,
        flags: usize,
        chars: String,
        plain: String,
    },
    /// An input method composing `text`, or committing it.
    Marked(String),
    Insert(String),
    /// Which Option key is Meta, as the person's settings would make it (they are not written).
    Meta(crate::surface::OptionMeta),
    /// The pointer over the middle of the cell at `column` of row `row` in view, with the
    /// modifier `flags` held: moved there, pressed, dragged there, let go, or only the
    /// modifiers changed. Presses close in time are one double or triple click.
    Pointer {
        pointing: Pointing,
        column: u16,
        row: u16,
        flags: usize,
    },
    /// The pointer in the scrollbar's column, `percent` of the way down the view.
    Bar {
        pointing: Pointing,
        percent: u8,
    },
    /// The wheel turned by whole rows: up (more than 0) or down.
    Wheel(i32),
    /// Copy, Paste or Select All, as the Edit menu sends them.
    Edit(Edit),
    /// The view given the keys (`true`) or its window's keys given to nothing.
    Focus(bool),
    /// The system's display options as the person would set them (they are not changed):
    /// more contrast, less transparency.
    Display {
        more: bool,
        solid: bool,
    },
    /// VoiceOver as the view answers it, a screen reader running from now on.
    Reader,
    /// A change of the settings, as the Settings page makes it, told to every window (a test
    /// build keeps its settings in its own home).
    Settings(serde_json::Value),
    /// The terminal views of the window in use, counted (whatever terminal the probe names).
    Views,
    /// The window the terminal is in made this wide and tall (its content, in points), as a
    /// person's drag of its edge makes it; the size it had is answered.
    Window(u32, u32),
    /// The window's edge taken hold of (`true`) or let go, as the system tells each of its
    /// terminal views as a person's drag of it starts and ends.
    Edge(bool),
    /// The panes of the window as a carried tab's landing moves them (`TermAnswer::Carry`).
    Carry,
    /// A key pressed and let go through the app's queue, to what holds the window's keys
    /// (macOS): its virtual key code and the characters it types.
    KeyIn(u16, String),
    /// The window closed as its close button closes it (macOS).
    Close,
    /// AppKit's message that the window's live resize started, told to each of its terminal
    /// views, with the window in none: a message no end follows.
    EdgeTold,
    /// A person's left button at a place of the window (points from its content's top left),
    /// pressed, dragged there or let go, through the app's queue of events (macOS).
    Press(Pointing, i32, i32),
}

/// For checks: what the Edit menu sends a terminal's view.
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edit {
    Copy,
    Paste,
    SelectAll,
}

/// For checks: what VoiceOver reads of a terminal's view: its role and name, its text, the
/// line the insertion point is on and that line's text, and what it last read out.
#[doc(hidden)]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Reader {
    pub role: String,
    pub label: String,
    pub value: String,
    pub characters: usize,
    pub line: usize,
    pub line_text: String,
    pub said: String,
}

/// For checks: what the pointer does in a `TermProbe::Pointer`.
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pointing {
    Moved,
    Down,
    Dragged,
    Up,
    Flags,
}

/// For checks: what a probe found.
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TermAnswer {
    Seen(TermSeen),
    Pixels(Picture),
    Reader(Reader),
    Views(TermViews),
    /// The size the window's content had before a `TermProbe::Window`, in points.
    Window(u32, u32),
    /// The window's panes while a tab is carried over them.
    Carry(CarrySeen),
    Done,
}

/// For checks: the terminal views of a window: how many its tabs hold (open, closed by the
/// person, or ended with their last screen kept), how many are in its hierarchy, and how many
/// are alive in the process, each with its row pictures. A view let go of is in neither.
#[doc(hidden)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TermViews {
    pub held: usize,
    pub placed: usize,
    pub alive: usize,
}

/// For checks: a window's panes while a tab is carried over them (`browser/deck_preview.rs`),
/// and what carrying cost the main thread.
#[doc(hidden)]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CarrySeen {
    /// The carried tab standing where it would land now (its key), none while none does.
    pub standing: Option<String>,
    /// Each pane in sight, the carried tab's among them while it stands there.
    pub panes: Vec<PaneSeen>,
    /// How many times the window was laid out for a landing, or back from one.
    pub laid: u32,
    /// The pointer aimed over the panes: how many times, the longest and all of it, in
    /// microseconds of the main thread's work.
    pub aims: u32,
    pub aim_most: u32,
    pub aim_all: u64,
}

/// For checks: one pane of a window, by its own view (macOS).
#[doc(hidden)]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PaneSeen {
    /// Its key (`t2`).
    pub key: String,
    /// Where its view stands and where it is drawn now, in points from the window's content's
    /// top left: across, down, wide and tall; where the top left of what it holds is drawn,
    /// and how far up from there what it holds is drawn raised now.
    pub rect: [i32; 4],
    pub now: [i32; 4],
    pub holds: [i32; 2],
    pub raised: i32,
    /// How it is drawn now, in hundredths (100: not clear at all), how many springs and fades
    /// of a layout it still plays, and whether its view is in sight.
    pub opacity: u8,
    pub playing: usize,
    pub seen: bool,
}

/// How many terminal views are alive in the process: one `Alive` in each view's state, counted
/// as it is made and as it is freed, for checks (`TermViews`).
static ALIVE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// A view's place in the count of views alive.
#[cfg(any(target_os = "macos", target_os = "linux"))]
struct Alive(());

#[cfg(any(target_os = "macos", target_os = "linux"))]
impl Alive {
    fn new() -> Self {
        ALIVE.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Self(())
    }
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
impl Drop for Alive {
    fn drop(&mut self) {
        ALIVE.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
    }
}

/// How many terminal views are alive in the process.
pub(super) fn alive() -> usize {
    ALIVE.load(std::sync::atomic::Ordering::SeqCst)
}

/// A picture of a terminal: `width` by `height` pixels, each red, green, blue and alpha
/// (premultiplied), a row after another.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picture {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
}
