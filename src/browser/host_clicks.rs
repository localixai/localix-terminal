//! What the loop does with the person's clicks and the window's presence: a tab shown or closed,
//! the history menu's entry, a page's answer, the mini named, its picture, an event of its state.

use std::sync::Arc;

use serde_json::Value;

use super::super::presence::Stage;
use super::super::{Asked, Cmd, Heard, MINI, Shared, WINDOW_CLOSED};
use super::On;
use crate::browser::agents::AGENTS;
use crate::browser::capture::Shot;
use crate::browser::choice::Entry;
use crate::browser::deck::{Deck, Revisit, Shown};
use crate::browser::icons;
use crate::browser::presence::Event as Presence;
use crate::browser::sizes;
use crate::browser::strip::{Click, Key, Keyed, Tool, click, went};

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

    /// The person clicked the mini open.
    pub(super) fn opened(&mut self) {
        let mut stage = Stage {
            deck: &mut *self.deck,
            target: self.target,
            shared: self.shared,
            proxy: self.proxy,
        };
        self.presenter.opened(&mut stage);
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
        match click(said) {
            Some(Click::Visit(at)) => return self.visit(at),
            Some(Click::Offer(take)) => return self.offer(take),
            Some(Click::Collapse) => return self.presence(Presence::Collapse),
            Some(Click::Quit) => return self.shared.heard("", Heard::Quit),
            Some(Click::NewTab(x, y)) => return self.plus(x, y),
            Some(Click::Agent(at)) => return self.agent(at),
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
        for key in keys {
            self.close(key);
        }
        *self.deck = None;
    }

    /// The person closes tab `key`, and whoever works in it is told.
    pub(super) fn close(&mut self, key: Key) {
        if act(self.deck, Click::Close(key), self.shared).is_ok() {
            let what = match key.kind() {
                "web" => CLOSED_PAGE,
                "board" => CLOSED_BOARD,
                _ => CLOSED_TERMINAL,
            };
            self.shared.heard(&key.name(), Heard::Closed(what));
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
        let id = web.listed().1;
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
            Some(Tool::History) => self.clicked("history"),
            Some(Tool::Mini) => self.clicked("collapse"),
            Some(Tool::Events) => self.clicked("events"),
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
        let id = web.listed().1;
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

    /// "+": the system's menu under it, then a page, a terminal or the board of the project
    /// shown, opened by the app as it opens the agent's (so a page opens even when the window
    /// holds no page yet).
    fn plus(&mut self, x: u32, y: u32) {
        let Some(open) = self.deck.as_ref() else {
            return;
        };
        let at = (f64::from(x), f64::from(y));
        // The coding agents under "Agents", each with its mark: the ones on the PATH to pick,
        // the rest shown greyed.
        let path = std::env::var_os("PATH").unwrap_or_default();
        let found: Vec<Option<&'static str>> =
            AGENTS.iter().map(|agent| agent.command_on(&path)).collect();
        // Each with its picture and the person's own keys for it, as the menu bar shows them.
        let keyed = |label: &str, kind: &str, id: &str| Entry::Item {
            label: label.into(),
            marked: false,
            off: false,
            paste: false,
            icon: Some(icons::of_kind(kind)),
            key: crate::browser::shortcuts::keys_of(id),
        };
        let item = |label: String, off: bool, icon: Option<&'static str>| Entry::Item {
            label,
            marked: false,
            off,
            paste: false,
            icon,
            key: None,
        };
        let agents = (AGENTS.iter().zip(&found))
            .map(|(agent, command)| match command {
                Some(_) => item(agent.name.to_string(), false, Some(agent.mark)),
                None => item(
                    format!("{} \u{2014} not installed", agent.name),
                    true,
                    Some(agent.mark),
                ),
            })
            .collect();
        let entries = [
            keyed("New Page", "web", "new-page"),
            keyed("New Terminal", "term", "new-terminal"),
            keyed("Board", "board", "board"),
            Entry::Line { line: true },
            Entry::Sub {
                label: "Agents".into(),
                items: agents,
            },
        ];
        let asked = match open.pick(at, &entries) {
            Some(0) => Asked::Page(None),
            Some(1) => Asked::Terminal(None),
            Some(2) => Asked::Board,
            Some(at) => match found.get(at - 3).copied().flatten() {
                Some(command) => Asked::Agent(command),
                None => return,
            },
            None => return,
        };
        self.ask(asked);
    }

    /// The person asks for a new tab of the project shown.
    fn ask(&self, asked: Asked) {
        let Some(open) = self.deck.as_ref() else {
            return;
        };
        let group = open.shown_project(self.shared);
        self.shared.heard(&group, Heard::Asked(asked));
    }

    /// Agent `at` (its place in the agents' list) started in a terminal, where it is installed.
    fn agent(&self, at: u8) {
        let path = std::env::var_os("PATH").unwrap_or_default();
        let command = AGENTS
            .get(usize::from(at))
            .and_then(|agent| agent.command_on(&path));
        if let Some(command) = command {
            self.ask(Asked::Agent(command));
        }
    }

    /// A shortcut's doing (shortcuts.rs) that no button of the strip does.
    fn keyed(&mut self, keyed: Keyed) {
        let Some(open) = self.deck.as_mut() else {
            return;
        };
        let listed = open.listing(self.shared);
        let shown = listed.iter().position(|tab| tab.shown);
        let group = open.shown_project(self.shared);
        // The tabs the person sees listed: the project's shown, and the window's own (of none).
        let tabs: Vec<Key> = (listed.iter())
            .filter(|tab| tab.group == group || tab.group.is_empty())
            .filter_map(|tab| Key::parse(&tab.id))
            .collect();
        let here = shown
            .and_then(|at| listed.get(at))
            .and_then(|tab| tabs.iter().position(|key| key.name() == tab.id));
        let step = |forward: bool| {
            let count = tabs.len();
            let at = match (here, forward) {
                (Some(at), true) => (at + 1) % count.max(1),
                (Some(at), false) => (at + count.max(1) - 1) % count.max(1),
                (None, _) => 0,
            };
            tabs.get(at).copied()
        };
        let asked = |asked: Asked| self.shared.heard(&group, Heard::Asked(asked));
        let show = match keyed {
            Keyed::Tab(at) => tabs.get(usize::from(at) - 1).copied(),
            Keyed::LastTab => tabs.last().copied(),
            Keyed::Next => step(true),
            Keyed::Previous => step(false),
            Keyed::NewWindow => return asked(Asked::Window),
            Keyed::NewPage => return asked(Asked::Page(None)),
            Keyed::NewTerminal => return asked(Asked::Terminal(None)),
            Keyed::NewBoard => return asked(Asked::Board),
            Keyed::Close => {
                if let Some(key) = shown.and_then(|at| Key::parse(&listed.get(at)?.id)) {
                    self.close(key);
                }
                return;
            }
            Keyed::Reopen => {
                use crate::browser::strip::State;
                let rows = open.rows(self.shared);
                let closed =
                    (rows.iter()).find(|row| matches!(row.state, State::Closed | State::Hidden));
                if let Some(at) = closed.map(|row| row.at) {
                    self.visit(at);
                }
                return;
            }
            Keyed::Find => return open.find_tab(self.shared),
            Keyed::Address => return open.keyed_in_strip("address"),
            Keyed::Size => return open.keyed_in_strip("size"),
            Keyed::Split => return self.split_view(),
            Keyed::Settings => return self.settings(),
            Keyed::Projects => return open.keyed_in_strip("projects"),
            Keyed::OpenFolder => return self.open_folder(),
            Keyed::CloseProject => {
                let shown = crate::lock(&self.shared.scope).shown().to_string();
                return self.close_project(&shown);
            }
        };
        if let Some(key) = show
            && act(self.deck, Click::Show(key), self.shared).is_ok()
            && let Some(open) = self.deck.as_ref()
        {
            open.focus_shown();
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
        if let Some(row) = open
            .choose((0.0, 0.0), &items, None)
            .and_then(|at| rows.get(at))
        {
            self.visit(row.at);
        }
    }

    /// Row `at` of the shown project's history shown again: an open tab here, else the app
    /// opens it again (a page at its address, a terminal in its folder, the board).
    fn visit(&mut self, at: i64) {
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
