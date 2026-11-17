//! For checks and agents: what a check tells a terminal's view as the system would (an input
//! method composing and committing, the pointer over a cell or the scrollbar, the wheel, the
//! Edit menu, the keys given or taken, the display options) and reads of it (what it drew, its
//! pictures laid together); and the picture of the terminal an agent asks for. A key is told by
//! a Mac's key code, which has no place here, and a screen reader is not yet given the text:
//! those two probes answer nothing. And how many terminal views a window holds in its box.

use std::sync::mpsc::SyncSender;

use gtk::prelude::{ContainerExt, WidgetExt};

use super::super::painter::{self, Image};
use super::super::{Edit, Pointing, TermAnswer, TermProbe};
use super::overlay::SCROLLBAR_WIDTH;
use super::pointer::{self, Point, Pointer};
use super::view::View;
use super::{events, hover, layout};
use crate::browser::capture::{self, Shot};

/// A probe's modifier flags (the system's on a Mac: Shift, Control, Option, Command) as GDK's.
fn state(flags: usize) -> u32 {
    let mut state = 0;
    for (mac, gdk) in [(17, 0), (18, 2), (19, 3), (20, 26)] {
        if flags & (1 << mac) != 0 {
            state |= 1 << gdk;
        }
    }
    state
}

const fn pointer_of(pointing: Pointing) -> Pointer {
    match pointing {
        Pointing::Moved | Pointing::Flags => Pointer::Moved,
        Pointing::Down => Pointer::Down,
        Pointing::Dragged => Pointer::Dragged,
        Pointing::Up => Pointer::Up,
    }
}

impl View {
    /// What `probe` asks: done, or what it read; none when it could not be done.
    pub(crate) fn probe(&self, probe: TermProbe) -> Option<TermAnswer> {
        match probe {
            TermProbe::Seen => self.seen().map(TermAnswer::Seen),
            TermProbe::Pixels => self.pixels().map(TermAnswer::Pixels),
            TermProbe::Marked(text) => {
                events::compose(self, &text);
                Some(TermAnswer::Done)
            }
            TermProbe::Insert(text) => {
                events::commit(self, &text);
                Some(TermAnswer::Done)
            }
            TermProbe::Meta(meta) => {
                let mut state = self.state()?;
                state.meta = meta;
                state.surface.option_meta(meta);
                Some(TermAnswer::Done)
            }
            TermProbe::Pointer {
                pointing,
                column,
                row,
                flags,
            } => {
                let (width, height) = self.state()?.painter.as_ref()?.cell();
                let pad = layout::gutter(self.state()?.frame.alternate);
                let at = (
                    f64::from(pad.left) + (f64::from(column) + 0.5) * width,
                    f64::from(pad.top) + (f64::from(row) + 0.5) * height,
                );
                self.point(pointing, at, state(flags));
                Some(TermAnswer::Done)
            }
            TermProbe::Bar { pointing, percent } => {
                let area = &self.area;
                let (width, height) = (
                    f64::from(area.allocated_width()),
                    f64::from(area.allocated_height()),
                );
                let at = (
                    width - SCROLLBAR_WIDTH / 2.0,
                    height * f64::from(percent.min(100)) / 100.0,
                );
                if pointing == Pointing::Down {
                    hover::pointer_in(self, true);
                }
                self.point(pointing, at, 0);
                Some(TermAnswer::Done)
            }
            TermProbe::Wheel(rows) => {
                let at = self.state()?.pointer.unwrap_or((1.0, 1.0));
                let point = Point {
                    at,
                    state: 0,
                    button: 0,
                    time: 0,
                };
                pointer::wheel(self, point, f64::from(rows), false);
                Some(TermAnswer::Done)
            }
            TermProbe::Edit(edit) => {
                match edit {
                    Edit::Copy => events::copy(self),
                    Edit::Paste => events::paste(self),
                    Edit::SelectAll => events::select_all(self),
                }
                Some(TermAnswer::Done)
            }
            TermProbe::Focus(on) => {
                events::focus_probe(self, on);
                Some(TermAnswer::Done)
            }
            TermProbe::Display { more, solid } => {
                if let Some(mut state) = self.state() {
                    state.display = Some((more, solid));
                }
                self.restyle();
                Some(TermAnswer::Done)
            }
            // The window's to carry out (host_dispatch_term.rs).
            TermProbe::Key { .. }
            | TermProbe::Reader
            | TermProbe::Settings(_)
            | TermProbe::Views
            | TermProbe::Window(..)
            | TermProbe::Edge(_)
            | TermProbe::EdgeTold
            | TermProbe::Press(..)
            | TermProbe::Carry
            | TermProbe::KeyIn(..)
            | TermProbe::Close => None,
        }
    }

    /// The pointer `pointing` at `at` (points) with GDK's modifier `state` held.
    fn point(&self, pointing: Pointing, at: (f64, f64), state: u32) {
        let pressed = matches!(pointing, Pointing::Down | Pointing::Dragged | Pointing::Up);
        let point = Point {
            at,
            state,
            button: u32::from(pressed),
            time: 0,
        };
        pointer::mouse(self, point, pointer_of(pointing));
    }

    /// The picture of the terminal now, sent to `reply` as JPEG.
    pub(crate) fn shot(&self, reply: SyncSender<Result<Shot, String>>) {
        match self.picture() {
            Some(image) => capture::drawn(&image, reply),
            None => drop(reply.send(Err("the terminal's view drew no picture".into()))),
        }
    }

    /// The whole frame drawn: the view's own when it is shown (brought up to now first), else
    /// one asked of the terminal for the picture alone.
    fn picture(&self) -> Option<Image> {
        let shown = self.state()?.shown;
        if shown {
            self.redraw(false);
        }
        let state = self.state()?;
        let shown = shown.then_some(&state.frame);
        painter::for_agent(state.painter.as_ref()?, shown, &*state.surface)
    }
}

/// The name each terminal's drawing area is given, by which a window's are counted.
pub(super) const NAME: &str = "llxterm-terminal";

impl super::view::TermView {
    /// How many terminal views are in `native`'s box.
    pub(crate) fn placed_in(native: &tao::window::Window) -> usize {
        use tao::platform::unix::WindowExtUnix;
        native.default_vbox().map_or(0, |vbox| {
            (vbox.children().iter())
                .filter(|child| child.widget_name() == NAME)
                .count()
        })
    }
}
