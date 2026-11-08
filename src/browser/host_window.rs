//! What the person does to the window itself: closes it, resizes it, or changes the system's
//! appearance under it.

use tao::event::WindowEvent;
use tao::window::WindowId;

use super::super::deck::Deck;
use super::super::frame;
use super::presence::Presenter;
use super::{Heard, Shared};

/// Window `window_id`'s `event`, for its deck (the main window's when `main`): whether the
/// person closed a window that is not the main one (the loop closes it, and its tabs).
pub(super) fn on_window(
    (deck, presenter): (&mut Option<Deck>, &Presenter),
    window_id: WindowId,
    event: &WindowEvent<'_>,
    shared: &Shared,
    main: bool,
) -> bool {
    // The mini's own window: where the person moves it is where it stays.
    if presenter.owns(window_id) {
        if let WindowEvent::Moved(_) = event {
            presenter.mini_moved(shared);
        }
        return false;
    }
    let Some(open) = deck.as_mut().filter(|d| d.frame.native.id() == window_id) else {
        return false;
    };
    #[expect(
        clippy::wildcard_enum_match_arm,
        reason = "tao's events: foreign, non-exhaustive"
    )]
    match event {
        // The window is the app's own: closing it is quitting the app, which the desk asks the
        // person about when programs run, as ⌘Q does. (Its minimize button collapses it to the
        // mini instead, nspanel.rs.)
        WindowEvent::CloseRequested if main => shared.heard("", Heard::Quit),
        // Another window closed: it goes, and its tabs with it.
        WindowEvent::CloseRequested => return true,
        event if frame::relayout(event) => {
            // Full screen over: the system made the window's buttons anew, and the main
            // window's minimize button is the mini's again.
            #[cfg(target_os = "macos")]
            if open.frame.follow_full_screen() && main {
                crate::browser::nspanel::hook_collapse(&open.frame.native);
            }
            #[cfg(not(target_os = "macos"))]
            open.frame.follow_full_screen();
            open.tell_full_screen();
            open.layout();
            if main {
                presenter.deck_moved(open, event, shared);
            }
        }
        WindowEvent::Moved(_) if main => {
            open.frame.follow_full_screen();
            presenter.deck_moved(open, event, shared);
        }
        // The system's appearance changed: a glass that follows it is tinted again.
        WindowEvent::ThemeChanged(_) => open.frame.retint(),
        _ => {}
    }
    false
}
