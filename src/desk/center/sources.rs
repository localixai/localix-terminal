//! The parts of the desk that have events for the person. Each reads what it keeps itself and
//! says its events; none knows the window, the bell or the other sources. A new one is a type
//! here and its place in `SOURCES`.

use super::{Do, Event, Kind, Leads, Subject};
use crate::board::Column;
use crate::desk::Desk;

/// A part of the desk with events for the person.
pub(super) trait Source: Sync {
    /// Its events now, read from what it keeps. It takes its own locks, one owner's at a time,
    /// and holds none of another source's.
    fn gather(&self, desk: &Desk) -> Vec<Event>;
}

/// Every source, gathered in turn.
pub(super) const SOURCES: &[&dyn Source] = &[&Asks, &Boards, &Terminals];

/// What the agent asked the person: something to do on a tab, or that its session waits.
struct Asks;

impl Source for Asks {
    fn gather(&self, desk: &Desk) -> Vec<Event> {
        let registry = crate::lock(&desk.registry);
        let asks = crate::lock(&desk.asks);
        let mut asked: Vec<_> = asks.iter().collect();
        asked.sort_by_key(|(subject, _)| *subject);
        let group = |owner: &str| registry.group_of(owner);
        asked
            .into_iter()
            .filter_map(|(subject, ask)| {
                let event = match subject {
                    // Its tab closed and the notice is still on its way: nothing to do there.
                    Subject::Tab(tab) if registry.owner_named(tab).is_none() => return None,
                    Subject::Tab(tab) => Event::new(
                        format!("ask:{tab}:{}", ask.n),
                        Kind::Ask,
                        &group(&ask.owner),
                        tab,
                    )
                    .button("Done", Do::Answer { tab: tab.clone() })
                    .button("Show", Do::Open(Leads::Tab(tab.clone()))),
                    Subject::Terminal(tab) if registry.owner_named(tab).is_none() => return None,
                    Subject::Terminal(tab) => Event::new(
                        format!("waiting:{tab}:{}", ask.n),
                        Kind::Waiting,
                        &group(&ask.owner),
                        tab,
                    )
                    .button("Show", Do::Open(Leads::Tab(tab.clone()))),
                    Subject::Session(owner) => {
                        let group = group(owner);
                        let id = format!("waiting:{owner}:{}", ask.n);
                        let event = Event::new(id, Kind::Waiting, &group, "");
                        // It runs outside the window: its agent's program continues it here.
                        match &ask.from {
                            Some((program, folder)) => event.button(
                                "Continue Here",
                                Do::Open(Leads::Session {
                                    owner: owner.clone(),
                                    group,
                                    program: program.clone(),
                                    folder: folder.clone(),
                                }),
                            ),
                            None => event,
                        }
                    }
                };
                Some(event.says(&ask.words, String::new()).at(ask.at))
            })
            .collect()
    }
}

/// The boards' cards that wait for the person: in Review, or blocked.
struct Boards;

impl Source for Boards {
    fn gather(&self, desk: &Desk) -> Vec<Event> {
        let boards = crate::lock(&desk.boards);
        let mut boards: Vec<_> = boards.iter().collect();
        boards.sort_by_key(|(project, _)| *project);
        let mut events = vec![];
        for (project, kept) in boards {
            let (project, tab) = (*project, kept.key.as_str());
            let group = crate::projects::key(project);
            let group = group.as_str();
            let board = || Do::Open(Leads::Board(group.to_string()));
            for card in &kept.board.cards {
                let id = card.id.clone();
                if card.column == Column::Review {
                    let since = card.since().unwrap_or(0);
                    let event = Event::new(
                        format!("review:{project}:{id}:{since}"),
                        Kind::Review,
                        group,
                        tab,
                    )
                    .says(&card.title, handed_in(card))
                    .at(since)
                    .button(
                        "Accept",
                        Do::Accept {
                            project,
                            card: id.clone(),
                        },
                    )
                    .button("Open Board", board());
                    events.push(event);
                }
                if let Some(reason) = &card.blocked {
                    let event = Event::new(
                        format!("blocked:{project}:{id}:{reason}"),
                        Kind::Blocked,
                        group,
                        tab,
                    )
                    .says(&card.title, format!("Blocked: {reason}"))
                    .button("Unblock", Do::Unblock { project, card: id })
                    .button("Open Board", board());
                    events.push(event);
                }
            }
        }
        events
    }
}

/// Who brought a card to Review, in a line: a worker's hand-in says what they did; the person's
/// own move says so.
fn handed_in(card: &crate::board::Card) -> String {
    let moved = (card.history.iter().rev())
        .find(|step| step.to == Some(Column::Review))
        .map(|step| step.by.as_str());
    match (moved, card.result.as_deref()) {
        (Some("person"), _) => "You moved it to Review".to_string(),
        (by, Some(result)) => format!("{}: {result}", by.unwrap_or("agent")),
        (by, None) => format!("{} handed it in", by.unwrap_or("agent")),
    }
}

/// The terminals whose program ended or has a note for the person, while their tab is open.
struct Terminals;

/// What a terminal has for the person: its command, how its program ended if it did, and the
/// note it waits with.
struct Had {
    id: u32,
    command: String,
    ended: bool,
    status: Option<i32>,
    note: Option<(u32, String)>,
}

impl Source for Terminals {
    fn gather(&self, desk: &Desk) -> Vec<Event> {
        // The sessions' lock is let go before the registry's is taken.
        let had: Vec<Had> = (crate::lock(&desk.sessions).iter())
            .map(|(id, session)| Had {
                id: *id,
                command: session.command().trim().to_string(),
                ended: session.ended(),
                status: session.status(),
                note: session.note(),
            })
            .filter(|had| had.ended || had.note.is_some())
            .collect();
        let registry = crate::lock(&desk.registry);
        let mut events = vec![];
        for had in had {
            let key = format!("t{}", had.id);
            let Some(owner) = registry.owner_named(&key) else {
                continue;
            };
            let group = registry.group_of(&owner);
            let title = if had.command.is_empty() {
                "shell"
            } else {
                &had.command
            };
            let show = || Do::Open(Leads::Tab(key.clone()));
            // The program's own words, plain: shown to the person as its title is.
            if let Some((number, words)) = had.note {
                let event = Event::new(format!("said:{key}:{number}"), Kind::Said, &group, &key)
                    .says(title, words)
                    .names_its_tab()
                    .button("Show", show());
                events.push(event);
            }
            if had.ended {
                let (kind, detail) = match had.status {
                    Some(0) => (Kind::Finished, "Finished".to_string()),
                    Some(code) => (Kind::Failed, format!("Exited with {code}")),
                    None => (Kind::Failed, "Ended".to_string()),
                };
                let event = Event::new(format!("ended:{key}"), kind, &group, &key)
                    .says(title, detail)
                    .names_its_tab()
                    .button("Show", show())
                    .button("Close", Do::Close { key: key.clone() });
                events.push(event);
            }
        }
        events
    }
}
