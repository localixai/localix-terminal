//! What the terminal's view keeps between the system's calls: the terminal it shows and how it
//! draws it, the last frame, and what the person is doing in it now (the keys, an input method's
//! text, the wheel, a selection dragged past an edge, the link pointed at, the scrollbar).

use std::sync::Arc;

use objc2::rc::Retained;
use objc2::runtime::AnyObject;

use super::super::spoken::{Lines, Spoken};
use super::TermPainter;
use super::layers::Layers;
use crate::surface::{Edge, Frame, Link, Mouse, OptionMeta, Surface};

/// What the view keeps: the terminal it shows, how it draws it, and what the person is doing in
/// it now.
#[expect(
    clippy::struct_excessive_bools,
    reason = "the view's states, each the system's or the person's own: shown, hushed, in use"
)]
pub(crate) struct State {
    pub(super) surface: Arc<dyn Surface>,
    pub(super) painter: Option<TermPainter>,
    pub(super) layers: Layers,
    /// The last frame, its rows kept from one to the next as the terminal fills them.
    pub(super) frame: Frame,
    /// The grid the terminal was told, and the screen it was fitted on (the other screen has
    /// no gutter).
    pub(super) grid: (u16, u16),
    pub(super) fitted_alternate: bool,
    pub(super) shown: bool,
    /// It was shown once: out of sight since, it keeps the grid it had in sight.
    pub(super) was_shown: bool,
    /// The person holds a divider of its split (`divider`, as the window's layout says) or its
    /// window's edge (`edge`: as a check says; none, as AppKit says): the grid it had is kept
    /// until they let go, its rows raised by `lift` points meanwhile, as far as keeps the
    /// cursor's row in the pane.
    pub(super) divider: bool,
    pub(super) edge: Option<bool>,
    pub(super) lift: f64,
    /// Under the window's dialog: no click, no key, no pointer.
    pub(super) hushed: bool,
    /// The view has the keys, in the key window; and its window is the key one.
    pub(super) active: bool,
    pub(super) key: bool,
    /// An input method's text as it composes.
    pub(super) marked: String,
    /// What the input method committed while a key is read, none outside one.
    pub(super) committed: Option<String>,
    /// The wheel's turn not yet a whole row.
    pub(super) wheel: f64,
    /// A selection dragged past an edge, the display link that scrolls it on meanwhile, and
    /// where the pointer was last.
    pub(super) dragging: Option<(Edge, Retained<AnyObject>, Mouse)>,
    /// Which Option key is Meta, as the person's settings say.
    pub(super) meta: OptionMeta,
    /// The system's secure input was turned on for a program that reads a password.
    pub(super) secure: bool,
    /// How many frames were drawn, how many left a row of theirs with no picture, and how many
    /// grids the terminal was told, for checks.
    pub(super) drawn: u64,
    pub(super) blank: u64,
    pub(super) resized: u64,
    /// Where the pointer was last over the view (its points), and the link under it while
    /// Command is held.
    pub(super) pointer: Option<(f64, f64)>,
    pub(super) link: Option<Link>,
    /// What opens a link the person Command-clicks.
    pub(super) opens: Box<dyn Fn(&str)>,
    /// The scrollbar: shown while the pointer is over the view, the pointer on its slider, and
    /// where on the slider it was taken while it is dragged (points from its top).
    pub(super) bar_shown: bool,
    pub(super) bar_pointed: bool,
    pub(super) bar_held: Option<f64>,
    /// The text of each row in view; what a screen reader was told of the screen, and what it read out last; whether one
    /// runs, as a check says (none: as the system says).
    pub(super) spoken: Spoken,
    pub(super) lines: Lines,
    pub(super) said: String,
    pub(super) reader: Option<bool>,
    /// The system's display options as a check sets them (more contrast, less transparency);
    /// none: as the system says.
    pub(super) display: Option<(bool, bool)>,
    /// The view's place in the count of views alive, for checks: freed with the view.
    _alive: super::super::Alive,
}

impl State {
    /// The state of a new view of `surface`, on a screen of `scale` pixels to a point, opening
    /// links with `opens`.
    pub(super) fn new(surface: Arc<dyn Surface>, scale: f64, opens: Box<dyn Fn(&str)>) -> Self {
        Self {
            surface,
            painter: None,
            layers: Layers::new(scale),
            frame: Frame::default(),
            grid: (0, 0),
            fitted_alternate: false,
            shown: false,
            was_shown: false,
            divider: false,
            edge: None,
            lift: 0.0,
            hushed: false,
            active: false,
            key: false,
            marked: String::new(),
            committed: None,
            wheel: 0.0,
            dragging: None,
            meta: OptionMeta::Off,
            secure: false,
            drawn: 0,
            blank: 0,
            resized: 0,
            pointer: None,
            link: None,
            opens,
            bar_shown: false,
            bar_pointed: false,
            bar_held: None,
            spoken: Spoken::default(),
            lines: Lines::default(),
            said: String::new(),
            reader: None,
            display: None,
            _alive: super::super::Alive::new(),
        }
    }
}
