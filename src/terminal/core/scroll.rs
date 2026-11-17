//! The view of a core's history: scrolled by rows (the wheel, a key), to the top or the bottom,
//! or to a row (the scrollbar dragged); and how much memory the core's pages take.

use ghostty_vt_sys as vt;

use super::Core;
use crate::surface::ScrollTo;

impl Core {
    /// The view moves `rows` toward history (more than 0) or back toward the screen.
    pub fn scroll(&mut self, rows: isize) {
        self.scroll_viewport(vt::TerminalScrollViewport {
            tag: vt::TerminalScrollViewportTag::DELTA,
            value: vt::TerminalScrollViewportValue { delta: -rows },
        });
    }

    pub fn scroll_to(&mut self, to: ScrollTo) {
        let (tag, value) = match to {
            ScrollTo::Top => (
                vt::TerminalScrollViewportTag::TOP,
                vt::TerminalScrollViewportValue { row: 0 },
            ),
            ScrollTo::Bottom => (
                vt::TerminalScrollViewportTag::BOTTOM,
                vt::TerminalScrollViewportValue { row: 0 },
            ),
            ScrollTo::Row(row) => (
                vt::TerminalScrollViewportTag::ROW,
                vt::TerminalScrollViewportValue {
                    row: usize::try_from(row).unwrap_or(usize::MAX),
                },
            ),
        };
        self.scroll_viewport(vt::TerminalScrollViewport { tag, value });
    }

    fn scroll_viewport(&mut self, to: vt::TerminalScrollViewport) {
        // SAFETY: a scroll of a live terminal's view, the value read by its tag.
        unsafe { vt::ghostty_terminal_scroll_viewport(self.term, to) };
    }

    /// The bytes the core's pages hold in memory now.
    pub fn memory(&self) -> u64 {
        let mut usage = vt::sized!(vt::TerminalMemoryUsage);
        let out: *mut vt::TerminalMemoryUsage = &raw mut usage;
        // SAFETY: a sized struct of the type the usage is written as.
        let _ = unsafe {
            vt::ghostty_terminal_get(self.term, vt::TerminalData::MEMORY_USAGE, out.cast())
        };
        usage.primary_resident_bytes
    }
}
