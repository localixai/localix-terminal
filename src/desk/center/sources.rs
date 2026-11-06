//! The parts of the desk that have events for the person. Each reads what it keeps itself and
//! says its events; none knows the window, the bell or the other sources. A new one is a type
//! here and its place in `SOURCES`.

use super::{Do, Event, Kind, Subject};
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
                    .button("Show", Do::Show { key: tab.clone() }),
                    Subject::Session(owner) => Event::new(
                        format!("waiting:{owner}:{}", ask.n),
                        Kind::Waiting,
                        &group(owner),
                        "",
                    ),
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
            let board = || Do::Board {
                group: group.to_string(),
            };
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

/// The terminals whose program ended, while their tab is open.
struct Terminals;

impl Source for Terminals {
    fn gather(&self, desk: &Desk) -> Vec<Event> {
        // The sessions' lock is let go before the registry's is taken.
        let ended: Vec<(u32, String, Option<i32>)> = (crate::lock(&desk.sessions).iter())
            .filter(|(_, session)| session.ended())
            .map(|(id, session)| (*id, session.command().trim().to_string(), session.status()))
            .collect();
        let registry = crate::lock(&desk.registry);
        let mut events = vec![];
        for (id, command, status) in ended {
            let key = format!("t{id}");
            let Some(owner) = registry.owner_named(&key) else {
                continue;
            };
            let (kind, detail) = match status {
                Some(0) => (Kind::Finished, "Finished".to_string()),
                Some(code) => (Kind::Failed, format!("Exited with {code}")),
                None => (Kind::Failed, "Ended".to_string()),
            };
            let title = if command.is_empty() {
                "shell"
            } else {
                &command
            };
            let event = Event::new(
                format!("ended:{key}"),
                kind,
                &registry.group_of(&owner),
                &key,
            )
            .says(title, detail)
            .button("Show", Do::Show { key: key.clone() })
            .button("Close", Do::Close { key });
            events.push(event);
        }
        events
    }
}
