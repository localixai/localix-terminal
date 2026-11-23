//! The keys the person presses in the view, told to the terminal: the view's own Command keys,
//! Option as Meta on the side chosen, everything else read by the input method first
//! (`ime.rs`); Copy, Paste and Select All; focus; and the system's secure input while a program
//! reads a password, whether or not a key was pressed for it. The pointer is `pointer.rs`.

use std::sync::Arc;

use objc2::runtime::Bool;
use objc2::{Message, class, msg_send};
use objc2_app_kit::NSEvent;
use objc2_foundation::NSArray;

use super::acts::{self, Command};
use super::view::TermView;
use super::{keymap, ns};
use crate::surface::{Key, KeyAction, Mods, ScrollTo, Surface};

#[link(name = "Carbon", kind = "framework")]
unsafe extern "C" {
    fn EnableSecureEventInput() -> i32;
    fn DisableSecureEventInput() -> i32;
}

/// The terminal the view shows, if the view can be told anything now.
pub(super) fn surface(view: &TermView) -> Option<Arc<dyn Surface>> {
    let state = view.state()?;
    (!state.hushed).then(|| Arc::clone(&state.surface))
}

/// The view's place in the window's keys changed (`on`: it has them now): the program is told
/// when it asked to be, once for each change, and the view is drawn as the one in use or not,
/// in a key window or not.
pub(super) fn focused(view: &TermView, on: bool) {
    focused_in(view, on, ns::key_window(view));
}

/// As `focused`, its window the key one (`key`) or not.
pub(super) fn focused_in(view: &TermView, on: bool, key: bool) {
    let active = on && key;
    secure(view, active);
    let was = view.state().map(|state| state.active);
    if was != Some(active)
        && let Some(surface) = surface(view)
    {
        surface.focus(active);
    }
    view.activate(active, key);
}

/// The window became the key one or stopped being it: every view of it says so (the one with
/// its keys to its program), and a find's matches take the accent or the system's grey.
pub(super) fn window_key_changed(view: &TermView) {
    focused(view, view.has_keys());
}

/// The system's secure input on while the view has the keys in the key window (`asked`) and the
/// program reads what it does not show, and off as soon as either stops: checked as the view
/// takes or lets go of the keys, as a key is pressed, and as the terminal says its screen
/// changed (a program asks for a password, or is done with it, as it writes). Each on is
/// matched by one off.
pub(super) fn secure(view: &TermView, asked: bool) {
    let Some(mut state) = view.state() else {
        return;
    };
    let wanted = asked && state.surface.hides_input();
    if wanted != state.secure {
        state.secure = wanted;
        let _ = match wanted {
            // SAFETY: the system's own call, matched by one off (the flag above).
            true => unsafe { EnableSecureEventInput() },
            // SAFETY: the off that matches the one on.
            false => unsafe { DisableSecureEventInput() },
        };
    }
}

pub(super) fn key_down(view: &TermView, event: &NSEvent) {
    let Some(surface) = surface(view) else {
        return;
    };
    // SAFETY: NSCursor hides the pointer until it moves, as typing does in a text view.
    let _: () = unsafe { msg_send![class!(NSCursor), setHiddenUntilMouseMoves: Bool::YES] };
    secure(view, true);
    let flags = ns::flags(event);
    let mods = keymap::mods(flags);
    let action = match ns::repeats(event) {
        true => KeyAction::Repeat,
        false => KeyAction::Press,
    };
    let unshifted = ns::chars_with(event, 0);
    let key = Key {
        code: keymap::code(ns::key_code(event)),
        action,
        mods,
        unshifted: unshifted.chars().next(),
        ..Key::default()
    };
    if mods.has(Mods::SUPER) && !mods.has(Mods::CTRL) {
        return command(view, &*surface, &key);
    }
    let meta = view.state().map(|state| state.meta).unwrap_or_default();
    if mods.has(Mods::ALT) && meta.is_meta(mods) {
        let typed = acts::printable(&ns::chars_with(event, keymap::without_option(flags)));
        let text = keymap::meta_text(key.code, &typed, mods.has(Mods::SHIFT));
        let consumed = acts::consumed(&text, mods, Mods::SHIFT);
        surface.key(&Key {
            text,
            consumed,
            ..key
        });
        return;
    }
    typed(view, &*surface, event, &key);
}

/// A key read by the input method first: what it composes stays the view's, what it commits is
/// typed, and a key it leaves alone is encoded as the program asked.
fn typed(view: &TermView, surface: &dyn Surface, event: &NSEvent, key: &Key) {
    let had_marked = view.state().is_some_and(|state| !state.marked.is_empty());
    if let Some(mut state) = view.state() {
        state.committed = Some(String::new());
    }
    let events = NSArray::from_retained_slice(&[event.retain()]);
    // SAFETY: a responder reads key events through its input context, calling the view's
    // text input methods back.
    let _: () = unsafe { msg_send![view, interpretKeyEvents: &*events] };
    let (committed, marked) = view.state().map_or((String::new(), false), |mut state| {
        (
            state.committed.take().unwrap_or_default(),
            !state.marked.is_empty(),
        )
    });
    if marked || had_marked {
        if !committed.is_empty() {
            surface.text(&committed);
        }
        view.redraw(false);
        return;
    }
    let mut text = acts::printable(&committed);
    // With Control and Shift the input method hands over a control character, or none, and the
    // encoder would send nothing: the key is read by what it types with Shift (Ctrl+_ is
    // Control, Shift and minus; Ctrl+Shift+Space a NUL). Control alone is read by its place.
    if text.is_empty() && key.mods.has(Mods::CTRL) && key.mods.has(Mods::SHIFT) {
        let flags = keymap::without_control(ns::flags(event));
        text = acts::printable(&ns::chars_with(event, flags));
    }
    let consumed = acts::consumed(&text, key.mods, Mods::SHIFT | Mods::ALT);
    let key = Key {
        text: text.clone(),
        consumed,
        ..key.clone()
    };
    if !surface.key(&key) && !text.is_empty() {
        surface.text(&text);
    }
}

/// A Command key the window's menu did not take: the view's own (⌘K clears, ⌘↑ and ⌘↓ go to
/// the top and the bottom) or a Mac terminal's (⌘←, ⌘→, ⌘⌫, ⌘⌦); anything else sends nothing.
fn command(view: &TermView, surface: &dyn Surface, key: &Key) {
    match acts::command(key) {
        Command::Send => {
            surface.key(key);
            return;
        }
        Command::Clear => surface.clear(),
        Command::Top => surface.scroll_to(ScrollTo::Top),
        Command::Bottom => surface.scroll_to(ScrollTo::Bottom),
        Command::Nothing => return,
    }
    view.redraw(false);
}

pub(super) fn key_up(view: &TermView, event: &NSEvent) {
    if view.state().is_some_and(|state| !state.marked.is_empty()) {
        return;
    }
    let Some(surface) = surface(view) else {
        return;
    };
    surface.key(&Key {
        code: keymap::code(ns::key_code(event)),
        action: KeyAction::Release,
        mods: keymap::mods(ns::flags(event)),
        ..Key::default()
    });
}

/// A modifier key pressed or let go, for a program that asked to hear them; Command lights the
/// link under the pointer.
pub(super) fn flags_changed(view: &TermView, event: &NSEvent) {
    super::links::flags(view, ns::flags(event));
    let Some(surface) = surface(view) else {
        return;
    };
    let code = keymap::code(ns::key_code(event));
    let flags = ns::flags(event);
    let Some(down) = keymap::held_now(code, flags) else {
        return;
    };
    surface.key(&Key {
        code,
        action: if down {
            KeyAction::Press
        } else {
            KeyAction::Release
        },
        mods: keymap::mods(flags),
        ..Key::default()
    });
}

pub(super) fn copy(view: &TermView) {
    if let Some(text) = surface(view).and_then(|surface| surface.copy()) {
        crate::browser::platform::copy(&text);
    }
}

pub(super) fn paste(view: &TermView) {
    if let Some(surface) = surface(view) {
        crate::browser::platform::paste_into(move |text| surface.paste(&text));
    }
}

pub(super) fn select_all(view: &TermView) {
    if let Some(surface) = surface(view) {
        surface.select_all();
        view.redraw(false);
    }
}
