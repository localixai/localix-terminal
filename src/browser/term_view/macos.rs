//! The terminal drawn on macOS: Core Text into Core Graphics bitmaps (`canvas.rs`, `fonts.rs`,
//! `text.rs`), the canvas the shared painter draws rows, the cursor, the scrollbar and an input
//! method's text with (`painter.rs`); and the view in the window that shows them (`view.rs`,
//! what it keeps `state.rs`), with its layers (`layers.rs`), keys and pointer (`events.rs`,
//! `keymap.rs`), its grid held while its pane's edge is moved (`hold.rs`), its links (`links.rs`), its scrollbar (`bar.rs`), its input method (`ime.rs`),
//! VoiceOver (`access.rs`) and the picture an agent is given (`shot.rs`).

mod access;
mod bar;
mod canvas;
mod events;
mod fonts;
mod hold;
mod ime;
mod keymap;
mod layers;
mod links;
mod ns;
mod paint;
mod place;
mod pointer;
mod probe;
mod probe_more;
mod shot;
mod state;
mod text;
mod view;

pub(crate) use canvas::Canvas;
pub(crate) use place::Owned;
pub(crate) use view::TermView;

use super::painter::{Placed, TermPainter, pixels};
use super::{Picture, TermLook, acts, layout, overlay};
