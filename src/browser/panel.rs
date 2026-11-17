//! Tabs in the one window (deck.rs) that show what a caller paints: the board, the Store, an
//! editor, Settings, a terminal's tab. A panel shows the page its caller gives and runs the frames
//! painted into it, and hands what the person does in it to its caller's keyboard; it never
//! navigates past its own page. A terminal's tab is a panel whose view the window draws itself
//! from the terminal (`panel/face.rs`): its caller wakes it, and it draws the terminal's newest
//! frame.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use tao::event_loop::EventLoopProxy;
use wry::{PageLoadEvent, WebViewBuilder};

use super::choice::Entry;
use super::frame::{self, Look};
use super::host::Cmd;
use super::tab_view::{build_own, own_page};

mod face;
pub(super) use face::{Body, Face};

/// What a sender does with a command: false once the loop is gone. Shared, as a painter is
/// (the loop's proxy is).
pub(super) type Post = Box<dyn Fn(Cmd) -> bool + Send + Sync>;

/// What a panel's page posts: what the person did (`k` and it), that it has loaded (`r` and a
/// size it once gave, read no more), an address they opened (`o URL`), or its menu asked for
/// where they right-clicked (`m X Y ITEMS`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Said<'a> {
    Typed(&'a str),
    Loaded,
    /// An http or https address the person opened from the page's text, for a tab of its own.
    Open(&'a str),
    /// The page's menu, where the person right-clicked: its own items.
    Menu(u32, u32, Vec<Entry>),
}

/// Where the person looks and types, as a finished command's notice, a program's copy and its
/// bell ask it: the window reads it from its deck and the system (`Frame::is_key`); a check
/// gives it as it likes, with no window the system made key.
pub(super) trait Keys {
    /// Whether terminal `id` is in sight: the tab shown, or a pane of the split.
    fn in_sight(&self, id: u32) -> bool;
    /// Whether the window is the key one: the person's keys come to it.
    fn key_window(&self) -> bool;
    /// Whether terminal `id`'s view holds the window's keys.
    fn typing(&self, id: u32) -> bool;
}

/// Whether a command the person typed in terminal `id`, which ran `took`, ended while they were
/// not looking, as a finished command's notice asks it: it ran at least `after` (none: they want
/// no such notice), and the terminal was not in sight in the key window of the app in use
/// (`active`; `keys` none: no window shows it). Asked once, as the shell says it ended.
pub(super) fn unattended(
    keys: Option<&dyn Keys>,
    id: u32,
    took: std::time::Duration,
    after: Option<std::time::Duration>,
    active: bool,
) -> bool {
    let looking = keys.is_some_and(|keys| active && keys.key_window() && keys.in_sight(id));
    after.is_some_and(|after| took >= after) && !looking
}

/// Whether `address` is one a tab of the window opens: http or https. A terminal's link to
/// anything else (a file, another app's scheme) is not followed.
pub(super) fn web(address: &str) -> bool {
    ["http://", "https://"].into_iter().any(|scheme| {
        address
            .get(..scheme.len())
            .is_some_and(|start| start.eq_ignore_ascii_case(scheme))
    })
}

/// A program's copy from terminal `id` (OSC 52), given to `write` (the pasteboard) only while
/// that terminal is in sight and has the person's keys in the key window: a program out of
/// sight, or in a pane the person is not typing in, cannot change what they paste next.
/// Whether it was written.
pub(super) fn copy_where(keys: &dyn Keys, id: u32, text: &str, write: impl FnOnce(&str)) -> bool {
    let here = keys.in_sight(id) && keys.key_window() && keys.typing(id);
    if here {
        write(text);
    }
    here
}

/// What `body`, a message of a panel's page, says; none when it says nothing known.
pub(super) fn said(body: &str) -> Option<Said<'_>> {
    if let Some(typed) = body.strip_prefix('k') {
        return Some(Said::Typed(typed));
    }
    if body == "r" || body.starts_with("r ") {
        return Some(Said::Loaded);
    }
    if let Some(address) = body.strip_prefix("o ") {
        return web(address).then_some(Said::Open(address));
    }
    // "m x y [items]": the page's menu, the items as `choice::Entry` reads them.
    let mut parts = body.strip_prefix("m ")?.splitn(3, ' ');
    let x = parts.next()?.parse().ok()?;
    let y = parts.next()?.parse().ok()?;
    let items = serde_json::from_str(parts.next()?).ok()?;
    Some(Said::Menu(x, y, items))
}

/// What the person types in a panel, as its page sends it.
pub(super) type Typed = Arc<dyn Fn(&str) + Send + Sync>;

/// What hears that a panel's page has loaded: it has drawn nothing yet, and is painted whole.
pub type Loaded = Arc<dyn Fn() + Send + Sync>;

/// A panel's unrun frames are kept to this size; past it the older go. Each frame a caller
/// paints draws its page whole, so the newest is all the page needs.
const PENDING_MAX: usize = 8 << 20;

/// What a panel and the loop share: the frames not yet run, whether a paint is queued, whether the
/// window is still open, and where what the person does goes.
#[derive(Default)]
pub(super) struct Slot {
    frames: Mutex<String>,
    typed: Mutex<Option<Typed>>,
    loaded: Mutex<Option<Loaded>>,
    queued: AtomicBool,
    pub(super) alive: AtomicBool,
}

impl Slot {
    /// The frames painted since the last, one script, for the loop: a paint after this one
    /// queues again. A page that loads late asks its caller to paint it (`Panel::on_load`).
    pub(super) fn latest(&self) -> Option<String> {
        self.queued.store(false, Ordering::SeqCst);
        let frames = std::mem::take(&mut *crate::lock(&self.frames));
        (!frames.is_empty()).then_some(frames)
    }
}

/// A panel as its caller holds it: painted from any thread, closed when dropped.
pub struct Panel {
    id: u32,
    pub(super) slot: Arc<Slot>,
    send: Arc<Post>,
}

impl Panel {
    pub(super) fn new(id: u32, send: Post) -> Self {
        let slot = Slot {
            alive: AtomicBool::new(true),
            ..Slot::default()
        };
        Self {
            id,
            slot: Arc::new(slot),
            send: Arc::new(send),
        }
    }

    pub fn id(&self) -> u32 {
        self.id
    }

    /// Show `frame`, a script the page runs. At most one paint waits in the loop's queue, and it
    /// runs every frame painted since the last, in order. False once the window is closed.
    pub fn paint(&self, frame: &str) -> bool {
        paint(self.id, &self.slot, &self.send, frame)
    }

    /// What tells this panel to draw again, held apart from it: a terminal's, called as its
    /// screen changes. At most one waits in the loop's queue. False once the window is closed.
    pub fn waker(&self) -> impl Fn() -> bool + Send + Sync + 'static {
        let (id, slot, send) = (self.id, Arc::clone(&self.slot), Arc::clone(&self.send));
        move || {
            if slot.queued.swap(true, Ordering::SeqCst) {
                return slot.alive.load(Ordering::SeqCst);
            }
            if !send(Cmd::Paint(id)) {
                slot.alive.store(false, Ordering::SeqCst);
            }
            slot.alive.load(Ordering::SeqCst)
        }
    }

    /// What paints this panel, held apart from it: for a caller that finds the panel under a
    /// lock and paints once it has let the lock go.
    pub(super) fn brush(&self) -> Brush {
        Brush {
            id: self.id,
            slot: Arc::clone(&self.slot),
            send: Arc::clone(&self.send),
        }
    }

    /// Whether its tab is still the window's.
    pub fn alive(&self) -> bool {
        self.slot.alive.load(Ordering::SeqCst)
    }

    /// Tell `loaded` each time the page loads.
    pub fn on_load(&self, loaded: Loaded) {
        *crate::lock(&self.slot.loaded) = Some(loaded);
    }

    /// Hand what the person types in the panel to `typed`: until then it goes nowhere.
    pub fn keyboard(&self, typed: Typed) {
        *crate::lock(&self.slot.typed) = Some(typed);
    }
}

/// A panel's painter apart from the panel (`Panel::brush`): it paints as the panel does, and
/// closes nothing when it is dropped.
pub(super) struct Brush {
    id: u32,
    slot: Arc<Slot>,
    send: Arc<Post>,
}

impl Brush {
    pub(super) fn paint(&self, frame: &str) -> bool {
        paint(self.id, &self.slot, &self.send, frame)
    }
}

/// `frame` queued for panel `id`'s page: at most one paint waits in the loop's queue.
fn paint(id: u32, slot: &Arc<Slot>, send: &Arc<Post>, frame: &str) -> bool {
    {
        let mut frames = crate::lock(&slot.frames);
        if frames.len() + frame.len() > PENDING_MAX {
            frames.clear();
        }
        frames.push_str(frame);
        frames.push(';');
    }
    if slot.queued.swap(true, Ordering::SeqCst) {
        return slot.alive.load(Ordering::SeqCst);
    }
    if !send(Cmd::Paint(id)) {
        slot.alive.store(false, Ordering::SeqCst);
    }
    slot.alive.load(Ordering::SeqCst)
}

impl Drop for Panel {
    fn drop(&mut self) {
        (self.send)(Cmd::Unpanel(self.id));
    }
}

/// An open panel, as the loop keeps it: a tab of the window, titled as its page says.
pub(super) struct Open {
    pub id: u32,
    pub title: String,
    pub face: Face,
    pub slot: Arc<Slot>,
    pub held: Held,
}

/// How the window holds a panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Held {
    /// A tab of the window.
    Live,
    /// Its tab closed by the person: still painted, out of sight, until shown again.
    Hidden,
    /// Its caller let it go: its last frame stays, for the history menu to show.
    Ended,
}

/// What a panel opens with: its id, title, page or terminal, and what it shares with its caller.
pub(super) type Opening = (u32, String, Body, Arc<Slot>);

/// Open a panel as a tab of `native`, the window, in its `look`.
pub(super) fn open(
    (id, title, body, slot): Opening,
    native: &tao::window::Window,
    look: Look,
    proxy: EventLoopProxy<Cmd>,
) -> Result<Open, String> {
    let html = match body {
        Body::Page(html) => html,
        Body::Term(surface) => {
            // A link the person Command-clicks opens as an address they opened from its text.
            let opens = Box::new(move |address: &str| {
                if web(address) {
                    let _ = proxy.send_event(Cmd::PanelLink(id, address.to_owned()));
                }
            });
            let view = super::term_view::TermView::open(native, surface, opens)
                .ok_or("the terminal's view could not be made")?;
            return Ok(Open {
                id,
                title,
                face: Face::Term(view),
                slot,
                held: Held::Live,
            });
        }
    };
    let titles = proxy.clone();
    let keys = Arc::clone(&slot);
    let menus = proxy.clone();
    let opens = proxy.clone();
    let webview = own_page(WebViewBuilder::new())
        // The first click in a window that is not the key one reaches the page: the person
        // never clicks twice, once to wake the window and once to do what they meant.
        .with_accept_first_mouse(true)
        .with_html(frame::dress(&html, look))
        // The person's fonts, before the page's own scripts (settings.rs).
        .with_initialization_script(super::settings::look_script(&super::settings::now()))
        // Over glass, the page is clear: the window's material and coat are its ground.
        .with_transparent(look == Look::Glass)
        // Its own page is the only one it shows: what it posts is the person's. The keyboard is
        // copied out first: it writes to a terminal, never under the lock.
        .with_ipc_handler(move |posted| match said(posted.body()) {
            Some(Said::Typed(typed)) => {
                let keyboard = crate::lock(&keys.typed).clone();
                if let Some(keyboard) = keyboard {
                    keyboard(typed);
                }
            }
            Some(Said::Loaded) => {
                let loaded = crate::lock(&keys.loaded).clone();
                if let Some(loaded) = loaded {
                    loaded();
                }
            }
            Some(Said::Open(url)) => {
                let _ = opens.send_event(Cmd::PanelLink(id, url.to_owned()));
            }
            Some(Said::Menu(x, y, items)) => {
                let _ = menus.send_event(Cmd::PanelMenu(id, (x, y), items));
            }
            None => {}
        })
        .with_document_title_changed_handler(move |title| {
            let _ = titles.send_event(Cmd::Titled(id, title));
        })
        // Frames queued meanwhile run once the page can run them.
        .with_on_page_load_handler(move |event, _| {
            if matches!(event, PageLoadEvent::Finished) {
                let _ = proxy.send_event(Cmd::Paint(id));
            }
        });
    let webview = build_own(webview, native).map_err(|error| error.to_string())?;
    super::platform::keep_tab_rendering(&webview);
    Ok(Open {
        id,
        title,
        face: Face::Page(Box::new(webview)),
        slot,
        held: Held::Live,
    })
}

#[cfg(test)]
#[path = "panel_tests.rs"]
mod tests;
