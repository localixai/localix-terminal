//! Tabs in the one window (deck.rs) that show what a caller paints: a terminal session's screen.
//! A panel shows the page its caller gives and runs the frames painted into it, and hands what
//! the person types in it to its caller's keyboard (a terminal session's input); it never
//! navigates past its own page.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use tao::event_loop::EventLoopProxy;
use wry::{PageLoadEvent, WebView, WebViewBuilder};

use super::choice::Entry;
use super::frame::{self, Look};
use super::host::Cmd;
use super::tab_view::{build_own, own_page};

/// What a sender does with a command: false once the loop is gone. Shared, as a painter is
/// (the loop's proxy is).
pub(super) type Post = Box<dyn Fn(Cmd) -> bool + Send + Sync>;

/// What a panel's page posts: keys the person typed or pasted (`k` and them), its size once it
/// has loaded (`r COLS ROWS`) and when its pane changes (`s COLS ROWS`), an address they opened
/// (`o URL`), a program's copy (`c BASE64`, OSC 52), or its menu asked for where they
/// right-clicked, and whether something is selected (`m X Y 1`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Said<'a> {
    Typed(&'a str),
    Sized(Pane, u16, u16),
    /// An http or https address the person opened from the page's text, for a tab of its own.
    Open(&'a str),
    /// The page's menu, where the person right-clicked: its own items.
    Menu(u32, u32, Vec<Entry>),
    /// The text a program asked to put on the pasteboard (OSC 52), at most `COPIED_MAX` bytes of
    /// UTF-8; the window decides whether it goes there.
    Copied(String),
}

/// The most a program puts on the pasteboard at once: more than any selection a person makes in
/// a terminal, and far less than a program could flood the app with.
pub(super) const COPIED_MAX: usize = 1 << 20;

/// What a program's copy says, base64 with or without its padding: its text, if it is text and
/// no longer than `COPIED_MAX`. Longer is not decoded at all.
fn copied(payload: &str) -> Option<String> {
    use base64::engine::{self, DecodePaddingMode, GeneralPurposeConfig};
    if payload.is_empty() || payload.len() > COPIED_MAX.div_ceil(3) * 4 {
        return None;
    }
    let lenient =
        GeneralPurposeConfig::new().with_decode_padding_mode(DecodePaddingMode::Indifferent);
    let engine = engine::GeneralPurpose::new(&base64::alphabet::STANDARD, lenient);
    let bytes = base64::Engine::decode(&engine, payload).ok()?;
    if bytes.len() > COPIED_MAX {
        return None;
    }
    String::from_utf8(bytes).ok()
}

/// Where the person's keys are, as a program's copy asks it: the window reads it from its deck
/// and the system (`Frame::is_key`, `platform::has_keys`); a check gives it as it likes, with no
/// window the system made key.
pub(super) trait Keys {
    /// Whether terminal `id` is in sight: the tab shown, or a half of the split.
    fn in_sight(&self, id: u32) -> bool;
    /// Whether the window is the key one: the person's keys come to it.
    fn key_window(&self) -> bool;
    /// Whether terminal `id`'s page holds the window's keys (its first responder).
    fn typing(&self, id: u32) -> bool;
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

/// What a page's size message says of the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pane {
    /// The page just loaded, and has drawn nothing yet.
    Loaded,
    /// The pane changed size.
    Resized,
}

/// What `body`, a message of a panel's page, says; none when it says nothing known.
pub(super) fn said(body: &str) -> Option<Said<'_>> {
    if let Some(typed) = body.strip_prefix('k') {
        return Some(Said::Typed(typed));
    }
    for (prefix, pane) in [("r ", Pane::Loaded), ("s ", Pane::Resized)] {
        if let Some(size) = body.strip_prefix(prefix) {
            let (columns, rows) = size.split_once(' ')?;
            return Some(Said::Sized(pane, columns.parse().ok()?, rows.parse().ok()?));
        }
    }
    if let Some(address) = body.strip_prefix("o ") {
        let web = ["http://", "https://"].into_iter().any(|scheme| {
            address
                .get(..scheme.len())
                .is_some_and(|start| start.eq_ignore_ascii_case(scheme))
        });
        return web.then_some(Said::Open(address));
    }
    if let Some(payload) = body.strip_prefix("c ") {
        return copied(payload).map(Said::Copied);
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

/// What a page's size is taken by: the pane's change, and its columns and rows.
pub type Fitted = Arc<dyn Fn(Pane, u16, u16) + Send + Sync>;

/// A panel's unrun frames are kept to this size; past it the oldest go, and the caller is told
/// (`Panel::dropped`) so it paints the screen whole again, as a terminal does.
const PENDING_MAX: usize = 8 << 20;

/// What a panel and the loop share: the frames not yet run, whether a paint is queued, whether the
/// window is still open, and where what the person types goes.
#[derive(Default)]
pub(super) struct Slot {
    frames: Mutex<String>,
    typed: Mutex<Option<Typed>>,
    sized: Mutex<Option<Fitted>>,
    queued: AtomicBool,
    /// Frames were dropped (`PENDING_MAX`) since the caller last asked.
    dropped: AtomicBool,
    pub(super) alive: AtomicBool,
    /// Out of sight: its page still runs what it is sent, only less often (`HIDDEN_EVERY`).
    hidden: AtomicBool,
}

/// How often a panel out of sight runs what it was sent: its screen stays live for whoever
/// reads it, at a fraction of the work.
const HIDDEN_EVERY: std::time::Duration = std::time::Duration::from_millis(250);

impl Slot {
    /// The frames painted since the last, one script, for the loop: a paint after this one
    /// queues again. Output is incremental, so none is skipped; a page that loads late asks the
    /// caller for the screen (`Pane::Loaded`).
    pub(super) fn latest(&self) -> Option<String> {
        self.queued.store(false, Ordering::SeqCst);
        let frames = std::mem::take(&mut *crate::lock(&self.frames));
        (!frames.is_empty()).then_some(frames)
    }

    /// The panel in sight (`shown`) or out of it; back in sight, whether frames wait to run now.
    pub(super) fn shown(&self, shown: bool) -> bool {
        let was_hidden = self.hidden.swap(!shown, Ordering::SeqCst);
        shown && was_hidden && !crate::lock(&self.frames).is_empty()
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

    /// What paints this panel, held apart from it: for a caller that finds the panel under a
    /// lock and paints once it has let the lock go.
    pub(super) fn brush(&self) -> Brush {
        Brush {
            id: self.id,
            slot: Arc::clone(&self.slot),
            send: Arc::clone(&self.send),
        }
    }

    /// Whether frames were dropped since this was last asked: the page is behind what was painted.
    pub fn dropped(&self) -> bool {
        self.slot.dropped.swap(false, Ordering::SeqCst)
    }

    /// Whether its tab is still the window's.
    pub fn alive(&self) -> bool {
        self.slot.alive.load(Ordering::SeqCst)
    }

    /// Hand the page's size, when it loads and when its pane changes, to `sized`.
    pub fn sizing(&self, sized: Fitted) {
        *crate::lock(&self.slot.sized) = Some(sized);
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
            slot.dropped.store(true, Ordering::SeqCst);
        }
        frames.push_str(frame);
        frames.push(';');
    }
    if slot.queued.swap(true, Ordering::SeqCst) {
        return slot.alive.load(Ordering::SeqCst);
    }
    if slot.hidden.load(Ordering::SeqCst) {
        // Out of sight: what comes meanwhile runs with it, a while later (or as it shows).
        let (send, later_slot) = (Arc::clone(send), Arc::clone(slot));
        let later = std::thread::Builder::new()
            .name("desk-paint-later".into())
            .spawn(move || {
                #[expect(
                    clippy::disallowed_methods,
                    reason = "the wait is the point: out of sight, a panel runs its frames \
                              a few times a second"
                )]
                std::thread::sleep(HIDDEN_EVERY);
                // The loop gone, nothing is left to paint for: the caller is told.
                if !send(Cmd::Paint(id)) {
                    later_slot.alive.store(false, Ordering::SeqCst);
                }
            });
        if later.is_ok() {
            return slot.alive.load(Ordering::SeqCst);
        }
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
    pub webview: WebView,
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

/// What a panel opens with: its id, title, page, and what it shares with its caller.
pub(super) type Opening = (u32, String, String, Arc<Slot>);

/// Open a panel as a tab of `native`, the window, in its `look`.
pub(super) fn open(
    (id, title, html, slot): Opening,
    native: &tao::window::Window,
    look: Look,
    proxy: EventLoopProxy<Cmd>,
) -> Result<Open, String> {
    let titles = proxy.clone();
    let keys = Arc::clone(&slot);
    let menus = proxy.clone();
    let opens = proxy.clone();
    let copies = proxy.clone();
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
            Some(Said::Sized(pane, columns, rows)) => {
                let sized = crate::lock(&keys.sized).clone();
                if let Some(sized) = sized {
                    sized(pane, columns, rows);
                }
            }
            Some(Said::Open(url)) => {
                let _ = opens.send_event(Cmd::PanelLink(id, url.to_owned()));
            }
            Some(Said::Menu(x, y, items)) => {
                let _ = menus.send_event(Cmd::PanelMenu(id, (x, y), items));
            }
            Some(Said::Copied(text)) => {
                let _ = copies.send_event(Cmd::PanelCopy(id, text));
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
        webview,
        slot,
        held: Held::Live,
    })
}

#[cfg(test)]
#[path = "panel_tests.rs"]
mod tests;
