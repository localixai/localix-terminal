//! What the loop does with the person's clicks and the window's presence: a tab shown or closed,
//! the history menu's entry, a page's answer, the mini named, its picture, an event of its state.

use std::sync::Arc;

use serde_json::Value;

use super::super::presence::Stage;
use super::super::{Cmd, Heard, MINI, Shared, WINDOW_CLOSED};
use super::On;
use crate::browser::capture::Shot;
use crate::browser::deck::{After, Deck, Revisit, Shown};
use crate::browser::presence::Event as Presence;
use crate::browser::strip::{Click, Key, SEARCH, click, went};

impl On<'_> {
    /// The mini names the tab shown again: its project may be known by now.
    pub(super) fn renamed(&mut self) {
        let stage = Stage {
            deck: &mut *self.deck,
            target: self.target,
            shared: self.shared,
            proxy: self.proxy,
        };
        self.presenter.rename(&stage);
    }

    pub(super) fn pictured(&mut self, shot: &Result<Shot, String>) {
        let mut stage = Stage {
            deck: &mut *self.deck,
            target: self.target,
            shared: self.shared,
            proxy: self.proxy,
        };
        self.presenter.pictured(shot, &mut stage);
    }

    /// `event` for the window's presence.
    pub(super) fn presence(&mut self, event: Presence) {
        let mut stage = Stage {
            deck: &mut *self.deck,
            target: self.target,
            shared: self.shared,
            proxy: self.proxy,
        };
        self.presenter.event(event, &mut stage);
    }

    /// The person's click: closing the browser's last tab is closing its window.
    pub(super) fn clicked(&mut self, said: &str) {
        if let Some(url) = went(said) {
            return self.go(url);
        }
        match click(said) {
            Some(Click::Visit(at)) => return self.visit(at),
            Some(Click::Collapse) => return self.presence(Presence::Collapse),
            Some(Click::NewTab(x, y)) => return self.plus(x, y),
            // GTK lays the strip and the pages in a column, so the strip's own list could only
            // take the pages' place: the history is the system's menu there.
            #[cfg(not(target_os = "macos"))]
            Some(Click::History) => return self.history_menu(),
            Some(Click::Menu(key, x, y)) => return self.tab_menu(key, (x, y)),
            // The strip says it on its own refreshes: answered without one.
            Some(Click::Glass(rect)) => {
                if let Some(open) = self.deck.as_ref() {
                    open.glass(rect);
                }
                return;
            }
            _ => {}
        }
        match click(said) {
            Some(Click::Close(key)) => self.close(key),
            Some(said) => {
                let _ = act(self.deck, said, self.shared, Some(self.on_close));
            }
            None => {}
        }
    }

    /// The person closes tab `key`, and whoever works in it is told.
    pub(super) fn close(&mut self, key: Key) {
        if act(
            self.deck,
            Click::Close(key),
            self.shared,
            Some(self.on_close),
        )
        .is_ok()
        {
            let what = match key {
                Key::Web(_) => CLOSED_PAGE,
                Key::Term(_) => CLOSED_TERMINAL,
            };
            self.shared.heard(&key.name(), Heard::Closed(what));
        }
    }

    /// What the person typed in the address field: the page shown goes there (a terminal shown,
    /// a new tab does), and whoever works in it is told.
    fn go(&mut self, url: String) {
        let Some(open) = self.deck.as_ref() else {
            return;
        };
        let web = open.web.as_ref().filter(|_| open.shown == Shown::Web);
        let Some(web) = web else {
            let _ = self.proxy.send_event(Cmd::NewTab { url, reply: None });
            return;
        };
        let _ = web.active().load_url(&url);
        let id = web.listed().1;
        let said = format!("The person went to {url} in this browser tab.");
        self.shared.heard(&Key::Web(id).name(), Heard::Did(&said));
    }

    /// "+": the system's menu under it, then a page on the search page, or a terminal of the
    /// person's in the project shown (the app starts it).
    fn plus(&mut self, x: u32, y: u32) {
        let Some(open) = self.deck.as_ref() else {
            return;
        };
        let at = (f64::from(x), f64::from(y));
        match open.choose(at, &["New Page", "New Terminal"]) {
            Some(0) => {
                let url = SEARCH.trim_end_matches("/search").to_string();
                let _ = self.proxy.send_event(Cmd::NewTab { url, reply: None });
            }
            Some(_) => {
                let listed = open.listing(self.shared);
                let shown = listed.iter().find(|tab| tab.shown);
                let group = shown.map(|tab| tab.group.clone()).unwrap_or_default();
                self.shared.heard(&group, Heard::Terminal);
            }
            None => {}
        }
    }

    /// The history as the system's menu, newest first; an entry picked is shown again.
    #[cfg(not(target_os = "macos"))]
    fn history_menu(&mut self) {
        use crate::browser::strip::State;
        let Some(open) = self.deck.as_ref() else {
            return;
        };
        let rows = open.rows(self.shared);
        let labels: Vec<String> = rows
            .iter()
            .map(|row| match row.state {
                State::Open => row.title.clone(),
                State::Hidden => format!("{} \u{2014} closed \u{b7} still running", row.title),
                State::Ended => format!("{} \u{2014} ended", row.title),
                State::Closed => format!("{} \u{2014} closed", row.title),
            })
            .collect();
        let mut items: Vec<&str> = labels.iter().map(String::as_str).collect();
        if items.is_empty() {
            items.push("No tabs yet");
        }
        if let Some(row) = open.choose((0.0, 0.0), &items).and_then(|at| rows.get(at)) {
            self.visit(row.at);
        }
    }

    /// An entry of the history menu shown again; a closed page opens in a tab of its own.
    fn visit(&mut self, at: u64) {
        let entry = self.shared.history.get(at);
        if let (Some(entry), Some(open)) = (entry, self.deck.as_mut()) {
            if let Revisit::Load(url) = open.revisit(&entry, self.shared) {
                let _ = self.proxy.send_event(Cmd::NewTab { url, reply: None });
            }
            open.refresh(self.shared);
        }
    }

    /// `script` in a panel's page, or in the strip's (`STRIP`, for checks).
    pub(super) fn eval_panel(&self, id: u64, script: &str, panel: u32) {
        let answer = Arc::clone(self.shared);
        let reply = move |value: String| {
            let value = serde_json::from_str(&value).unwrap_or(Value::String(value));
            answer.answer(id, Ok(value));
        };
        let page = match panel {
            MINI => self.presenter.page(),
            panel => self.deck.as_ref().and_then(|open| open.page(panel)),
        };
        match page {
            Some(page) => drop(page.evaluate_script_with_callback(script, reply)),
            None => self.shared.answer(id, Err(WINDOW_CLOSED.into())),
        }
    }
}

/// Show or close the tab `said` names. A person's click closing the browser's last tab stops the
/// task as closing its window does (`on_close`); the agent's goes on.
pub(super) fn act(
    deck: &mut Option<Deck>,
    said: Click,
    shared: &Shared,
    on_close: Option<fn()>,
) -> Result<(), String> {
    let open = deck.as_mut().ok_or("the window is not open")?;
    let after = open.act(said, shared)?;
    if let (After::BrowserGone { .. }, Some(on_close)) = (&after, on_close) {
        on_close();
    }
    match after {
        After::Empty | After::BrowserGone { empty: true } => *deck = None,
        After::Kept | After::BrowserGone { .. } => open.refresh(shared),
    }
    Ok(())
}

/// What the person's closing says, to whoever listens.
const CLOSED_PAGE: &str = "The person closed this browser tab.";
const CLOSED_TERMINAL: &str = "The person closed this tab: its program runs on, out of sight.";
