//! The pointer, the focus and a paste as a program asked to hear them: a pointer event encoded
//! by the core's mouse encoder in the tracking mode and format the program set (X10, UTF-8, SGR,
//! URxvt, SGR pixels), the focus reported only while the program asked (mode 1004), and pasted
//! text made safe (control bytes that could act become spaces) and bracketed while the program
//! asked (mode 2004), its line breaks made returns where it did not.

use ghostty_vt_sys as vt;

use super::Core;
use super::keys::{encoded, mods};
use crate::surface::{Button, Geometry, Mouse, MouseAction};

impl Core {
    /// What the pointer event `mouse` sends the program in a view of `geometry`, `held` while a
    /// button is down; nothing when the program does not ask for it.
    pub fn encode_mouse(&mut self, mouse: &Mouse, geometry: &Geometry, held: bool) -> Vec<u8> {
        let term = self.term;
        let Some((encoder, event)) = self.handles.mouse() else {
            return vec![];
        };
        // SAFETY: a live encoder takes a live terminal's tracking mode and format.
        let _ = unsafe { vt::ghostty_mouse_encoder_set_from_terminal(encoder, term) };
        let size = vt::MouseEncoderSize {
            screen_width: geometry.width,
            screen_height: geometry.height,
            cell_width: geometry.cell_width.max(1),
            cell_height: geometry.cell_height.max(1),
            padding_top: geometry.pad_top,
            padding_bottom: geometry.pad_bottom,
            padding_right: geometry.pad_right,
            padding_left: geometry.pad_left,
            ..vt::sized!(vt::MouseEncoderSize)
        };
        set(encoder, vt::MouseEncoderOption::SIZE, &size);
        set(encoder, vt::MouseEncoderOption::ANY_BUTTON_PRESSED, &held);
        set(encoder, vt::MouseEncoderOption::TRACK_LAST_CELL, &true);
        let action = match mouse.action {
            MouseAction::Press => vt::MouseAction::PRESS,
            MouseAction::Release => vt::MouseAction::RELEASE,
            MouseAction::Motion => vt::MouseAction::MOTION,
        };
        // SAFETY: each setter writes one field of a live event.
        let _ = unsafe { vt::ghostty_mouse_event_set_action(event, action) };
        match mouse.button {
            // SAFETY: as above.
            Some(button) => unsafe { vt::ghostty_mouse_event_set_button(event, button_of(button)) },
            // SAFETY: as above.
            None => unsafe { vt::ghostty_mouse_event_clear_button(event) },
        };
        // SAFETY: as above.
        let _ = unsafe { vt::ghostty_mouse_event_set_mods(event, mods(mouse.mods)) };
        let at = vt::MousePosition {
            x: mouse.x,
            y: mouse.y,
        };
        // SAFETY: as above.
        let _ = unsafe { vt::ghostty_mouse_event_set_position(event, at) };
        encoded(|buf, len, written| {
            // SAFETY: a live encoder and event; the buffer has `len` bytes of room.
            unsafe { vt::ghostty_mouse_encoder_encode(encoder, event, buf, len, written) }
        })
    }

    /// What the program is told of the view taking (`on`) or losing the keys: nothing unless it
    /// asked (mode 1004).
    pub fn focus_report(&self, on: bool) -> Vec<u8> {
        if !self.mode(1004) {
            return vec![];
        }
        let event = if on {
            vt::FocusEvent::GAINED
        } else {
            vt::FocusEvent::LOST
        };
        encoded(|buf, len, written| {
            // SAFETY: the buffer has `len` bytes of room.
            unsafe { vt::ghostty_focus_encode(event, buf, len, written) }
        })
    }

    /// `text` as a paste sends it to the program now.
    pub fn paste_bytes(&self, text: &str) -> Vec<u8> {
        let bracketed = self.bracketed_paste();
        // Unframed, each line's end is a Return: a CR LF is one, as terminals send it.
        let mut data = match bracketed {
            true => text.as_bytes().to_vec(),
            false => text.replace("\r\n", "\r").into_bytes(),
        };
        let (from, len) = (data.as_mut_ptr().cast(), data.len());
        encoded(|buf, room, written| {
            // SAFETY: `data` is the paste's own copy, changed in place and alive through the
            // call; the buffer has `room` bytes of room.
            unsafe { vt::ghostty_paste_encode(from, len, bracketed, buf, room, written) }
        })
    }

    /// Whether the wheel on the other screen is sent as cursor keys (mode 1007), for a program
    /// that does not take the mouse.
    pub fn alternate_scroll(&self) -> bool {
        self.mode(1007)
    }
}

fn set<T>(encoder: vt::MouseEncoder, option: vt::MouseEncoderOption::Type, value: &T) {
    let value: *const T = value;
    // SAFETY: the option reads the type `value` points to during the call.
    let _ = unsafe { vt::ghostty_mouse_encoder_set(encoder, option, value.cast()) };
}

fn button_of(button: Button) -> vt::MouseButton::Type {
    match button {
        Button::Left => vt::MouseButton::LEFT,
        Button::Right => vt::MouseButton::RIGHT,
        Button::Middle => vt::MouseButton::MIDDLE,
        Button::WheelUp => vt::MouseButton::FOUR,
        Button::WheelDown => vt::MouseButton::FIVE,
        Button::WheelLeft => vt::MouseButton::SIX,
        Button::WheelRight => vt::MouseButton::SEVEN,
    }
}

#[cfg(test)]
#[path = "mouse_tests.rs"]
mod tests;
