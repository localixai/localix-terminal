//! A session as a view draws and acts on it (`surface::Surface`): its frame from the core's
//! render state, the first half of each update under the session's lock and the second after it
//! is let go of (a frame a program holds back is not in the core's screen until it lets go, so
//! nothing of it is drawn before); and what the view tells it:
//! keys, text, the pointer and the wheel, focus, pastes, the selection and a find, each turned
//! into the bytes the program asked for and queued, never written while the lock is held.

use super::super::{keys, pty};
use super::{Session, read};
use crate::surface::{
    Button, CellPixels, Count, FindStep, Frame, Geometry, Key, KeyCode, Link, Mods, Mouse,
    MouseAction, OptionMeta, Palette, Routed, ScrollTo, Surface,
};
use crate::terminal::Render;

/// What a session keeps for its view: which Option key is Alt, whether a button is held down in
/// a program that takes the mouse, and whether what lies over the rows (a find's matches)
/// changed so the next frame draws them all.
#[derive(Debug, Default)]
pub(super) struct Viewed {
    meta: OptionMeta,
    held: bool,
    redraw: bool,
}

impl Session {
    /// Bytes the person's keys or text make, queued as what they typed.
    fn keyed(&self, bytes: &[u8]) {
        let text = String::from_utf8_lossy(bytes);
        {
            let mut state = self.shared.lock();
            if read::edits_the_line(&text) {
                state.screen.marks_mut().typed_in(true);
            }
            state.screen.core_mut().scroll_to(ScrollTo::Bottom);
        }
        self.attended(&text);
        self.input.queue(bytes);
    }
}

impl Surface for Session {
    fn frame(&self, into: &mut Frame, full: bool) -> bool {
        // The session's lock first, then the render state's, always in that order.
        let mut state = self.shared.lock();
        let mut slot = crate::lock(&self.render);
        let new = slot.is_none();
        if new {
            *slot = Render::new().ok();
        }
        let Some(render) = slot.as_mut() else {
            return false;
        };
        // A frame held past its second is drawn now, whether or not the reader woke for it.
        state.screen.core_mut().let_go_if_long();
        if render.begin(state.screen.core()).is_err() {
            return false;
        }
        let full = full || new || std::mem::take(&mut state.viewed.redraw);
        let lines = state.screen.size().0;
        let top = render.scroll().offset;
        let found = state.screen.core_mut().found_in_view(top, lines);
        let ended = state.ended.then(|| read::ended_words(state.status));
        drop(state);
        let drawn = render.finish(into, full);
        if drawn {
            (into.matches, into.current) = found;
        }
        // The program's end changes nothing the core draws: the frame is new for its line.
        let told = into.ended != ended;
        into.ended = ended;
        drawn || told
    }

    fn resize(&self, columns: u16, lines: u16, cell: CellPixels) {
        self.resized(columns, lines, cell);
    }

    fn hidden(&self) {
        *crate::lock(&self.render) = None;
    }

    fn colors(&self, palette: &Palette) {
        let mut state = self.shared.lock();
        state.screen.core_mut().set_colors(palette);
        state.viewed.redraw = true;
    }

    fn option_meta(&self, meta: OptionMeta) {
        self.shared.lock().viewed.meta = meta;
    }

    fn key(&self, key: &Key) -> bool {
        let bytes = {
            let mut state = self.shared.lock();
            let meta = state.viewed.meta;
            match keys::mac::sent(key, meta.is_meta(key.mods)) {
                Some(sent) => sent.to_vec(),
                None => state.screen.core_mut().encode_key(key, meta),
            }
        };
        if !bytes.is_empty() {
            self.keyed(&bytes);
        }
        !bytes.is_empty()
    }

    fn text(&self, committed: &str) {
        if !committed.is_empty() {
            self.keyed(committed.as_bytes());
        }
    }

    fn mouse(&self, mouse: &Mouse, geometry: &Geometry) -> Routed {
        let (routed, bytes) = {
            let mut state = self.shared.lock();
            let core = state.screen.core();
            let taken = core.mouse_tracking() && !mouse.mods.has(Mods::ALT);
            match taken {
                true => {
                    let held = match mouse.action {
                        MouseAction::Press => true,
                        MouseAction::Release => false,
                        MouseAction::Motion => state.viewed.held,
                    };
                    state.viewed.held = held;
                    let bytes = state.screen.core_mut().encode_mouse(mouse, geometry, held);
                    (Routed::Program, bytes)
                }
                false => {
                    let scroll = state.screen.core_mut().select(mouse, geometry);
                    (Routed::Selection { scroll }, vec![])
                }
            }
        };
        self.input.queue(&bytes);
        routed
    }

    fn wheel(&self, lines: i32, mouse: &Mouse, geometry: &Geometry) {
        let bytes = {
            let mut state = self.shared.lock();
            let meta = state.viewed.meta;
            let core = state.screen.core_mut();
            let up = lines > 0;
            let count = lines.unsigned_abs();
            if core.mouse_tracking() && !mouse.mods.has(Mods::ALT) {
                let button = if up {
                    Button::WheelUp
                } else {
                    Button::WheelDown
                };
                let press = Mouse {
                    button: Some(button),
                    action: MouseAction::Press,
                    ..*mouse
                };
                let once = core.encode_mouse(&press, geometry, false);
                once.repeat(count as usize)
            } else if core.alternate() && core.alternate_scroll() {
                let code = if up {
                    KeyCode::ArrowUp
                } else {
                    KeyCode::ArrowDown
                };
                let arrow = Key {
                    code,
                    ..Key::default()
                };
                core.encode_key(&arrow, meta).repeat(count as usize)
            } else {
                core.scroll(isize::try_from(lines).unwrap_or(0));
                vec![]
            }
        };
        self.input.queue(&bytes);
    }

    fn scroll_to(&self, to: ScrollTo) {
        self.shared.lock().screen.core_mut().scroll_to(to);
    }

    fn focus(&self, on: bool) {
        let bytes = self.shared.lock().screen.core().focus_report(on);
        self.input.queue(&bytes);
    }

    fn paste(&self, text: &str) {
        let bytes = self.shared.lock().screen.core().paste_bytes(text);
        if !bytes.is_empty() {
            self.keyed(&bytes);
        }
    }

    fn copy(&self) -> Option<String> {
        self.shared.lock().screen.core().selection_text()
    }

    fn select_all(&self) {
        self.shared.lock().screen.core_mut().select_all();
    }

    fn clear(&self) {
        {
            let mut state = self.shared.lock();
            let core = state.screen.core_mut();
            if core.alternate() {
                return;
            }
            // The history and the screen go; the shell is asked to draw its prompt again.
            core.feed(b"\x1b[H\x1b[2J\x1b[3J");
        }
        self.input.queue(b"\x0c");
    }

    fn find(&self, step: &FindStep) -> Count {
        let mut state = self.shared.lock();
        // What lies over the rows changed, but for a recount: the rows it lights are drawn anew
        // as they come.
        state.viewed.redraw |= *step != FindStep::Again;
        state.screen.core_mut().find(step)
    }

    fn link_at(&self, column: u16, y: u16) -> Option<Link> {
        self.shared.lock().screen.core().link_at(column, y)
    }

    fn copied(&self) -> Option<String> {
        self.shared.lock().screen.core_mut().program_copy()
    }

    fn rang(&self) -> bool {
        self.shared.lock().screen.core_mut().bell_rang()
    }

    fn reader_text(&self) -> String {
        self.shared.lock().screen.text().0
    }

    fn hides_input(&self) -> bool {
        pty::hides_input(&self.control)
    }
}

#[cfg(test)]
#[path = "surface_tests.rs"]
mod tests;
