//! The new-tab menu and agent picker, expressed as requests to the desk.

use super::On;
use crate::browser::agents::AGENTS;
use crate::browser::choice::Entry;
use crate::browser::{Asked, Heard, icons};

impl On<'_> {
    /// "+": the system's menu under it, then a page, a terminal or the board of the project
    /// shown, opened by the app as it opens the agent's (so a page opens even when the window
    /// holds no page yet).
    pub(super) fn plus(&mut self, x: u32, y: u32) {
        let Some(open) = self.deck.as_ref() else {
            return;
        };
        let at = (f64::from(x), f64::from(y));
        // The coding agents under "Agents", each with its mark: the ones on the person's PATH to pick,
        // the rest shown greyed.
        let path = crate::browser::agents::path();
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

    /// An agent's mark where no tab is left: a terminal of the project shown that runs it, when
    /// it is installed.
    pub(super) fn start_agent(&self, id: &str) {
        let path = crate::browser::agents::path();
        let agent = AGENTS.iter().find(|agent| agent.id == id);
        if let Some(command) = agent.and_then(|agent| agent.command_on(&path)) {
            self.ask(Asked::Agent(command));
        }
    }

    /// The person asks for a new tab of the project shown.
    fn ask(&self, asked: Asked) {
        let Some(open) = self.deck.as_ref() else {
            return;
        };
        let group = open.shown_project(self.shared);
        self.shared.heard(&group, Heard::Asked(asked));
    }
}
