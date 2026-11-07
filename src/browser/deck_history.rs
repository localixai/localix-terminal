//! The history menu's side of the window: the tabs of the project shown, from its ledger (the
//! app keeps it, `Browser::ledger`), each with what became of it: open, a terminal hidden or
//! ended, or closed (opened again by the app, from its row).

use super::super::host::Shared;
use super::super::panel::Held;
use super::super::strip::{Key, Row, State};
use super::super::{Past, TabKind, Visited};
use super::{Deck, Shown};

/// What showing an entry of the history again comes to.
#[derive(Debug, PartialEq, Eq)]
pub(in super::super) enum Revisit {
    Shown,
    /// Not open in the window: for the app to open again, from its row of the ledger.
    Again(i64),
}

impl Deck {
    /// The tabs of `listed` the person sees listed under project `group`: its own, and the
    /// window's own (of none, as Settings): the ones the keys and the tabs' menu go among.
    pub(in super::super) fn project_tabs<'a>(
        listed: &'a [crate::WindowTab],
        group: &'a str,
    ) -> impl Iterator<Item = &'a crate::WindowTab> {
        listed
            .iter()
            .filter(move |tab| tab.group == group || tab.group.is_empty())
    }

    /// The project shown (scope.rs): else the one of the tab shown, else the first the window
    /// knows.
    pub(in super::super) fn shown_project(&self, shared: &Shared) -> String {
        let scoped = crate::lock(&shared.scope).shown().to_string();
        if !scoped.is_empty() {
            return scoped;
        }
        let listed = self.listing(shared);
        let shown = listed
            .into_iter()
            .find(|tab| tab.shown)
            .map(|tab| tab.group);
        shown
            .filter(|group| !group.is_empty())
            .or_else(|| {
                crate::lock(&shared.places)
                    .first()
                    .map(|place| place.key.clone())
            })
            .unwrap_or_default()
    }

    /// The history's entries of the project shown, newest first.
    pub(in super::super) fn rows(&self, shared: &Shared) -> Vec<Row> {
        let project = self.shown_project(shared);
        let ledgers = crate::lock(&shared.ledgers);
        let Some(ledger) = ledgers.get(&project) else {
            return vec![];
        };
        (ledger.tabs.iter())
            .map(|past| {
                let key = past.key.as_deref().and_then(Key::parse);
                let title = past.title.strip_prefix("desk · ").unwrap_or(&past.title);
                Row {
                    at: past.row,
                    key,
                    kind: kind(past.kind),
                    title: title.to_string(),
                    detail: past.detail.clone(),
                    state: key.and_then(|key| self.state(key)).unwrap_or(State::Closed),
                    group: project.clone(),
                }
            })
            .collect()
    }

    /// The pages the project shown showed, newest first: what the address field suggests.
    pub(in super::super) fn visited(&self, shared: &Shared) -> Vec<Visited> {
        let project = self.shown_project(shared);
        (crate::lock(&shared.ledgers).get(&project))
            .map(|ledger| ledger.visited.clone())
            .unwrap_or_default()
    }

    /// What became of open tab `key`, while the window has it.
    fn state(&self, key: Key) -> Option<State> {
        match key {
            Key::Web(id) => {
                let open = self.web.as_ref().filter(|web| web.has(id));
                open.map(|_| State::Open)
            }
            Key::Term(id) => self.term(id).map(|open| match open.held {
                Held::Live => State::Open,
                Held::Hidden => State::Hidden,
                Held::Ended => State::Ended,
            }),
        }
    }

    /// Show `past` again, and close the menu: an open tab, a terminal the person closed (a tab
    /// again) or one that ended (its last screen, which is no tab); else it is the app's to
    /// open again.
    pub(in super::super) fn revisit(&mut self, past: &Past, shared: &Shared) -> Revisit {
        self.menu = None;
        let key = past.key.as_deref().and_then(Key::parse);
        match (key, key.and_then(|key| self.state(key))) {
            (Some(Key::Web(id)), Some(_)) => {
                let switched = self.web.as_mut().map(|web| web.switch(id, shared));
                self.show(Shown::Web);
                match switched {
                    Some(Ok(())) => Revisit::Shown,
                    Some(Err(_)) | None => Revisit::Again(past.row),
                }
            }
            (Some(Key::Term(id)), Some(_)) => {
                self.unhide_term(id);
                self.shown = Shown::Term(id);
                Revisit::Shown
            }
            (_, None) | (None, _) => Revisit::Again(past.row),
        }
    }
}

/// How the strip names a kind of tab.
fn kind(kind: TabKind) -> &'static str {
    match kind {
        TabKind::Page => "web",
        TabKind::Terminal => "term",
        TabKind::Board => "board",
    }
}
