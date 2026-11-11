//! The window's own Settings, a tab of the side as a terminal is (`s1`): opened by its shortcut
//! or the side's gear, in the project shown; what the person changes there is kept (settings.rs)
//! and the whole window takes it at once: the menus' shortcuts, the theme, the glass, the fonts.
//! Its Integrations are setup.rs's, each read and turned on a thread of its own (they run the
//! person's programs), the page told when they are done.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};

use super::super::{Browser, Cmd, Shared};
use super::On;
use super::clicks::act;
use crate::browser::panel::Panel;
use crate::browser::settings;
use crate::browser::strip::{Click, Key, SETTINGS};

/// The page, as settings.html is.
const PAGE: &str = include_str!("settings.html");
/// The SHA-384 of the page's one script: the page's policy runs it alone.
const SCRIPT_SHA384: &str = "zGzETdws/tyS8VphVBKZirpNZd6vf9+l6JGv5t/V6Ke0NOPq3BJWykiG/nGwbveV";

/// The page, its script run by its hash.
fn page() -> String {
    PAGE.replacen("{{hash}}", SCRIPT_SHA384, 1)
}

impl On<'_> {
    /// Settings shown: its tab made the first time, and kept (hidden when the person closes it)
    /// after. It is the window's own, of no project: any project shows it, and closing one never
    /// closes it.
    pub(super) fn settings(&mut self) {
        let key = Key::Term(SETTINGS);
        crate::lock(&self.shared.groups).insert(key.name(), String::new());
        let alive = crate::lock(&self.shared.settings)
            .as_ref()
            .is_some_and(|panel| panel.alive());
        if alive && act(self.deck, Click::Show(key), self.shared).is_ok() {
            integrations(Arc::downgrade(self.shared), self.proxy.clone(), None);
            return;
        }
        // Gone with the window: the old one's end reaches the loop before the new one's start.
        let mut kept = crate::lock(&self.shared.settings);
        drop(kept.take());
        let browser = Browser {
            proxy: self.proxy.clone(),
            shared: Arc::clone(self.shared),
        };
        let panel = browser.panel(SETTINGS, "Settings", page());
        let (shared, proxy) = (Arc::downgrade(self.shared), self.proxy.clone());
        panel.keyboard(Arc::new(move |posted: &str| {
            changed(&shared, &proxy, posted)
        }));
        panel.paint(&script(&settings::now(), None));
        *kept = Some(panel);
        drop(kept);
        integrations(Arc::downgrade(self.shared), self.proxy.clone(), None);
    }

    /// The settings changed: every page and the menus take them.
    pub(super) fn restyled(&mut self) {
        #[cfg(target_os = "macos")]
        crate::browser::app_menu::install(self.proxy);
        self.restyle_deck();
    }

    /// This window's pages take the settings as they are now.
    pub(super) fn restyle_deck(&mut self) {
        if let Some(open) = self.deck.as_mut() {
            open.restyle(&settings::now());
        }
    }
}

/// The page's call that draws `settings`, with what was wrong with the last change.
fn script(settings: &settings::Settings, error: Option<&str>) -> String {
    format!(
        "P({},{})",
        settings.page(),
        serde_json::to_string(&error).unwrap_or_else(|_| "null".into())
    )
}

/// The page's call that says `text` for a moment, as an error or not.
fn toast(text: &str, error: bool) -> String {
    format!(
        "T({},{error})",
        serde_json::to_string(text).unwrap_or_else(|_| "\"\"".into())
    )
}

/// Whether a plain read of the integrations is under way (see [`integrations`]).
static READING: AtomicBool = AtomicBool::new(false);

/// A plain read under way: held by it, and let go when it ends, however it ends.
struct Reading;

impl Reading {
    /// The read, when none is under way.
    fn start() -> Option<Self> {
        (!READING.swap(true, Ordering::SeqCst)).then_some(Self)
    }
}

impl Drop for Reading {
    fn drop(&mut self) {
        READING.store(false, Ordering::SeqCst);
    }
}

/// The integrations read again, after `turning` one of them (its id, and on or off), on a thread
/// of its own; the page shows them, and says how the turn went.
fn integrations(
    shared: Weak<Shared>,
    proxy: tao::event_loop::EventLoopProxy<Cmd>,
    turning: Option<(String, bool)>,
) {
    // A plain read already under way gives the page what it asks for: Settings shown again (a
    // shortcut pressed twice) never starts another run of the person's programs. A turn always
    // runs, and reads after it.
    let reading = if turning.is_none() {
        match Reading::start() {
            Some(reading) => Some(reading),
            None => return,
        }
    } else {
        None
    };
    let read = std::thread::Builder::new()
        .name("desk-integrations".into())
        .spawn(move || {
            let _reading = reading;
            let turned = turning.map(|(id, on)| crate::setup::turn(&id, on));
            let states = crate::setup::states();
            let Some(shared) = shared.upgrade() else {
                return;
            };
            // One reading for both: the bar offers what Settings shows as off, at once.
            shared.offers.read_all(&states);
            let _ = proxy.send_event(Cmd::Strip);
            // Painted with the lock let go: the window's loop takes it too.
            let brush = crate::lock(&shared.settings).as_ref().map(Panel::brush);
            if let Some(panel) = brush {
                panel.paint(&format!("I({states})"));
                match turned {
                    Some(Ok(done)) => drop(panel.paint(&toast(&done, false))),
                    Some(Err(why)) => drop(panel.paint(&toast(&why, true))),
                    None => {}
                }
            }
        });
    if let Err(why) = read {
        log::warn!("settings: the integrations were not read (no thread: {why})");
    }
}

/// What the person did on the page (JSON): an integration turned, the MCP entry copied, else a
/// change of the settings (settings.rs `change`), kept, the page drawn again and the window
/// told. It runs on the window's thread, as the page's message comes: a change is a small file
/// written, and an integration's work goes to a thread of its own.
fn changed(shared: &Weak<Shared>, proxy: &tao::event_loop::EventLoopProxy<Cmd>, posted: &str) {
    let asked: serde_json::Value = serde_json::from_str(posted).unwrap_or_default();
    if let Some(id) = asked["integration"].as_str() {
        let on = asked["on"].as_bool().unwrap_or(true);
        return integrations(
            Weak::clone(shared),
            proxy.clone(),
            Some((id.to_string(), on)),
        );
    }
    let Some(shared) = shared.upgrade() else {
        return;
    };
    if asked["copy"] == "mcp" {
        let said = match crate::setup::mcp_entry() {
            Ok(entry) => {
                crate::browser::platform::copy(&entry);
                toast(
                    "The MCP entry is copied: paste it into your host's MCP servers.",
                    false,
                )
            }
            Err(why) => toast(&why, true),
        };
        let brush = crate::lock(&shared.settings).as_ref().map(Panel::brush);
        if let Some(panel) = brush {
            panel.paint(&said);
        }
        return;
    }
    let change = serde_json::from_str(posted).map_err(|why| why.to_string());
    let (now, error) = match change.and_then(|change| settings::change(&change)) {
        Ok(now) => (now, None),
        Err(why) => (settings::now(), Some(why)),
    };
    let brush = crate::lock(&shared.settings).as_ref().map(Panel::brush);
    if let Some(panel) = brush {
        panel.paint(&script(&now, error.as_deref()));
    }
    if error.is_none() {
        let _ = proxy.send_event(Cmd::Restyle);
    }
}

#[cfg(test)]
#[path = "host_settings_tests.rs"]
mod tests;
