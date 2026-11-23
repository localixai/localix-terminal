//! A key the person pressed made into the bytes the program asked for: the core's key encoder,
//! set from the modes the program set (cursor keys, the keypad, modifyOtherKeys, the keyboard
//! protocol whose flags a program pushes with `CSI > u`), with Option as Alt on the side the
//! person chose. Control on a key whose text the input method left out is encoded by the key's
//! ASCII character, as the encoder reads Control (so modifyOtherKeys tells it apart).
//!
//! While a program asked for neither the keyboard protocol nor modifyOtherKeys, it reads plain
//! bytes, and the keys are sent as terminals send them to it: Return, Tab and Escape with
//! Control or Option, and Escape with Shift, are the keys themselves (Shift+Tab stays a back-tab),
//! and Control on a character is its control
//! byte (Ctrl+I a Tab, Ctrl+[ an Escape, Ctrl+Shift+A a ^A), where the encoder would tell them
//! apart in a form only those modes ask for. Option as Meta puts ESC before them. One rule is
//! ours: Shift+Enter sends a line feed, as coding agents read it for a new line in what is typed.

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
        let plain = self.keyboard_flags() == 0 && key.action != KeyAction::Release;
        let shift_enter =
            key.code == KeyCode::Enter && key.mods.0 & !Mods::SHIFT_RIGHT == Mods::SHIFT;
        if shift_enter && plain {
            return b"\n".to_vec();
        }
        let typed;
        let key = match control_text(key) {
            Some(text) => {
                typed = Key {
                    text,
                    ..key.clone()
                };
                &typed
            }
            None => key,
        };
        let bytes = self.encoded_key(key, meta);
        if !plain || !told_apart(&bytes) || self.modify_other_keys() {
            return bytes;
        }
        plain_bytes(key, meta).unwrap_or(bytes)
    }

    /// Whether the program turned on modifyOtherKeys (level 2), which the terminal does not say:
    /// the encoder then tells Ctrl+A from a plain ^A.
    fn modify_other_keys(&mut self) -> bool {
        let probe = Key {
            code: KeyCode::Letter(b'a'),
            mods: Mods(Mods::CTRL),
            text: "a".into(),
            unshifted: Some('a'),
            ..Key::default()
        };
        self.encoded_key(&probe, OptionMeta::Off) != [1]
    }

    /// What the core's encoder sends for `key`, with the terminal's modes and Option as Alt on
    /// the side `meta` says.
    fn encoded_key(&mut self, key: &Key, meta: OptionMeta) -> Vec<u8> {
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

/// The text a key held with Control is encoded by when the input method gave it none: its own
/// ASCII character (a key of another script is read by its place, as it is).
fn control_text(key: &Key) -> Option<String> {
    let unshifted = key
        .unshifted
        .filter(|c| c.is_ascii_graphic() || *c == ' ')?;
    (key.mods.has(Mods::CTRL) && key.text.is_empty()).then(|| unshifted.to_string())
}

/// Whether the encoder told a key apart in a form a program must ask for: the keyboard
/// protocol's (`CSI … u`), modifyOtherKeys' (`CSI 27;…~`), or nothing at all.
fn told_apart(sent: &[u8]) -> bool {
    let csi = sent.starts_with(b"\x1b[");
    sent.is_empty() || (csi && (sent.ends_with(b"u") || sent.starts_with(b"\x1b[27;")))
}

/// What `key` sends to a program that reads plain bytes: Return, Tab or Escape itself, or the
/// control byte of the character Control is held on; ESC before it for Option as Meta. None
/// for a key with Command, an Option that types its own character, or no control byte.
fn plain_bytes(key: &Key, meta: OptionMeta) -> Option<Vec<u8>> {
    let held = key.mods.0 & (Mods::SHIFT | Mods::CTRL | Mods::ALT | Mods::SUPER);
    let alt = held & Mods::ALT != 0;
    if held & Mods::SUPER != 0 || (alt && !meta.is_meta(key.mods)) {
        return None;
    }
    const KEYS: [(KeyCode, u8); 3] = [
        (KeyCode::Enter, b'\r'),
        (KeyCode::Tab, b'\t'),
        (KeyCode::Escape, 0x1b),
    ];
    let byte = match KEYS.iter().find(|(code, _)| *code == key.code) {
        Some((_, byte)) => *byte,
        None if held & Mods::CTRL != 0 => {
            let mut chars = key.text.chars();
            let typed = chars.next().filter(|_| chars.next().is_none());
            // The character typed, else the key's own, else (a layout of another script) the
            // one its place has in the US layout, as Ctrl+C is read under any layout.
            let ascii = |c: &char| c.is_ascii();
            let c = typed.filter(ascii).or_else(|| key.unshifted.filter(ascii));
            control_byte(c.or_else(|| place(key.code))?)?
        }
        None => return None,
    };
    Some(if alt { vec![0x1b, byte] } else { vec![byte] })
}

/// The character a key's place has in the US layout, for the keys Control makes a byte of.
fn place(code: KeyCode) -> Option<char> {
    const PLACES: [(KeyCode, char); 5] = [
        (KeyCode::BracketLeft, '['),
        (KeyCode::Backslash, '\\'),
        (KeyCode::BracketRight, ']'),
        (KeyCode::Slash, '/'),
        (KeyCode::Space, ' '),
    ];
    if let KeyCode::Letter(letter) = code {
        return Some(char::from(letter));
    }
    if let KeyCode::Digit(digit) = code {
        return char::from_digit(u32::from(digit), 10);
    }
    PLACES.iter().find(|(at, _)| *at == code).map(|(_, c)| *c)
}

/// The control byte Control makes of `c`: a letter's (either case), and those of the
/// punctuation and digits that have one.
fn control_byte(c: char) -> Option<u8> {
    const MORE: [(&str, u8); 7] = [
        ("@ 2", 0x00),
        ("[3", 0x1b),
        ("\\4", 0x1c),
        ("]5", 0x1d),
        ("^6~", 0x1e),
        ("_/7", 0x1f),
        ("?8", 0x7f),
    ];
    let byte = u8::try_from(c).ok().filter(u8::is_ascii)?;
    if byte.is_ascii_alphabetic() {
        return Some(byte.to_ascii_lowercase() - b'a' + 1);
    }
    MORE.iter()
        .find(|(chars, _)| chars.as_bytes().contains(&byte))
        .map(|(_, control)| *control)
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

#[cfg(test)]
#[path = "keys_spec_tests.rs"]
mod spec_tests;
