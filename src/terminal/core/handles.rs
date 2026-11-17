//! The core's encoders and its selection gesture: the key and mouse encoders with an event each
//! to fill, and the gesture with its press, drag and release events. Each is made the first time
//! a view needs it (a terminal no one looks at never makes one) and freed with the core, the
//! gesture before its terminal.

use std::ptr;

use ghostty_vt_sys as vt;

#[derive(Debug)]
pub(super) struct Handles {
    key_encoder: vt::KeyEncoder,
    key_event: vt::KeyEvent,
    mouse_encoder: vt::MouseEncoder,
    mouse_event: vt::MouseEvent,
    gesture: vt::SelectionGesture,
    /// The gesture's events: a press, a drag and a release.
    gestures: [vt::SelectionGestureEvent; 3],
}

impl Default for Handles {
    fn default() -> Self {
        Self {
            key_encoder: ptr::null_mut(),
            key_event: ptr::null_mut(),
            mouse_encoder: ptr::null_mut(),
            mouse_event: ptr::null_mut(),
            gesture: ptr::null_mut(),
            gestures: [ptr::null_mut(); 3],
        }
    }
}

/// `handle`, made by `make` if it is not yet; none when the library could not make it.
fn made<T>(
    handle: &mut *mut T,
    make: impl FnOnce(*mut *mut T) -> vt::Result::Type,
) -> Option<*mut T> {
    if handle.is_null() && make(handle) != vt::Result::SUCCESS {
        *handle = ptr::null_mut();
    }
    (!handle.is_null()).then_some(*handle)
}

impl Handles {
    pub(super) fn keys(&mut self) -> Option<(vt::KeyEncoder, vt::KeyEvent)> {
        // SAFETY: a null allocator is the library's default; the handle is written on success.
        let encoder = made(&mut self.key_encoder, |out| unsafe {
            vt::ghostty_key_encoder_new(ptr::null(), out)
        })?;
        // SAFETY: as above.
        let event = made(&mut self.key_event, |out| unsafe {
            vt::ghostty_key_event_new(ptr::null(), out)
        })?;
        Some((encoder, event))
    }

    pub(super) fn mouse(&mut self) -> Option<(vt::MouseEncoder, vt::MouseEvent)> {
        // SAFETY: as above.
        let encoder = made(&mut self.mouse_encoder, |out| unsafe {
            vt::ghostty_mouse_encoder_new(ptr::null(), out)
        })?;
        // SAFETY: as above.
        let event = made(&mut self.mouse_event, |out| unsafe {
            vt::ghostty_mouse_event_new(ptr::null(), out)
        })?;
        Some((encoder, event))
    }

    /// The gesture and its event of `kind` (a press, a drag or a release).
    pub(super) fn gesture(
        &mut self,
        kind: vt::SelectionGestureEventType::Type,
    ) -> Option<(vt::SelectionGesture, vt::SelectionGestureEvent)> {
        // SAFETY: as above.
        let gesture = made(&mut self.gesture, |out| unsafe {
            vt::ghostty_selection_gesture_new(ptr::null(), out)
        })?;
        let slot = match kind {
            vt::SelectionGestureEventType::PRESS => 0,
            vt::SelectionGestureEventType::DRAG => 1,
            _ => 2,
        };
        let event = self.gestures.get_mut(slot)?;
        // SAFETY: as above, an event of the kind asked for.
        let event = made(event, |out| unsafe {
            vt::ghostty_selection_gesture_event_new(ptr::null(), out, kind)
        })?;
        Some((gesture, event))
    }

    /// Frees what was made; the gesture with the terminal it was used with, before that goes.
    pub(super) fn free(&mut self, terminal: vt::Terminal) {
        if !self.key_event.is_null() {
            // SAFETY: made by `keys`, freed once.
            unsafe { vt::ghostty_key_event_free(self.key_event) };
        }
        if !self.key_encoder.is_null() {
            // SAFETY: as above.
            unsafe { vt::ghostty_key_encoder_free(self.key_encoder) };
        }
        if !self.mouse_event.is_null() {
            // SAFETY: made by `mouse`, freed once.
            unsafe { vt::ghostty_mouse_event_free(self.mouse_event) };
        }
        if !self.mouse_encoder.is_null() {
            // SAFETY: as above.
            unsafe { vt::ghostty_mouse_encoder_free(self.mouse_encoder) };
        }
        for event in self.gestures.into_iter().filter(|event| !event.is_null()) {
            // SAFETY: made by `gesture`, freed once.
            unsafe { vt::ghostty_selection_gesture_event_free(event) };
        }
        if !self.gesture.is_null() {
            // SAFETY: made by `gesture`, freed once with the terminal it was used with.
            unsafe { vt::ghostty_selection_gesture_free(self.gesture, terminal) };
        }
        *self = Self::default();
    }
}
