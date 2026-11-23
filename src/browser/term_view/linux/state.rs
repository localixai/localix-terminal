//! What the terminal's view keeps between GTK's calls: the terminal it shows and how it draws
//! it, the last frame and the pictures over its rows, and what the person is doing in it now
//! (the keys and their sides, an input method's text, the wheel, a selection dragged past an
//! edge, the link pointed at, the scrollbar, the cursor's blinking).

use std::sync::Arc;

use super::super::spoken::Lines;
use super::{Placed, TermPainter};
use crate::surface::{CellPixels, Edge, Frame, Link, Mouse, OptionMeta, Surface};

/// The pictures drawn over the rows: the cursor, an input method's text, the line under a
/// program that ended, and the scrollbar's slider.
#[derive(Default)]
pub(super) struct Over {
    pub(super) cursor: Option<Placed>,
    pub(super) marked: Option<Placed>,
    pub(super) ended: Option<Placed>,
    pub(super) bar: Option<Placed>,
}

/// What the view keeps: the terminal it shows, how it draws it, and what the person is doing in
/// it now.
#[expect(
    clippy::struct_excessive_bools,
    reason = "the view's states, each the system's or the person's own: shown, hushed, in use"
)]
pub(crate) struct State {
    pub(super) surface: Arc<dyn Surface>,
    pub(super) painter: Option<TermPainter>,
    /// The last frame, its rows kept from one to the next as the terminal fills them, and the
    /// text of each.
    pub(super) frame: Frame,
    pub(super) lines: Lines,
    pub(super) over: Over,
    /// The grid the terminal was told, its cell's size in pixels, and the screen it was fitted
    /// on.
    pub(super) grid: (u16, u16),
    pub(super) cell: CellPixels,
    pub(super) fitted_alternate: bool,
    pub(super) shown: bool,
    /// Under the window's dialog: no click, no key, no pointer.
    pub(super) hushed: bool,
    /// The view has the keys, in the key window; and its window is the key one.
    pub(super) active: bool,
    pub(super) key: bool,
    /// An input method's text as it composes, and what it committed while a key is read (none
    /// outside one).
    pub(super) marked: String,
    pub(super) committed: Option<String>,
    /// The right modifier keys down now (`Mods`' right bits), and the key last pressed and not
    /// let go (a press of it again is the key repeating).
    pub(super) sides: u16,
    pub(super) down: Option<u16>,
    /// The wheel's turn not yet a whole row.
    pub(super) wheel: f64,
    /// A selection dragged past an edge, the frame clock's callback that scrolls it on
    /// meanwhile, and where the pointer was last.
    pub(super) dragging: Option<(Edge, gtk::TickCallbackId, Mouse)>,
    /// Which Option (Alt) key is Meta, as the person's settings say.
    pub(super) meta: OptionMeta,
    /// How many frames were drawn and how many grids the terminal was told, for checks.
    pub(super) drawn: u64,
    pub(super) resized: u64,
    /// Where the pointer was last over the view (its points), and the link under it while
    /// Super is held.
    pub(super) pointer: Option<(f64, f64)>,
    pub(super) link: Option<Link>,
    /// What opens a link the person Super-clicks.
    pub(super) opens: Box<dyn Fn(&str)>,
    /// The scrollbar: shown while the pointer is over the view, the pointer on its slider, and
    /// where on the slider it was taken while it is dragged (points from its top).
    pub(super) bar_shown: bool,
    pub(super) bar_pointed: bool,
    pub(super) bar_held: Option<f64>,
    /// The cursor's blinking: where it began (from shown again where it moves), whether it is
    /// shown in this half of its period, and which run of turns is the current one.
    pub(super) blink: Option<[f64; 4]>,
    pub(super) blink_shown: bool,
    pub(super) blink_turn: u64,
    /// The timeout of the cursor's next turn while it blinks, taken off as the view closes.
    pub(super) blink_source: Option<gtk::glib::SourceId>,
    /// The window the view hears become active, and its handler, let go of as the view moves
    /// to another window or closes.
    pub(super) watching: Option<(
        gtk::glib::WeakRef<gtk::ApplicationWindow>,
        gtk::glib::SignalHandlerId,
    )>,
    /// The display options as a check sets them (more contrast, less transparency); none: as
    /// the system says.
    pub(super) display: Option<(bool, bool)>,
    /// The view's place in the count of views alive, for checks: freed with the view.
    _alive: super::super::Alive,
}

impl State {
    /// The state of a new view of `surface`, opening links with `opens`.
    pub(super) fn new(surface: Arc<dyn Surface>, opens: Box<dyn Fn(&str)>) -> Self {
        Self {
            surface,
            painter: None,
            frame: Frame::default(),
            lines: Lines::default(),
            over: Over::default(),
            grid: (0, 0),
            cell: CellPixels::default(),
            fitted_alternate: false,
            shown: false,
            hushed: false,
            active: false,
            key: false,
            marked: String::new(),
            committed: None,
            sides: 0,
            down: None,
            wheel: 0.0,
            dragging: None,
            meta: OptionMeta::Off,
            drawn: 0,
            resized: 0,
            pointer: None,
            link: None,
            opens,
            bar_shown: false,
            bar_pointed: false,
            bar_held: None,
            blink: None,
            blink_shown: true,
            blink_turn: 0,
            blink_source: None,
            watching: None,
            display: None,
            _alive: super::super::Alive::new(),
        }
    }
}
