//! A tab transferred between native windows, keeping its view and viewport as they were.

use std::collections::HashMap;

use super::super::host::{Shared, Window};
use super::super::popup;
use super::super::sizes::Frame;
use super::{Slot, Tabs};

/// A page on its way to another window: its view as it is, moved and not loaded again
/// (docs/windows.md), with what its window kept of it.
pub(in super::super) struct Carried {
    slot: Slot,
    frame: Option<Frame>,
    window: Window,
}

impl Carried {
    /// How its window opened its pages: one with none of its own yet opens them the same.
    pub(in super::super) fn window(&self) -> &Window {
        &self.window
    }
}

impl Tabs {
    /// No page yet, in `native`: its pages come from other windows.
    pub(in super::super) fn empty(native: &tao::window::Window, window: &Window) -> Self {
        // Joined, the pages end with `tabs` (its drop), as an opened one's do; the places stay.
        popup::join();
        Self {
            slots: vec![],
            active: 1,
            window: window.clone(),
            frames: HashMap::new(),
            native: native.id(),
        }
    }

    /// Page `id` leaves for another window, its view as it is: the page acted on here is the one
    /// before it, as when it closes.
    pub(in super::super) fn take(&mut self, id: u32, shared: &Shared) -> Option<Carried> {
        let at = (self.slots.iter()).position(|slot| slot.id == id && slot.open)?;
        let slot = self.slots.remove(at);
        let frame = self.frames.remove(&id);
        let ids = self.ids();
        if self.active == id
            && let Some(last) = ids.last()
        {
            self.active = (ids.iter().rev().find(|other| **other < id)).map_or(*last, |one| *one);
        }
        shared.set_tabs(self.native, self.active, &ids);
        Some(Carried {
            slot,
            frame,
            window: self.window.clone(),
        })
    }

    /// A page come from another window: its view is put in `native`, this window, as it was,
    /// and it is the page acted on.
    pub(in super::super) fn take_in(
        &mut self,
        carried: Carried,
        native: &tao::window::Window,
        shared: &Shared,
    ) {
        super::super::platform::reparent(&carried.slot.webview, native);
        let id = carried.slot.id;
        if let Some(frame) = carried.frame {
            self.frames.insert(id, frame);
        }
        self.slots.push(carried.slot);
        self.active = id;
        shared.set_tabs(self.native, self.active, &self.ids());
    }
}
