//! The window's views moved by a layout, where the system plays no springs for the app yet:
//! each view stands where the layout put it at once (`glide.rs`; macOS plays the way there).

use super::{Moved, Opens};

/// Where the views stood before a layout: nothing is kept of it here.
pub(in crate::browser) struct Standing;

/// Where `views` stand now, to be read before the layout that moves them.
pub(in crate::browser) fn standing(_views: &[Moved<'_>]) -> Standing {
    Standing
}

impl Standing {
    /// The layout made: the views stand where it put them.
    #[expect(clippy::unused_self, reason = "macOS's, with nothing to move here")]
    pub(in crate::browser) fn go(self, _opens: Option<&Opens<'_>>) {}
}
