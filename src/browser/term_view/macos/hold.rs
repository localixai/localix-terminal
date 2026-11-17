//! The view while the person moves its pane's edge, by a divider of its split or by its
//! window's edge: the terminal keeps the grid it had, so its program is not told a new size at
//! every step of the pointer and redraws nothing meanwhile; its text stays where it was, cut at
//! the pane's edge (the root clips to its bounds), and raised only as far as keeps the cursor's
//! row in a pane grown shorter. As they let go, the grid that fits the pane is told once.

use objc2::encode::{Encode, Encoding};
use objc2::msg_send;

use super::layers::still;
use super::layout;
use super::view::TermView;

/// A Core Animation transform: four rows of four, by its fields' order.
#[repr(C)]
struct Transform([f64; 16]);

const D: Encoding = Encoding::Double;

// SAFETY: CATransform3D is a C struct of sixteen CGFloats, which are doubles on every Mac the
// app runs on.
unsafe impl Encode for Transform {
    const ENCODING: Encoding = Encoding::Struct("CATransform3D", &[D; 16]);
}

impl Transform {
    /// Moved `y` points down its layer (a layer that counts from the top).
    fn down(y: f64) -> Self {
        let mut m = [0.0; 16];
        for at in [0, 5, 10, 15] {
            m[at] = 1.0;
        }
        m[13] = y;
        Self(m)
    }
}

impl TermView {
    /// Whether a divider of the view's split is held now, as the window's layout says before
    /// it places the view (which then fits it, `place`).
    pub(crate) fn hold(&self, on: bool) {
        if let Some(mut state) = self.state() {
            state.divider = on;
        }
    }

    /// Whether the window's edge is held: as a check says (`edge`), else as AppKit says, the
    /// window in a live resize. Asked each time, so no message lost or told twice can leave a
    /// grid held.
    pub(super) fn edge_held(&self, edge: Option<bool>) -> bool {
        // SAFETY: an NSView answers whether its window is in a live resize, a BOOL.
        edge.unwrap_or_else(|| unsafe { objc2::msg_send![self, inLiveResize] })
    }

    /// The window's edge let go (AppKit's word, or a check's): the grid that fits is told.
    pub(super) fn edge_let_go(&self) {
        self.lifted();
        self.fit();
    }

    /// For checks: the window's edge held from now on, or as AppKit says again.
    pub(super) fn edge_check(&self, on: bool) {
        if let Some(mut state) = self.state() {
            state.edge = on.then_some(true);
        }
        if !on {
            self.edge_let_go();
        }
    }

    /// The rows raised as far as keeps the cursor's row in view while the grid is held in a
    /// pane shorter than it; where they lie from its top otherwise.
    pub(super) fn lifted(&self) {
        let height = super::ns::bounds(self).size.height;
        let Some(mut state) = self.state() else {
            return;
        };
        let tall = state
            .painter
            .as_ref()
            .map_or(0.0, |painter| painter.cell().1);
        let pad = f64::from(layout::gutter(state.frame.alternate).top);
        let below = state
            .frame
            .cursor
            .at
            .map_or(0.0, |(_, row)| pad + (f64::from(row) + 1.0) * tall);
        let held = state.divider || self.edge_held(state.edge);
        let lift = if held { (below - height).max(0.0) } else { 0.0 };
        if lift == state.lift {
            return;
        }
        state.lift = lift;
        let root = &state.layers.root;
        // SAFETY: a layer takes the transform of its sublayers, a CATransform3D.
        still(|| unsafe {
            let _: () = msg_send![&**root, setSublayerTransform: Transform::down(-lift)];
        });
    }
}
