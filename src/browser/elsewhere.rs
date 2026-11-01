//! A click in another app, while the deck is expanded: the person went back to their own work,
//! and the deck collapses to the mini. The deck is a panel that never takes the app's focus
//! (nspanel.rs), so it has no focus to lose; a click elsewhere is what losing it means. AppKit's
//! global monitor sees only the events meant for other apps (a click in the deck or the mini is
//! the deck's own) and needs no permission for the mouse. `LLX_DESK_KEEP_EXPANDED=1` keeps
//! the deck open beside the person's work; an unseen window (a test's) is never watched.

use tao::event_loop::EventLoopProxy;

use super::host::Cmd;

/// The setting that keeps the deck expanded when the person clicks elsewhere.
const KEEP: &str = "LLX_DESK_KEEP_EXPANDED";

/// Whether a click elsewhere collapses the deck here.
fn wanted() -> bool {
    !super::platform::unseen() && std::env::var_os(KEEP).is_none_or(|v| v.is_empty() || v == "0")
}

/// The watch, while it lives.
#[cfg(target_os = "macos")]
pub(super) struct Elsewhere(objc2::rc::Retained<objc2::runtime::AnyObject>);

#[cfg(target_os = "macos")]
impl Elsewhere {
    /// Watches for a click in another app, told to `proxy` as a collapse; none when unwanted or
    /// when AppKit gives no monitor.
    pub(super) fn watch(proxy: &EventLoopProxy<Cmd>) -> Option<Self> {
        use block2::RcBlock;
        use objc2::rc::Retained;
        use objc2::runtime::AnyObject;
        use objc2::{class, msg_send};

        /// NSEventMask: a press of the left, the right or another mouse button.
        const PRESSES: u64 = (1 << 1) | (1 << 3) | (1 << 25);
        if !wanted() {
            return None;
        }
        let proxy = proxy.clone();
        let block = RcBlock::new(move |_event: *mut AnyObject| {
            let _ = proxy.send_event(Cmd::Presence(super::presence::Event::Collapse));
        });
        // SAFETY: a class method taking a mask and a block of `void (^)(NSEvent *)`, answering
        // the monitor (autoreleased, retained here), or nil; AppKit keeps the block.
        let monitor: Option<Retained<AnyObject>> = unsafe {
            msg_send![class!(NSEvent), addGlobalMonitorForEventsMatchingMask: PRESSES, handler: &*block]
        };
        monitor.map(Self)
    }
}

#[cfg(target_os = "macos")]
impl Drop for Elsewhere {
    fn drop(&mut self) {
        use objc2::{class, msg_send};
        // SAFETY: the monitor `addGlobalMonitor…` answered, removed once.
        let () = unsafe { msg_send![class!(NSEvent), removeMonitor: &*self.0] };
    }
}

/// Elsewhere there is no mini to collapse to.
#[cfg(not(target_os = "macos"))]
pub(super) struct Elsewhere;

#[cfg(not(target_os = "macos"))]
impl Elsewhere {
    pub(super) fn watch(_proxy: &EventLoopProxy<Cmd>) -> Option<Self> {
        let _ = wanted();
        None
    }
}
