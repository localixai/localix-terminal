//! The cursor's blinking on Linux: shown for the first half of its period and hidden for the
//! second, from shown again where it moves, as the terminal page's was. GTK has no animation a
//! drawing area's picture can be given, so one timeout of half a period turns it, made again by
//! the turn before only while it blinks: none exists while the cursor stands still, is out of
//! sight or out of use; a turn outdated by a change of the blinking is taken off, as the
//! pending one is when the view closes.

use std::time::Duration;

use gtk::glib;
use gtk::prelude::WidgetExt;

use super::overlay::BLINK;
use super::view::View;

impl View {
    /// The cursor blinks from `at` (where it is) from now on, or not (none).
    pub(super) fn blink(&self, at: Option<[f64; 4]>) {
        let turn = {
            let Some(mut state) = self.state() else {
                return;
            };
            if state.blink == at {
                return;
            }
            state.blink = at;
            state.blink_shown = true;
            // Any turn on its way is outdated, and taken off.
            state.blink_turn = state.blink_turn.wrapping_add(1);
            if let Some(source) = state.blink_source.take() {
                source.remove();
            }
            at.map(|_| state.blink_turn)
        };
        if let Some(turn) = turn {
            self.turn_after(turn);
        }
    }

    /// The `turn`th turn of the cursor, half a period from now.
    fn turn_after(&self, turn: u64) {
        let view = self.weak();
        let half = Duration::from_secs_f64(BLINK / 2.0);
        let source = glib::timeout_add_local_once(half, move || {
            let Some(view) = view() else {
                return;
            };
            let still = view.state().is_some_and(|mut state| {
                // The timeout is spent as it runs: nothing is left to take off.
                state.blink_source = None;
                let current = state.blink_turn == turn && state.blink.is_some();
                if current {
                    state.blink_shown = !state.blink_shown;
                }
                current
            });
            if still {
                view.area.queue_draw();
                view.turn_after(turn);
            }
        });
        match self.state() {
            Some(mut state) => state.blink_source = Some(source),
            None => source.remove(),
        }
    }

    /// The cursor's next turn taken off: the view closes.
    pub(super) fn unblink(&self) {
        let source = self.state().and_then(|mut state| state.blink_source.take());
        if let Some(source) = source {
            source.remove();
        }
    }
}
