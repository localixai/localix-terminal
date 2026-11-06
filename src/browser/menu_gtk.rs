//! A choice put to the person as GTK's own menu (Linux), where the pointer is: what the strip's
//! buttons offer ("+": a page or a terminal; the history). A menu pops over the pages, so nothing
//! of the window's layout changes for it; it answers the item picked, or none. A question (go on,
//! or Cancel) is GTK's own dialog.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use gtk::gdk::prelude::SeatExt;
use gtk::glib;
use gtk::prelude::{
    Cast, CheckMenuItemExt, GtkMenuExt, GtkMenuExtManual, GtkMenuItemExt, MenuShellExt, WidgetExt,
};
use wry::{WebView, WebViewExtUnix};

use super::choice::Entry;

/// Show `entries` as a menu at the pointer, over `strip`'s window, and wait for the person: the
/// place of the item picked among them (`choice::flat`), or none when they dismissed it.
pub(super) fn show(strip: &WebView, entries: &[Entry]) -> Option<usize> {
    let menu = gtk::Menu::new();
    // The index picked, `usize::MAX` for none.
    let picked = Arc::new(AtomicUsize::new(usize::MAX));
    fill(&menu, entries, &picked, &mut 0);
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

/// `entries` added to `menu`, their items numbered from `next` on.
fn fill(menu: &gtk::Menu, entries: &[Entry], picked: &Arc<AtomicUsize>, next: &mut usize) {
    for entry in entries {
        match entry {
            Entry::Line { .. } => menu.append(&gtk::SeparatorMenuItem::new()),
            Entry::Sub { label, items } => {
                let sub = gtk::Menu::new();
                fill(&sub, items, picked, next);
                let opens = gtk::MenuItem::with_label(label);
                opens.set_submenu(Some(&sub));
                menu.append(&opens);
            }
            Entry::Item {
                label, marked, off, ..
            } => {
                let at = *next;
                *next += 1;
                // The item chosen now is checked, as on macOS; checked before the handler below
                // is connected: checking an item activates it.
                let item: gtk::MenuItem = if *marked {
                    let chosen = gtk::CheckMenuItem::with_label(label);
                    chosen.set_active(true);
                    chosen.upcast()
                } else {
                    gtk::MenuItem::with_label(label)
                };
                item.set_sensitive(!off);
                let picked = Arc::clone(picked);
                item.connect_activate(move |_| picked.store(at, Ordering::SeqCst));
                menu.append(&item);
            }
        }
    }
}

/// `question` asked of the person in GTK's dialog: whether they went on.
pub(super) fn confirm(question: &super::Confirm) -> bool {
    use gtk::prelude::{DialogExt, GtkWindowExt, MessageDialogExt};
    let dialog = gtk::MessageDialog::new(
        None::<&gtk::Window>,
        gtk::DialogFlags::MODAL,
        gtk::MessageType::Question,
        gtk::ButtonsType::None,
        &question.title,
    );
    dialog.set_secondary_text(Some(&question.detail));
    dialog.add_button("Cancel", gtk::ResponseType::Cancel);
    dialog.add_button(&question.yes, gtk::ResponseType::Accept);
    let answer = dialog.run();
    dialog.close();
    answer == gtk::ResponseType::Accept
}

/// Nothing outside the window quits the app here: its close does.
pub(super) fn quit_through(_proxy: &tao::event_loop::EventLoopProxy<super::host::Cmd>) {}

/// Elsewhere there is no menu bar to grey.
pub(super) fn page_shown(_shown: bool) {}
