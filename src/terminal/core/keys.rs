//! A key the person pressed made into the bytes the program asked for: the core's key encoder,
//! set from the modes the program set (cursor keys, the keypad, modifyOtherKeys, the keyboard
//! protocol whose flags a program pushes with `CSI > u`), with Option as Alt on the side the
//! person chose, and one rule of ours: Shift+Enter sends a line feed while no program asked for
//! the keyboard protocol, as coding agents read it for a new line in what is typed.

use ghostty_vt_sys as vt;

use super::{Core, done};
use crate::surface::{Key, KeyAction, KeyCode, Mods, OptionMeta};

impl Core {
    /// The keyboard protocol's flags the program set (0: it asked for none).
    pub fn keyboard_flags(&self) -> u8 {
        self.get(vt::TerminalData::KITTY_KEYBOARD_FLAGS)
    }

    /// What `key` sends the program now; nothing for a key that sends nothing (a modifier
    /// alone, a release the program did not ask to hear).
    pub fn encode_key(&mut self, key: &Key, meta: OptionMeta) -> Vec<u8> {
        let shift_enter =
            key.code == KeyCode::Enter && key.mods.0 & !Mods::SHIFT_RIGHT == Mods::SHIFT;
        if shift_enter && key.action != KeyAction::Release && self.keyboard_flags() == 0 {
            return b"\n".to_vec();
        }
        let term = self.term;
        let Some((encoder, event)) = self.handles.keys() else {
            return vec![];
        };
        // SAFETY: a live encoder takes the modes of a live terminal.
        let _ = unsafe { vt::ghostty_key_encoder_set_from_terminal(encoder, term) };
        let alt: vt::OptionAsAlt::Type = match meta {
            OptionMeta::Off => vt::OptionAsAlt::FALSE,
            OptionMeta::Left => vt::OptionAsAlt::LEFT,
            OptionMeta::Right => vt::OptionAsAlt::RIGHT,
            OptionMeta::Both => vt::OptionAsAlt::TRUE,
        };
        let value: *const vt::OptionAsAlt::Type = &alt;
        let option = vt::KeyEncoderOption::MACOS_OPTION_AS_ALT;
        // SAFETY: the option reads the value `value` points to during the call.
        let _ = unsafe { vt::ghostty_key_encoder_set(encoder, option, value.cast()) };
        fill(event, key);
        let bytes = encoded(|buf, len, written| {
            // SAFETY: a live encoder and event; the buffer has `len` bytes of room.
            unsafe { vt::ghostty_key_encoder_encode(encoder, event, buf, len, written) }
        });
        // The event keeps no copy of the key's text: it is cleared before `key` can go, so the
        // event that outlives this call points at nothing of it.
        let none = vt::String {
            ptr: std::ptr::null(),
            len: 0,
        };
        // SAFETY: a text of no length clears it; its pointer is never read.
        let _ = unsafe { vt::ghostty_key_event_set_utf8(event, none) };
        bytes
    }
}

/// Fills the core's key event from `key`; its text is borrowed, not copied, and the caller
/// clears it once the event is encoded.
fn fill(event: vt::KeyEvent, key: &Key) {
    let action = match key.action {
        KeyAction::Press => vt::KeyAction::PRESS,
        KeyAction::Repeat => vt::KeyAction::REPEAT,
        KeyAction::Release => vt::KeyAction::RELEASE,
    };
    // SAFETY: each setter writes one field of a live event.
    let _ = unsafe { vt::ghostty_key_event_set_action(event, action) };
    // SAFETY: as above.
    let _ = unsafe { vt::ghostty_key_event_set_key(event, code(key.code)) };
    // SAFETY: as above.
    let _ = unsafe { vt::ghostty_key_event_set_mods(event, mods(key.mods)) };
    // SAFETY: as above.
    let _ = unsafe { vt::ghostty_key_event_set_consumed_mods(event, mods(key.consumed)) };
    // SAFETY: as above.
    let _ = unsafe { vt::ghostty_key_event_set_composing(event, key.composing) };
    let text = vt::String::from(key.text.as_str());
    // SAFETY: the event holds `key`'s text without copying it: it is read only by the encode
    // that follows, while `key` lives, and cleared before `encode_key` returns.
    let _ = unsafe { vt::ghostty_key_event_set_utf8(event, text) };
    let unshifted = key.unshifted.map_or(0, u32::from);
    // SAFETY: as above.
    let _ = unsafe { vt::ghostty_key_event_set_unshifted_codepoint(event, unshifted) };
}

/// What `encode` writes, tried in 128 bytes and again in the room it asks for.
pub(super) fn encoded(
    mut encode: impl FnMut(*mut u8, usize, *mut usize) -> vt::Result::Type,
) -> Vec<u8> {
    let mut bytes = vec![0u8; 128];
    let mut written = 0usize;
    let mut result = encode(bytes.as_mut_ptr(), bytes.len(), &raw mut written);
    if result == vt::Result::OUT_OF_SPACE {
        bytes.resize(written, 0);
        result = encode(bytes.as_mut_ptr(), bytes.len(), &raw mut written);
    }
    match done(result, "to encode") {
        Ok(()) => {
            bytes.truncate(written);
            bytes
        }
        Err(_) => vec![],
    }
}

/// Our modifiers as the core's (the same bits, side bits set for the right one).
pub(super) fn mods(mods: Mods) -> vt::Mods {
    let pairs = [
        (Mods::SHIFT, 1 << 0),
        (Mods::CTRL, 1 << 1),
        (Mods::ALT, 1 << 2),
        (Mods::SUPER, 1 << 3),
        (Mods::CAPS_LOCK, 1 << 4),
        (Mods::NUM_LOCK, 1 << 5),
        (Mods::SHIFT_RIGHT, 1 << 6),
        (Mods::CTRL_RIGHT, 1 << 7),
        (Mods::ALT_RIGHT, 1 << 8),
        (Mods::SUPER_RIGHT, 1 << 9),
    ];
    pairs
        .iter()
        .filter(|(ours, _)| mods.has(*ours))
        .fold(0, |all, (_, theirs)| all | theirs)
}

/// A key by its place, as the core names it.
fn code(code: KeyCode) -> vt::Key::Type {
    use vt::Key as K;
    let offset = |base: vt::Key::Type, n: u8, most: u8| match n <= most {
        true => base + vt::Key::Type::from(n),
        false => K::UNIDENTIFIED,
    };
    match code {
        KeyCode::Letter(c) => offset(K::A, c.wrapping_sub(b'a'), 25),
        KeyCode::Digit(n) => offset(K::DIGIT_0, n, 9),
        KeyCode::F(n) => offset(K::F1, n.wrapping_sub(1), 24),
        KeyCode::Numpad(n) => offset(K::NUMPAD_0, n, 9),
        KeyCode::Unidentified => K::UNIDENTIFIED,
        KeyCode::Backquote => K::BACKQUOTE,
        KeyCode::Backslash => K::BACKSLASH,
        KeyCode::BracketLeft => K::BRACKET_LEFT,
        KeyCode::BracketRight => K::BRACKET_RIGHT,
        KeyCode::Comma => K::COMMA,
        KeyCode::Equal => K::EQUAL,
        KeyCode::Minus => K::MINUS,
        KeyCode::Period => K::PERIOD,
        KeyCode::Quote => K::QUOTE,
        KeyCode::Semicolon => K::SEMICOLON,
        KeyCode::Slash => K::SLASH,
        KeyCode::IntlBackslash => K::INTL_BACKSLASH,
        KeyCode::Backspace => K::BACKSPACE,
        KeyCode::Tab => K::TAB,
        KeyCode::Enter => K::ENTER,
        KeyCode::Space => K::SPACE,
        KeyCode::Escape => K::ESCAPE,
        KeyCode::Delete => K::DELETE,
        KeyCode::Insert => K::INSERT,
        KeyCode::Home => K::HOME,
        KeyCode::End => K::END,
        KeyCode::PageUp => K::PAGE_UP,
        KeyCode::PageDown => K::PAGE_DOWN,
        KeyCode::ArrowUp => K::ARROW_UP,
        KeyCode::ArrowDown => K::ARROW_DOWN,
        KeyCode::ArrowLeft => K::ARROW_LEFT,
        KeyCode::ArrowRight => K::ARROW_RIGHT,
        KeyCode::NumpadAdd => K::NUMPAD_ADD,
        KeyCode::NumpadSubtract => K::NUMPAD_SUBTRACT,
        KeyCode::NumpadMultiply => K::NUMPAD_MULTIPLY,
        KeyCode::NumpadDivide => K::NUMPAD_DIVIDE,
        KeyCode::NumpadDecimal => K::NUMPAD_DECIMAL,
        KeyCode::NumpadEnter => K::NUMPAD_ENTER,
        KeyCode::NumpadEqual => K::NUMPAD_EQUAL,
        KeyCode::CapsLock => K::CAPS_LOCK,
        KeyCode::ShiftLeft => K::SHIFT_LEFT,
        KeyCode::ShiftRight => K::SHIFT_RIGHT,
        KeyCode::ControlLeft => K::CONTROL_LEFT,
        KeyCode::ControlRight => K::CONTROL_RIGHT,
        KeyCode::AltLeft => K::ALT_LEFT,
        KeyCode::AltRight => K::ALT_RIGHT,
        KeyCode::MetaLeft => K::META_LEFT,
        KeyCode::MetaRight => K::META_RIGHT,
        KeyCode::ContextMenu => K::CONTEXT_MENU,
    }
}

#[cfg(test)]
#[path = "keys_tests.rs"]
mod tests;
