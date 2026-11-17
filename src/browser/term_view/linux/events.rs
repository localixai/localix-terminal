//! The keys the person presses in the view, told to the terminal: read by the input method first
//! (GTK's IM context: what it composes stays the view's, drawn at the cursor; what it commits is
//! typed), the view's own Super keys, Alt as Meta as the terminal page had it (the core sends ESC
//! and the key); Copy, Paste and Select All; and focus. The pointer is `pointer.rs`.

use std::sync::Arc;

use gtk::gdk;
use gtk::glib::Propagation;
use gtk::prelude::{Cast, GtkWindowExt, IMContextExt, WidgetExt};

use super::super::acts::Command;
use super::view::{View, handled};
use super::{acts, hover, keymap};
use crate::surface::{Key, KeyAction, KeyCode, Mods, ScrollTo, Surface};

/// The view's keys, its focus and its input method heard.
pub(super) fn connect(view: &View) {
    let (area, make) = (&view.area, view.maker());
    let made = make.clone();
    area.connect_key_press_event(move |area, event| handled(key_down(&made(area), event)));
    let made = make.clone();
    area.connect_key_release_event(move |area, event| handled(key_up(&made(area), event)));
    let made = make.clone();
    area.connect_focus_in_event(move |area, _| {
        focused(&made(area), true);
        Propagation::Proceed
    });
    area.connect_focus_out_event(move |area, _| {
        focused(&make(area), false);
        Propagation::Proceed
    });
    let weak = view.weak();
    view.im.connect_commit(move |_, text| {
        if let Some(view) = weak() {
            commit(&view, text);
        }
    });
    let weak = view.weak();
    view.im.connect_preedit_changed(move |im| {
        if let Some(view) = weak() {
            compose(&view, &im.preedit_string().0);
        }
    });
}

/// The view's place in the window's keys changed (`on`: it has them now): the input method and
/// the program are told (the program when it asked to be, once for each change), and the view is
/// drawn as the one in use or not.
fn focused(view: &View, on: bool) {
    if on {
        view.im.focus_in();
    } else {
        view.im.focus_out();
    }
    let key = view.key_window();
    told(view, on && key, key);
}

/// The view is the one in use now (`active`) or not, in a window that is the active one (`key`)
/// or not: its program told when that changed, and the view drawn so.
fn told(view: &View, active: bool, key: bool) {
    let was = view.state().map(|state| state.active);
    if was != Some(active)
        && let Some(surface) = view.surface()
    {
        surface.focus(active);
    }
    view.activate(active, key);
}

/// For checks: the keys given to the view (`on`), or its window's keys given to nothing, in a
/// window taken as the active one (a check's run is never the app in front).
pub(super) fn focus_probe(view: &View, on: bool) {
    if on {
        view.focus();
    } else if let Some(window) =
        (view.area.toplevel()).and_then(|top| top.downcast::<gtk::Window>().ok())
    {
        window.set_focus(None::<&gtk::Widget>);
    }
    told(view, on, true);
}

/// The window became the active one or stopped being it.
pub(super) fn window_key_changed(view: &View) {
    focused(view, view.has_keys());
}

/// A key pressed: whether the view took it.
fn key_down(view: &View, event: &gdk::EventKey) -> bool {
    let Some(surface) = view.surface() else {
        return false;
    };
    let hardware = event.hardware_keycode();
    let code = keymap::code(hardware);
    let Some((mods, repeat)) = view.state().map(|mut state| {
        if let Some((bit, right)) = keymap::side(code) {
            state.sides = if right {
                state.sides | bit
            } else {
                state.sides & !bit
            };
        }
        let repeat = state.down == Some(hardware);
        state.down = Some(hardware);
        (keymap::mods(event.state().bits(), state.sides), repeat)
    }) else {
        return false;
    };
    let key = Key {
        code,
        action: if repeat {
            KeyAction::Repeat
        } else {
            KeyAction::Press
        },
        mods,
        unshifted: unshifted(&view.area, event),
        ..Key::default()
    };
    if matches!(code, KeyCode::MetaLeft | KeyCode::MetaRight) {
        hover::mods(view, Mods(mods.0 | Mods::SUPER));
    }
    if mods.has(Mods::SUPER) && !mods.has(Mods::CTRL) {
        return command(view, &*surface, &key);
    }
    typed(view, &*surface, event, &key)
}

/// A key read by the input method first: what it composes stays the view's, what it commits is
/// typed, and a key it leaves alone is encoded as the program asked (Alt held, the core sends
/// ESC and the key's character).
fn typed(view: &View, surface: &dyn Surface, event: &gdk::EventKey, key: &Key) -> bool {
    let had_marked = view.state().is_some_and(|state| !state.marked.is_empty());
    if let Some(mut state) = view.state() {
        state.committed = Some(String::new());
    }
    let filtered = view.im.filter_keypress(event);
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
        return true;
    }
    let text = match (committed.is_empty(), filtered || key.mods.has(Mods::CTRL)) {
        (false, _) => acts::printable(&committed),
        (true, true) => String::new(),
        (true, false) => acts::printable(
            &event
                .keyval()
                .to_unicode()
                .map(String::from)
                .unwrap_or_default(),
        ),
    };
    if filtered && text.is_empty() {
        return true;
    }
    let key = Key {
        consumed: acts::consumed(&text, key.mods, Mods::SHIFT),
        text: text.clone(),
        ..key.clone()
    };
    let sent = surface.key(&key);
    if !sent && !text.is_empty() {
        surface.text(&text);
    }
    sent || !text.is_empty()
}

/// The character `event`'s key makes with no modifier, in its layout.
fn unshifted(area: &gtk::DrawingArea, event: &gdk::EventKey) -> Option<char> {
    let keymap = gdk::Keymap::for_display(&area.display())?;
    let hardware = u32::from(event.hardware_keycode());
    let group = i32::from(event.group());
    let (keyval, ..) =
        keymap.translate_keyboard_state(hardware, gdk::ModifierType::empty(), group)?;
    // SAFETY: GDK's mapping of a key value to its character (0 for none), which reads nothing
    // but its argument.
    let character = unsafe { gdk::ffi::gdk_keyval_to_unicode(keyval) };
    char::from_u32(character).filter(|c| *c != '\0' && !c.is_control())
}

/// A Super key: the view's own (Super+K clears, Super+↑ and Super+↓ go to the top and the
/// bottom) or a Mac terminal's (Super+←, →, ⌫, ⌦), as the terminal page had them; anything else
/// is GTK's. Whether the view took it.
fn command(view: &View, surface: &dyn Surface, key: &Key) -> bool {
    match acts::command(key) {
        Command::Send => return surface.key(key),
        Command::Clear => surface.clear(),
        Command::Top => surface.scroll_to(ScrollTo::Top),
        Command::Bottom => surface.scroll_to(ScrollTo::Bottom),
        Command::Nothing => return false,
    }
    view.redraw(false);
    true
}

/// A key let go: for a program that asked to hear it, unless an input method composes.
fn key_up(view: &View, event: &gdk::EventKey) -> bool {
    if view.im.filter_keypress(event) {
        return true;
    }
    let hardware = event.hardware_keycode();
    let code = keymap::code(hardware);
    let Some((mods, marked)) = view.state().map(|mut state| {
        if let Some((bit, true)) = keymap::side(code) {
            state.sides &= !bit;
        }
        if state.down == Some(hardware) {
            state.down = None;
        }
        (
            keymap::mods(event.state().bits(), state.sides),
            !state.marked.is_empty(),
        )
    }) else {
        return false;
    };
    if matches!(code, KeyCode::MetaLeft | KeyCode::MetaRight) {
        hover::mods(view, Mods(mods.0 & !Mods::SUPER));
    }
    let Some(surface) = view.surface().filter(|_| !marked) else {
        return marked;
    };
    surface.key(&Key {
        code,
        action: KeyAction::Release,
        mods,
        ..Key::default()
    })
}

/// Text the input method committed: handed to the key being read, else typed now.
pub(super) fn commit(view: &View, text: &str) {
    let surface = {
        let Some(mut state) = view.state() else {
            return;
        };
        state.marked.clear();
        match state.committed.as_mut() {
            Some(committed) => {
                committed.push_str(text);
                None
            }
            None => Some(Arc::clone(&state.surface)),
        }
    };
    if let Some(surface) = surface {
        surface.text(text);
        view.redraw(false);
    }
}

/// What the input method composes now, drawn at the cursor, its candidates standing by it.
pub(super) fn compose(view: &View, text: &str) {
    let cell = view.state().and_then(|mut state| {
        state.marked = text.to_string();
        let cell = state.painter.as_ref()?.cell();
        acts::cursor_cell(&state.frame, cell)
    });
    if let Some([x, y, width, height]) = cell {
        let whole = |n: f64| {
            #[expect(
                clippy::cast_possible_truncation,
                reason = "a place in a view, in points"
            )]
            let n = n.round() as i32;
            n
        };
        let at = gdk::Rectangle::new(whole(x), whole(y), whole(width), whole(height));
        view.im.set_cursor_location(&at);
    }
    view.redraw(false);
}

pub(super) fn copy(view: &View) {
    if let Some(text) = view.surface().and_then(|surface| surface.copy()) {
        crate::browser::platform::copy(&text);
    }
}

/// The clipboard's text pasted once it comes, into the view's terminal if the view is still
/// there and can be told anything then.
pub(super) fn paste(view: &View) {
    let view = view.weak();
    crate::browser::platform::paste_into(move |text| {
        if let Some(surface) = view().and_then(|view| view.surface()) {
            surface.paste(&text);
        }
    });
}

pub(super) fn select_all(view: &View) {
    if let Some(surface) = view.surface() {
        surface.select_all();
        view.redraw(false);
    }
}
