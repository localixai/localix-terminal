//! An input method's text in the view (`NSTextInputClient`): what it composes (Japanese,
//! Chinese, Korean; a dead key's accent) is the view's alone, drawn at the cursor and never told
//! the terminal, which only gets what is committed; its window of candidates stands at the
//! cursor. Text committed while a key is read is handed to that key (`events.rs`); text that
//! comes on its own (the character viewer, dictation) is typed as it comes.

use objc2::runtime::AnyObject;
use objc2::{class, msg_send};
use objc2_core_foundation::{CGPoint, CGRect, CGSize};
use objc2_foundation::{NSRange, NSString};

use super::view::TermView;

/// No place, as the system says it.
const NOT_FOUND: usize = isize::MAX.unsigned_abs();

/// The text `string` holds: an NSString, or an NSAttributedString's characters.
fn text_of(string: &AnyObject) -> String {
    // SAFETY: every NSObject answers `isKindOfClass:`.
    let attributed: bool = unsafe { msg_send![string, isKindOfClass: class!(NSAttributedString)] };
    let plain: *const AnyObject = match attributed {
        // SAFETY: an attributed string answers its plain string.
        true => unsafe { msg_send![string, string] },
        false => string,
    };
    // SAFETY: anything else an input method hands over is an NSString, or nil.
    let plain = unsafe { plain.cast::<NSString>().as_ref() };
    plain.map(NSString::to_string).unwrap_or_default()
}

/// Text committed: handed to the key being read, else typed now.
pub(super) fn insert(view: &TermView, string: &AnyObject) {
    let text = text_of(string);
    let surface = {
        let Some(mut state) = view.state() else {
            return;
        };
        state.marked.clear();
        state.spoken.typed(&text);
        match state.committed.as_mut() {
            Some(committed) => {
                committed.push_str(&text);
                None
            }
            None => Some(std::sync::Arc::clone(&state.surface)),
        }
    };
    if let Some(surface) = surface {
        surface.text(&text);
        view.redraw(false);
    }
}

/// What is composing now, drawn at the cursor.
pub(super) fn mark(view: &TermView, string: &AnyObject) {
    let text = text_of(string);
    if let Some(mut state) = view.state() {
        state.marked = text;
    }
    view.redraw(false);
}

/// Composing ended with nothing committed.
pub(super) fn unmark(view: &TermView) {
    if let Some(mut state) = view.state() {
        state.marked.clear();
    }
    view.redraw(false);
}

/// Nothing of the terminal is selected as the input method reads it: the place is the end of
/// what composes.
pub(super) fn selected(view: &TermView) -> NSRange {
    let at = view
        .state()
        .map_or(0, |state| state.marked.encode_utf16().count());
    NSRange::new(at, 0)
}

pub(super) fn marked_range(view: &TermView) -> NSRange {
    match view
        .state()
        .map(|state| state.marked.encode_utf16().count())
    {
        Some(length) if length > 0 => NSRange::new(0, length),
        _ => NSRange::new(NOT_FOUND, 0),
    }
}

/// Where the cursor's cell is on the screen, for the window of candidates to stand by.
pub(super) fn cursor_rect(view: &TermView) -> CGRect {
    let cell = view.state().and_then(|state| {
        let cell = state.painter.as_ref()?.cell();
        let [x, y, width, height] = super::acts::cursor_cell(&state.frame, cell)?;
        Some(CGRect::new(CGPoint::new(x, y), CGSize::new(width, height)))
    });
    let Some(cell) = cell else {
        return CGRect::default();
    };
    let none = std::ptr::null::<AnyObject>();
    // SAFETY: a view converts a rect of its own into its window's (nil).
    let in_window: CGRect = unsafe { msg_send![view, convertRect: cell, toView: none] };
    match super::ns::window(view) {
        // SAFETY: a window converts a rect of its own to the screen's.
        Some(window) => unsafe { msg_send![&*window, convertRectToScreen: in_window] },
        None => in_window,
    }
}
