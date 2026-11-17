//! The terminal drawn on Linux: Pango into Cairo image surfaces (`canvas.rs`, `fonts.rs`,
//! `text.rs`), the canvas the shared painter draws rows, the cursor, the scrollbar and an input
//! method's text with (`painter.rs`); and the view in the window that shows them: a GTK drawing
//! area where the tab's pane is (`view.rs`, what it keeps `state.rs`), drawn from those pictures
//! (`paint.rs`), with its keys and input method (`events.rs`, `keymap.rs`), its pointer and wheel
//! (`pointer.rs`), links and scrollbar (`hover.rs`), the cursor's blinking (`blink.rs`), and what checks and an
//! agent's picture ask of it (`probe.rs`). A screen reader is told the view is a terminal and
//! its name, not yet its text.

mod blink;
mod canvas;
mod events;
mod fonts;
mod hover;
mod keymap;
mod paint;
mod pointer;
mod probe;
mod state;
mod text;
mod view;

pub(crate) use canvas::Canvas;
pub(crate) use view::TermView;

use super::painter::{Placed, TermPainter, pixels};
use super::{Picture, TermLook, acts, layout, overlay};
