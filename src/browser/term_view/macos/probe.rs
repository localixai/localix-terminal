//! For checks: what a check tells a terminal's view as the system would (a key pressed and let
//! go, the pointer moved, pressed or let go over a cell, the modifiers changed, each through the
//! same methods the system calls; an input method composing and committing) and reads of it
//! (what it drew, its layers as a picture); the wheel, the scrollbar, the Edit menu, the keys
//! given or taken, the system's display options and VoiceOver's questions (`probe_more.rs`).
//! Nothing else calls it.

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Bool};
use objc2::{class, msg_send, sel};
use objc2_core_foundation::CGPoint;
use objc2_foundation::{NSRange, NSString};

use super::super::{Pointing, TermAnswer, TermProbe};
use super::layout;
use super::view::TermView;

/// NSEventType's key down and key up, the modifiers changed, and the left button's and the
/// pointer's events.
const KEY_DOWN: usize = 10;
const KEY_UP: usize = 11;
const FLAGS_CHANGED: usize = 12;
pub(super) const MOUSE_DOWN: usize = 1;
pub(super) const MOUSE_UP: usize = 2;
pub(super) const MOUSE_MOVED: usize = 5;
pub(super) const MOUSE_DRAGGED: usize = 6;

/// The system's time now, as its events say it: presses close in it are one double click.
pub(super) fn now() -> f64 {
    // SAFETY: NSProcessInfo's class method answers the shared, live process info.
    let info: Retained<AnyObject> = unsafe { msg_send![class!(NSProcessInfo), processInfo] };
    // SAFETY: it answers the time since the system started, an NSTimeInterval.
    unsafe { msg_send![&*info, systemUptime] }
}

/// A pointer event of `kind` over the middle of the cell `(column, row)` of the view, with
/// `flags` held, as the system makes one.
fn pointer_event(
    view: &TermView,
    kind: usize,
    (column, row): (u16, u16),
    flags: usize,
) -> Option<Retained<AnyObject>> {
    let (width, height) = view.state()?.painter.as_ref()?.cell();
    let pad = layout::gutter(view.state()?.frame.alternate);
    let inside = CGPoint::new(
        f64::from(pad.left) + (f64::from(column) + 0.5) * width,
        f64::from(pad.top) + (f64::from(row) + 0.5) * height,
    );
    point_event(view, kind, inside, flags)
}

/// A pointer event of `kind` at `inside` (the view's points), with `flags` held.
pub(super) fn point_event(
    view: &TermView,
    kind: usize,
    inside: CGPoint,
    flags: usize,
) -> Option<Retained<AnyObject>> {
    // SAFETY: a view converts a point of its own into its window's (nil).
    let at: CGPoint =
        unsafe { msg_send![view, convertPoint: inside, toView: std::ptr::null::<AnyObject>()] };
    let number: isize = super::ns::window(view).map_or(0, |window| {
        // SAFETY: a window answers its number, an NSInteger.
        unsafe { msg_send![&*window, windowNumber] }
    });
    // SAFETY: each argument has the type NSEvent's class method declares for it, in its order;
    // it answers an autoreleased event or nil, which the call retains.
    unsafe {
        msg_send![
            class!(NSEvent),
            mouseEventWithType: kind,
            location: at,
            modifierFlags: flags,
            timestamp: now(),
            windowNumber: number,
            context: std::ptr::null::<AnyObject>(),
            eventNumber: 0_isize,
            clickCount: 1_isize,
            pressure: 1.0_f32
        ]
    }
}

/// A key event of `kind` in the view's window, as the system makes one.
fn key_event(
    view: &TermView,
    kind: usize,
    (code, flags): (u16, usize),
    chars: &str,
    plain: &str,
) -> Option<Retained<AnyObject>> {
    let (chars, plain) = (NSString::from_str(chars), NSString::from_str(plain));
    let number: isize = super::ns::window(view).map_or(0, |window| {
        // SAFETY: a window answers its number, an NSInteger.
        unsafe { msg_send![&*window, windowNumber] }
    });
    // SAFETY: as for a pointer event: each argument of the type the class method declares, in
    // its order, and an autoreleased event or nil answered, which the call retains.
    unsafe {
        msg_send![
            class!(NSEvent),
            keyEventWithType: kind,
            location: CGPoint::new(0.0, 0.0),
            modifierFlags: flags,
            timestamp: 0.0_f64,
            windowNumber: number,
            context: std::ptr::null::<AnyObject>(),
            characters: &*chars,
            charactersIgnoringModifiers: &*plain,
            isARepeat: Bool::NO,
            keyCode: code
        ]
    }
}

/// The kind of event the system sends for `pointing`.
pub(super) fn kind(pointing: Pointing) -> usize {
    match pointing {
        Pointing::Moved | Pointing::Flags => MOUSE_MOVED,
        Pointing::Down => MOUSE_DOWN,
        Pointing::Dragged => MOUSE_DRAGGED,
        Pointing::Up => MOUSE_UP,
    }
}

impl TermView {
    /// `event` handed to the view's own method for `pointing`, as the system hands it over.
    pub(super) fn pointed(&self, pointing: Pointing, event: &AnyObject) {
        let selector = match pointing {
            Pointing::Moved | Pointing::Flags => sel!(mouseMoved:),
            Pointing::Down => sel!(mouseDown:),
            Pointing::Dragged => sel!(mouseDragged:),
            Pointing::Up => sel!(mouseUp:),
        };
        // SAFETY: a responder takes a pointer event of the kind each of these methods names.
        let _: *mut AnyObject =
            unsafe { msg_send![self, performSelector: selector, withObject: event] };
    }

    /// What `probe` asks: done, or what it read; none when it could not be done.
    pub(crate) fn probe(&self, probe: TermProbe) -> Option<TermAnswer> {
        let none = NSRange::new(isize::MAX.unsigned_abs(), 0);
        match probe {
            TermProbe::Seen => self.seen().map(TermAnswer::Seen),
            TermProbe::Pixels => self.pixels().map(TermAnswer::Pixels),
            TermProbe::Key {
                code,
                flags,
                chars,
                plain,
            } => {
                let down = key_event(self, KEY_DOWN, (code, flags), &chars, &plain)?;
                let up = key_event(self, KEY_UP, (code, flags), &chars, &plain)?;
                // SAFETY: a responder takes a key's events as the system hands them over.
                let _: () = unsafe { msg_send![self, keyDown: &*down] };
                // SAFETY: as above.
                let _: () = unsafe { msg_send![self, keyUp: &*up] };
                Some(TermAnswer::Done)
            }
            TermProbe::Marked(text) => {
                let text = NSString::from_str(&text);
                let at = NSRange::new(text.length(), 0);
                // SAFETY: the input client's own method, as an input method calls it.
                let _: () = unsafe {
                    msg_send![self, setMarkedText: &*text, selectedRange: at, replacementRange: none]
                };
                Some(TermAnswer::Done)
            }
            TermProbe::Insert(text) => {
                let text = NSString::from_str(&text);
                // SAFETY: as above.
                let _: () = unsafe { msg_send![self, insertText: &*text, replacementRange: none] };
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
                let cell = (column, row);
                if pointing == Pointing::Flags {
                    let event = key_event(self, FLAGS_CHANGED, (0x37, flags), "", "")?;
                    // SAFETY: a responder takes the event as the system hands it over.
                    let _: () = unsafe { msg_send![self, flagsChanged: &*event] };
                    return Some(TermAnswer::Done);
                }
                let event = pointer_event(self, kind(pointing), cell, flags)?;
                self.pointed(pointing, &event);
                Some(TermAnswer::Done)
            }
            TermProbe::Bar { pointing, percent } => self.bar_probe(pointing, percent),
            TermProbe::Wheel(rows) => self.wheel_probe(rows),
            TermProbe::Edit(edit) => Some(self.edit_probe(edit)),
            TermProbe::Focus(on) => Some(self.focus_probe(on)),
            TermProbe::Display { more, solid } => Some(self.display_probe(more, solid)),
            TermProbe::Reader => Some(self.reader_probe()),
            // The window's to carry out (host_dispatch_term.rs).
            TermProbe::EdgeTold => {
                // SAFETY: a view takes the live resize's start as the system sends it.
                let _: *mut AnyObject =
                    unsafe { msg_send![self, performSelector: sel!(viewWillStartLiveResize)] };
                Some(TermAnswer::Done)
            }
            TermProbe::Edge(on) => {
                self.edge_check(on);
                Some(TermAnswer::Done)
            }
            TermProbe::Settings(_)
            | TermProbe::Views
            | TermProbe::Window(..)
            | TermProbe::Press(..)
            | TermProbe::Carry
            | TermProbe::KeyIn(..)
            | TermProbe::Close => None,
        }
    }
}
