//! Keyboard shortcut actions adapted to the same tab and project operations as clicks.

use super::{On, act};
use crate::browser::deck::Deck;
use crate::browser::strip::{Click, Key, Keyed};
use crate::browser::{Asked, Heard};

impl On<'_> {
    /// A shortcut's doing (shortcuts.rs) that no button of the strip does.
    pub(super) fn keyed(&mut self, keyed: Keyed) {
        let Some(open) = self.deck.as_mut() else {
            return;
        };
        let listed = open.listing(self.shared);
        let shown = listed.iter().position(|tab| tab.shown);
        let group = open.shown_project(self.shared);
        // The tabs the person sees listed: the project's shown, and the window's own (of none).
        let tabs: Vec<Key> = Deck::project_tabs(&listed, &group)
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
}
