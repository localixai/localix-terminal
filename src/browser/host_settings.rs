//! The window's own Settings, a tab of the side as a terminal is (`s1`): opened by its shortcut
//! or the side's gear, in the project shown; what the person changes there is kept (settings.rs)
//! and the whole window takes it at once: the menus' shortcuts, the theme, the glass, the fonts.
//! Its Updates are update.rs's: the page is told whenever how far an update is changed, and the
//! checks made for the person start here.

use std::sync::{Arc, Weak};

use super::super::{Browser, Cmd, Shared};
use super::On;
use super::clicks::act;
use crate::browser::panel::Panel;
use crate::browser::settings;
use crate::browser::strip::{Click, Key, SETTINGS};
use crate::browser::updates;

/// The page, as settings.html is.
const PAGE: &str = include_str!("settings.html");
/// The SHA-384 of the page's one script: the page's policy runs it alone.
const SCRIPT_SHA384: &str = "ItPi0uRMvgqjxNRoHYtgeQMJsunZt42X9pzj8crvRE86VD+dL2NSYGhWDA5AXNDj";

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
            return paint_update(self.shared);
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
        panel.paint(&update_script());
        *kept = Some(panel);
    }

    /// The app menu's Check for Updates…: Settings shown, where it is said how the check goes.
    pub(super) fn check_for_updates(&mut self) {
        self.settings();
        crate::update::app().check();
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

/// The page's call that draws how far the app's update is.
fn update_script() -> String {
    let row = updates::row(&crate::update::app().told(), crate::now());
    format!(
        "U({})",
        serde_json::to_string(&row).unwrap_or_else(|_| "null".into())
    )
}

/// Settings, when it is open, told how far the app's update is.
fn paint_update(shared: &Shared) {
    // Painted with the lock let go: the window's loop takes it too.
    let brush = crate::lock(&shared.settings).as_ref().map(Panel::brush);
    if let Some(panel) = brush {
        panel.paint(&update_script());
    }
}

/// The app's updates from here on: Settings and the offer bar are told whenever how far one is
/// changed, and one is looked for while the person's settings say to. Never for windows the
/// person does not see (a test's).
pub(super) fn watch_updates(shared: &Arc<Shared>, proxy: &tao::event_loop::EventLoopProxy<Cmd>) {
    if crate::browser::platform::unseen() {
        return;
    }
    let (shared, proxy) = (Arc::downgrade(shared), proxy.clone());
    crate::update::app().on_change(move || {
        if let Some(shared) = shared.upgrade() {
            paint_update(&shared);
        }
        let _ = proxy.send_event(Cmd::Strip);
    });
    crate::update::app().watch(|| settings::now().updates == updates::Updates::Auto);
}

/// What the person did in the page's Updates: a check, the restart into what is ready, or the
/// notes of a release, opened as a page of theirs in the project shown.
fn update(shared: &Shared, asked: &str) {
    match asked {
        "check" => crate::update::app().check(),
        "apply" => shared.heard("", crate::browser::Heard::Update),
        version => {
            let group = crate::lock(&shared.scope).shown().to_string();
            let page = crate::browser::Asked::Page(Some(updates::notes(version)));
            shared.heard(&group, crate::browser::Heard::Asked(page));
        }
    }
}

/// What the person did on the page (JSON): an update asked for, else a change of the settings
/// (settings.rs `change`), kept, the page drawn again and the window told. It runs on the
/// window's thread, as the page's message comes: a change is a small file written.
fn changed(shared: &Weak<Shared>, proxy: &tao::event_loop::EventLoopProxy<Cmd>, posted: &str) {
    let asked: serde_json::Value = serde_json::from_str(posted).unwrap_or_default();
    let Some(shared) = shared.upgrade() else {
        return;
    };
    if let Some(asked) = asked["update"].as_str() {
        return update(&shared, asked);
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
