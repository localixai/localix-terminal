//! The new-tab menu and agent picker, expressed as requests to the desk.

use super::On;
use crate::browser::agents::AGENTS;
use crate::browser::choice::Entry;
use crate::browser::{Asked, Heard, Side, icons};

impl On<'_> {
    /// "+": the system's menu under it, then a page, a terminal or the board of the project
    /// shown, opened by the app as it opens the agent's (so a page opens even when the window
    /// holds no page yet); or a terminal or an agent split beside the pane in use.
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
            icon: Some(icons::of_kind(kind)),
            key: crate::browser::shortcuts::keys_of(id),
        };
        let item = |label: String, off: bool, icon: Option<&'static str>| Entry::Item {
            label,
            marked: false,
            off,
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
        // After the board, each kind that draws itself.
        let kinds = crate::kind::abouts();
        let mut entries = vec![
            keyed("New Page", "web", "new-page"),
            keyed("New Terminal", "term", "new-terminal"),
            keyed("Board", "board", "board"),
        ];
        entries.extend(
            kinds
                .iter()
                .map(|about| keyed(about.title, about.id, about.command())),
        );
        entries.push(Entry::Line { line: true });
        entries.push(Entry::Sub {
            label: "Agents".into(),
            items: agents,
        });
        // A terminal or an agent beside the pane in use, each way it splits: greyed where the
        // new pane would not keep its least.
        let sides = [
            (Side::Right, "Split Right with\u{2026}"),
            (Side::Down, "Split Down with\u{2026}"),
        ];
        for (side, label) in sides {
            let fits = open.fits(side);
            let beside = (AGENTS.iter().zip(&found)).map(|(agent, command)| {
                let off = !fits || command.is_none();
                item(agent.name.to_string(), off, Some(agent.mark))
            });
            let terminal = item("Terminal".into(), !fits, Some(icons::of_kind("term")));
            entries.push(Entry::Sub {
                label: label.into(),
                items: std::iter::once(terminal).chain(beside).collect(),
            });
        }
        let own = 3 + kinds.len();
        let (agents, split) = (own + AGENTS.len(), 1 + AGENTS.len());
        let asked = match open.pick(at, &entries) {
            Some(0) => Asked::Page(None),
            Some(1) => Asked::Terminal(None),
            Some(2) => Asked::Board,
            Some(at) if at < own => match kinds.get(at - 3) {
                Some(about) => Asked::Kind(about.id),
                None => return,
            },
            Some(at) if at < agents => match found.get(at - own).copied().flatten() {
                Some(command) => Asked::Agent(command),
                None => return,
            },
            Some(at) => {
                let side = if at - agents < split {
                    Side::Right
                } else {
                    Side::Down
                };
                let asked = match (at - agents) % split {
                    0 => Asked::Terminal(None),
                    agent => match found.get(agent - 1).copied().flatten() {
                        Some(command) => Asked::Agent(command),
                        None => return,
                    },
                };
                return self.split_new(side, asked);
            }
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
