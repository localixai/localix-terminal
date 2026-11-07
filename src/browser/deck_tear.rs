//! The window as a tab dragged out of a list sees it (docs/windows.md): where it is on the
//! screen, where its list of tabs is, a window that follows the pointer, and one that turns
//! clear while the tab is over another window's list.

use tao::dpi::{LogicalPosition, LogicalSize};

use super::super::platform;
use super::Deck;

/// How far the pointer goes past the list of a window it is over and still is over it (points).
const LIST_SLACK: f64 = 12.0;

impl Deck {
    /// The window's content on the screen: its top left and its size, in points.
    pub(in super::super) fn on_screen(&self) -> Option<(f64, f64, f64, f64)> {
        let native = &self.frame.native;
        let scale = native.scale_factor();
        let at: LogicalPosition<f64> = native.inner_position().ok()?.to_logical(scale);
        let size: LogicalSize<f64> = native.inner_size().to_logical(scale);
        Some((at.x, at.y, size.width, size.height))
    }

    /// A point of the window, from the top left of its content, on the screen.
    pub(in super::super) fn to_screen(&self, x: f64, y: f64) -> Option<(f64, f64)> {
        let (left, top, ..) = self.on_screen()?;
        Some((left + x, top + y))
    }

    /// Whether screen point `x`, `y` is over the side's list of tabs, `LIST_SLACK` around it
    /// counted: the column under the strip, as wide as the side shows.
    pub(in super::super) fn over_list(&self, x: f64, y: f64) -> bool {
        let Some((left, top, _, tall)) = self.on_screen() else {
            return false;
        };
        if self.side.is_none() || !self.frame.native.is_visible() {
            return false;
        }
        let list = (left, top + self.top(), self.side_wide(), tall - self.top());
        x >= list.0 - LIST_SLACK
            && x <= list.0 + list.2 + LIST_SLACK
            && y >= list.1 - LIST_SLACK
            && y <= list.1 + list.3 + LIST_SLACK
    }

    /// The side's point of screen point `y`, from the top of its page.
    pub(in super::super) fn list_y(&self, y: f64) -> f64 {
        let top = self.on_screen().map_or(0.0, |(_, top, ..)| top);
        y - top - self.top()
    }

    /// Put the window's top left at screen point `x`, `y`.
    pub(in super::super) fn move_to(&self, x: f64, y: f64) {
        self.frame
            .native
            .set_outer_position(LogicalPosition::new(x, y));
    }

    /// The window clear (a tab carried in it is over another window's list) or seen again.
    pub(in super::super) fn see_through(&self, clear: bool) {
        platform::clear(&self.frame.native, clear);
    }

    /// The window over the others, without taking the keys.
    pub(in super::super) fn to_front(&self) {
        platform::to_front(&self.frame.native);
    }

    /// The window on the screen without taking the keys: a window a tab was carried out into.
    pub(in super::super) fn reveal(&self) {
        self.frame.reveal();
    }

    /// The side's page, whose card a tab held in it is drawn from.
    pub(in super::super) fn side_view(&self) -> Option<&wry::WebView> {
        self.side.as_ref()
    }

    /// Tell the side's page `script`: the slot a tab carried over its list would go into.
    pub(in super::super) fn tell_side(&self, script: &str) {
        if let Some(side) = &self.side {
            let _ = side.evaluate_script(script);
        }
    }
}
