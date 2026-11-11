//! What the loop does with the person's clicks and the window's presence: a tab shown or closed,
//! the history menu's entry, a page's answer, an event of the window's state.

use std::sync::Arc;

use serde_json::Value;

use super::super::presence::{Presenter, Stage};
use super::super::{Asked, Cmd, Heard, Shared, WINDOW_CLOSED};
use super::On;
use crate::browser::deck::{Deck, Revisit, Shown};
use crate::browser::presence::Event as Presence;
use crate::browser::sizes;
use crate::browser::strip::{Click, Key, Tool, click, went};

impl On<'_> {
    /// The window's presenter, and the stage it works on: the window in use.
    fn staged(&mut self) -> (&mut Presenter, Stage<'_>) {
        let stage = Stage {
            deck: &mut *self.deck,
            shared: self.shared,
        };
        (&mut *self.presenter, stage)
    }

    /// `event` for the window's presence.
    pub(super) fn presence(&mut self, event: Presence) {
        let (presenter, mut stage) = self.staged();
        presenter.event(event, &mut stage);
    }

    /// The person opened the app: the window comes back; with nothing open yet, an empty one,
    /// its start of a new tab.
    pub(super) fn opened(&mut self) {
        if self.deck.is_none() {
            match Deck::empty((self.target, self.steps), self.shared, self.proxy) {
                Ok(deck) => {
                    deck.refresh(self.shared);
                    deck.frame.focus();
                    *self.deck = Some(deck);
                }
                Err(why) => log::warn!("no window for the person: {why}"),
            }
        }
        let (presenter, mut stage) = self.staged();
        presenter.opened(&mut stage);
    }

    /// Whether `said` is one of what a drag sends a frame at a time (the side's edge, the line
    /// between the halves, a tab carried over the pages, the strip's glass): it changes no tab
    /// shown, so the window is neither kept on its project nor drawn again for it.
    pub(super) fn streamed(said: &str) -> bool {
        matches!(
            click(said),
            Some(
                Click::SideWidth(..)
                    | Click::SplitMove(..)
                    | Click::DropOver(..)
                    | Click::DropOff
                    | Click::Glass(_)
            )
        )
    }

    /// The person's click. Closing the last tab leaves the window, with its start of a new tab.
    pub(super) fn clicked(&mut self, said: &str) {
        if let Some(url) = went(said) {
            return self.go(url);
        }
        if let Some(yes) = crate::browser::dialog::said(said) {
            return self.dialog_answered(yes);
        }
        if said == crate::browser::updates::CHECK {
            return self.check_for_updates();
        }
        match click(said) {
            Some(Click::Visit(at)) => return self.visit(at),
            Some(Click::Offer(take)) => return self.offer(take),
            Some(Click::Collapse) => return self.presence(Presence::Collapse),
            Some(Click::Quit) => return self.shared.heard("", Heard::Quit),
            Some(Click::NewTab(x, y)) => return self.plus(x, y),
            Some(Click::Projects(x, y)) => return self.project_menu(x, y),
            Some(Click::Tabs(x, y)) => return self.tabs_menu(x, y),
            // Once a frame while the edge is dragged: laid out, nothing redrawn but at the end.
            Some(Click::SideWidth(width, done)) => {
                if let Some(open) = self.deck.as_mut() {
                    open.size_side(f64::from(width), done, self.shared);
                }
                return;
            }
            Some(Click::SplitMove(moved, done)) => {
                if let Some(open) = self.deck.as_mut() {
                    open.drag_split(moved, done);
                }
                if done {
                    self.split_sized();
                }
                return;
            }
            // The list shows the move already: the layout keeps it, and tells the window back.
            Some(Click::Move(key, before)) => {
                let before = before.map(|before| before.name());
                let moved = Heard::Moved {
                    before: before.as_deref(),
                    window: None,
                };
                self.shared.heard(&key.name(), moved);
                return;
            }
            // Carried over the pages, the window lays the tab out where it lands as it moves;
            // off them, back as it was.
            Some(Click::DropOver(key, x, y)) => {
                if let Some(open) = self.deck.as_mut() {
                    open.drop_over(key, Some((f64::from(x), f64::from(y))));
                }
                return;
            }
            Some(Click::DropOff) => {
                if let Some(open) = self.deck.as_mut() {
                    open.drop_off();
                }
                return;
            }
            Some(Click::Drop(key, x, y)) => return self.dropped(key, f64::from(x), f64::from(y)),
            Some(Click::SplitEven) => {
                if let Some(open) = self.deck.as_mut() {
                    open.split.share = 0.5;
                    open.split.grabbed = None;
                    open.layout();
                }
                self.split_sized();
                return;
            }
            Some(Click::Frame(x, y)) => return self.size_menu(x, y),
            Some(Click::More(x, y, held)) => return self.more_menu(x, y, held),
            // GTK lays the strip and the pages in a column, so the strip's own list could only
            // take the pages' place: the history is the system's menu there.
            #[cfg(not(target_os = "macos"))]
            Some(Click::History) => return self.history_menu(),
            Some(Click::Menu(key, x, y)) => return self.tab_menu(key, (x, y)),
            Some(Click::Key(keyed)) => return self.keyed(keyed),
            Some(Click::Agent(id)) => return self.start_agent(id),
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
                let shows = matches!(said, Click::Show(_));
                if act(self.deck, said, self.shared).is_ok()
                    && shows
                    && let Some(open) = self.deck.as_ref()
                {
                    open.focus_shown();
                }
            }
            None => {}
        }
    }

    /// The person closed this window (not the main one): each of its tabs closes as the person
    /// closing it would, and whoever works in it is told; then the window goes.
    pub(super) fn close_window(&mut self) {
        let keys = self.deck.as_ref().map(Deck::keys).unwrap_or_default();
        // Not drawn: its window is about to go.
        self.close_all(keys, false);
        *self.deck = None;
    }

    /// The person closes tab `key`, and whoever works in it is told.
    pub(super) fn close(&mut self, key: Key) {
        self.close_all([key], true);
    }

    /// The person closes tabs `keys` one after another, each told to whoever works in it; the
    /// window is drawn once after them all (`draw`), not once per tab.
    pub(super) fn close_all(&mut self, keys: impl IntoIterator<Item = Key>, draw: bool) {
        for key in keys {
            let closed = match self.deck.as_mut() {
                Some(open) => open.act(Click::Close(key), self.shared),
                None => Err("the window is not open".to_string()),
            };
            if closed.is_ok() {
                let what = match key.kind() {
                    "web" => CLOSED_PAGE,
                    "board" => CLOSED_BOARD,
                    _ => CLOSED_TERMINAL,
                };
                self.shared.heard(&key.name(), Heard::Closed(what));
            }
        }
        if draw && let Some(open) = self.deck.as_ref() {
            open.refresh(self.shared);
        }
    }

    /// What the person typed in the address field: the page shown goes there (a terminal shown,
    /// a new page of theirs does), and whoever works in it is told.
    fn go(&mut self, url: String) {
        let Some(open) = self.deck.as_ref() else {
            return;
        };
        let web = open.web.as_ref().filter(|_| open.shown == Shown::Web);
        let Some(web) = web else {
            // Of the project shown, whatever tab (the settings, of none) shows in it.
            let group = crate::lock(&self.shared.scope).shown().to_string();
            self.shared
                .heard(&group, Heard::Asked(Asked::Page(Some(url))));
            return;
        };
        let _ = web.active().load_url(&url);
        let id = web.active_id();
        let said = format!("The person went to {url} in this browser tab.");
        self.shared.heard(&Key::Web(id).name(), Heard::Did(&said));
    }

    /// "»": the system's menu under it, the buttons the strip had no room for; the one picked
    /// does as its button would, its own menu (if any) under "»".
    fn more_menu(&mut self, x: u32, y: u32, held: u8) {
        let Some(open) = self.deck.as_ref() else {
            return;
        };
        let tools: Vec<Tool> = Tool::ALL
            .into_iter()
            .filter(|tool| held & tool.bit() != 0)
            .collect();
        let labels: Vec<&str> = tools.iter().map(|tool| tool.label()).collect();
        let at = (f64::from(x), f64::from(y));
        match open.choose(at, &labels, None).and_then(|at| tools.get(at)) {
            Some(Tool::Events) => self.clicked("events"),
            Some(Tool::History) => self.clicked("history"),
            None => {}
        }
    }

    /// The size button: the system's menu under it, the sizes to pick from; the page shown takes
    /// the one picked, and whoever works in it is told.
    fn size_menu(&mut self, x: u32, y: u32) {
        let Some(open) = self.deck.as_ref() else {
            return;
        };
        let Some(web) = open.web.as_ref().filter(|_| open.shown == Shown::Web) else {
            return;
        };
        let now = web.frame(None).cloned();
        let (items, marked) = sizes::menu(now.as_ref());
        let titles: Vec<&str> = items.iter().map(String::as_str).collect();
        let at = (f64::from(x), f64::from(y));
        let Some(frame) = open
            .choose(at, &titles, marked)
            .and_then(|at| sizes::picked(at, now.as_ref()))
            .map(sizes::Picked::frame)
        else {
            return;
        };
        let id = web.active_id();
        let said = match &frame {
            Some(frame) => format!(
                "The person set this tab's page size to {} ({}).",
                frame.name,
                frame.sides()
            ),
            None => "The person let this tab's page fill the window again.".to_string(),
        };
        let _ = self.proxy.send_event(Cmd::Frame(Some(id), frame));
        self.shared.heard(&Key::Web(id).name(), Heard::Did(&said));
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
        if let Some(row) = open
            .choose((0.0, 0.0), &items, None)
            .and_then(|at| rows.get(at))
        {
            self.visit(row.at);
        }
    }

    /// Row `at` of the shown project's history shown again: an open tab here, else the app
    /// opens it again (a page at its address, a terminal in its folder, the board).
    pub(super) fn visit(&mut self, at: i64) {
        let Some(open) = self.deck.as_mut() else {
            return;
        };
        let project = open.shown_project(self.shared);
        let past = (crate::lock(&self.shared.ledgers).get(&project))
            .and_then(|ledger| ledger.tabs.iter().find(|past| past.row == at).cloned());
        let again = match past {
            Some(past) => open.revisit(&past, self.shared),
            None => Revisit::Again(at),
        };
        if let Revisit::Again(row) = again {
            self.shared.heard(&project, Heard::Asked(Asked::Again(row)));
        }
        open.refresh(self.shared);
    }

    /// `script` in a panel's page, or in the strip's (`STRIP`, for checks).
    pub(super) fn eval_panel(&self, id: u64, script: &str, panel: u32) {
        let answer = Arc::clone(self.shared);
        let reply = move |value: String| {
            let value = serde_json::from_str(&value).unwrap_or(Value::String(value));
            answer.answer(id, Ok(value));
        };
        match self.deck.as_ref().and_then(|open| open.page(panel)) {
            Some(page) => {
                if let Err(why) = page.evaluate_script_with_callback(script, reply) {
                    self.shared.answer(id, Err(why.to_string()));
                }
            }
            None => self.shared.answer(id, Err(WINDOW_CLOSED.into())),
        }
    }
}

/// Show or close the tab `said` names.
pub(super) fn act(deck: &mut Option<Deck>, said: Click, shared: &Shared) -> Result<(), String> {
    let open = deck.as_mut().ok_or("the window is not open")?;
    open.act(said, shared)?;
    open.refresh(shared);
    Ok(())
}

/// What the person's closing says, to whoever listens.
const CLOSED_PAGE: &str = "The person closed this browser tab.";
const CLOSED_TERMINAL: &str = "The person closed this tab: its program runs on, out of sight.";
const CLOSED_BOARD: &str =
    "The person closed the board's tab: the board is kept, and `board` shows it again.";
