//! A choice put to the person as GTK's own menu (Linux), where the pointer is: what the strip's
//! buttons offer ("+": a page or a terminal; the history). A menu pops over the pages, so nothing
//! of the window's layout changes for it; it answers the item picked, or none.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use gtk::gdk::prelude::SeatExt;
use gtk::glib;
use gtk::prelude::{
    Cast, CheckMenuItemExt, GtkMenuExt, GtkMenuExtManual, GtkMenuItemExt, MenuShellExt, WidgetExt,
};
use wry::{WebView, WebViewExtUnix};

/// Show `items` as a menu at the pointer, over `strip`'s window, and wait for the person: the
/// index of the item picked, or none when they dismissed it.
pub(super) fn choose(strip: &WebView, items: &[&str], marked: Option<usize>) -> Option<usize> {
    let menu = gtk::Menu::new();
    // The index picked, `usize::MAX` for none.
    let picked = Arc::new(AtomicUsize::new(usize::MAX));
    for (at, label) in items.iter().enumerate() {
        // The item chosen now (the page's size, commenting) is checked, as on macOS. Checked
        // before the handler below is connected: checking an item activates it.
        let entry: gtk::MenuItem = if marked == Some(at) {
            let chosen = gtk::CheckMenuItem::with_label(label);
            chosen.set_active(true);
            chosen.upcast()
        } else {
            gtk::MenuItem::with_label(label)
        };
        let picked = Arc::clone(&picked);
        entry.connect_activate(move |_| picked.store(at, Ordering::SeqCst));
        menu.append(&entry);
    }
    menu.set_attach_widget(Some(&strip.webview()));
    menu.show_all();
    // A menu is closed before its item acts: the wait ends once both are done.
    let waiting = glib::MainLoop::new(None, false);
    let done = waiting.clone();
    menu.connect_deactivate(move |_| {
        let done = done.clone();
        glib::idle_add_local_once(move || done.quit());
    });
    // Under the pointer, its right edge there: the buttons are at the window's right, and a
    // menu that ran to the right would leave it. The strip's click reached here as a message,
    // not a GDK event, so the pointer is asked where it is.
    let view = strip.webview();
    let pointer = view
        .window()
        .zip(
            view.display()
                .default_seat()
                .and_then(|seat| seat.pointer()),
        )
        .map(|(window, pointer)| {
            let (_, x, y, _) = window.device_position(&pointer);
            (window, x, y)
        });
    if let Some((window, x, y)) = pointer {
        let under = gtk::gdk::Rectangle::new(x, y + 12, 1, 1);
        menu.popup_at_rect(
            &window,
            &under,
            gtk::gdk::Gravity::SouthEast,
            gtk::gdk::Gravity::NorthEast,
            None,
        );
    }
    // Where that took no grab (no event under way), the old call at the pointer, which does.
    if !menu.is_visible() {
        menu.popup_easy(1, gtk::current_event_time());
    }
    if !menu.is_visible() {
        return None;
    }
    waiting.run();
    let at = picked.load(Ordering::SeqCst);
    (at < items.len()).then_some(at)
}
