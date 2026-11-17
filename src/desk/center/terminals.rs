//! The terminals' events, while their tab is open: a program that ended, what a program asks to
//! tell the person (OSC 9, OSC 777), what it says of its state (OSC 7501), and a command the
//! person typed that ended while they were not looking, and a bell its program rang meanwhile.
//! Of that, a bell, a program that waits for the person, one that failed and such a command want
//! them; one that is done is listed quietly; one at work or at rest is no event. Its words are the program's, shown as its title is.

use std::collections::HashSet;
use std::time::Duration;

use super::sources::Source;
use super::{Do, Event, Kind, Leads, Subject};
use crate::desk::Desk;
use crate::terminal::{Asks, Doing, Ended, Status};

/// The terminals with something for the person.
pub(super) struct Terminals;

/// What a terminal has for the person: its command, how its program ended if it did, the note it
/// waits with, and what its program says of its state.
struct Had {
    id: u32,
    command: String,
    ended: bool,
    status: Option<i32>,
    note: Option<(u32, String)>,
    said: Vec<Status>,
    /// A command the person typed that ended unseen, by its number.
    unseen: Option<(u32, Ended)>,
    /// A bell its program rang unseen, by its number.
    bell: Option<u32>,
}

impl Source for Terminals {
    fn gather(&self, desk: &Desk) -> Vec<Event> {
        // The sessions' lock is let go before the asks' and the registry's are taken.
        let had: Vec<Had> = (crate::lock(&desk.sessions).iter())
            .map(|(id, session)| Had {
                id: *id,
                command: session.command().trim().to_string(),
                ended: session.ended(),
                status: session.status(),
                note: session.note(),
                said: session.status_lines(),
                unseen: session.unseen_command(),
                bell: session.unseen_bell(),
            })
            .filter(|had| {
                had.ended
                    || had.note.is_some()
                    || !had.said.is_empty()
                    || had.unseen.is_some()
                    || had.bell.is_some()
            })
            .collect();
        // Where the agent in a terminal waits with words of its own (a long turn's last line),
        // its program's "done" is not said a second time.
        let worded: HashSet<String> = (crate::lock(&desk.asks).keys())
            .filter_map(|subject| match subject {
                Subject::Terminal(key) => Some(key.clone()),
                Subject::Tab(_) | Subject::Session(_) => None,
            })
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
            // A bell is the program's way to ask for the person, said in words of its own.
            if let Some(number) = had.bell {
                let event = Event::new(format!("bell:{key}:{number}"), Kind::Said, &group, &key)
                    .says(title, "Rang the bell".to_owned())
                    .names_its_tab()
                    .button("Show", show());
                events.push(event);
            }
            if let Some((number, ended)) = had.unseen {
                let id = format!("command:{key}:{number}");
                let event = Event::new(id, Kind::Command, &group, &key)
                    .says(title, finished(ended))
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
            let tab = Tab {
                key: &key,
                group: &group,
                title,
                worded: worded.contains(&key),
            };
            events.extend(had.said.iter().filter_map(|status| tab.said(status)));
        }
        events
    }
}

/// A terminal's tab, as its program's records are told on it.
struct Tab<'a> {
    key: &'a str,
    group: &'a str,
    /// What names the terminal: its command, or `shell`.
    title: &'a str,
    /// Its agent waits with words of its own.
    worded: bool,
}

impl Tab<'_> {
    /// The event of one record's line, if it is one: waiting for the person and failed, of the
    /// program or a part of it (a background agent); done, of the program alone. One event a
    /// line, replaced in place as the line changes: the program waiting again after the person
    /// typed is a line, and news, of its own (`terminal/status/lines.rs`). Its title is the
    /// terminal's, never the program's words, so a program cannot speak as the app: the name it
    /// gives itself is in the detail.
    fn said(&self, status: &Status) -> Option<Event> {
        let (kind, said) = match status.state {
            Doing::Blocked => (
                Kind::Waiting,
                match status.asks {
                    Some(Asks::Permission) => "Needs approval",
                    Some(Asks::Question) => "Has a question",
                    Some(Asks::Auth) => "Needs you to sign in",
                    None => "Waits for you",
                },
            ),
            Doing::Error => (Kind::Failed, "Stopped on an error"),
            Doing::Done if status.id.is_empty() && !self.worded => (Kind::Finished, "Done"),
            Doing::Done | Doing::Working | Doing::Idle => return None,
        };
        let words = (status.msg.clone().or_else(|| status.title.clone()))
            .unwrap_or_else(|| said.to_string());
        let detail = match status.app.as_deref() {
            Some(app) => format!("{} · {words}", named(app)),
            None => words.clone(),
        };
        let id = format!("status:{}:{}:{}", self.key, status.id, status.round);
        let mut event = Event::new(id, kind, self.group, self.key)
            .says(self.title, detail)
            .button("Show", Do::Open(Leads::Tab(self.key.to_string())));
        // Its tab waits with the program's words alone.
        event.words = words;
        Some(event)
    }
}

/// How a command ended, in words: its code, and how long it ran.
fn finished(ended: Ended) -> String {
    let took = lasted(ended.took);
    match ended.code {
        Some(0) => format!("Finished in {took}"),
        Some(code) => format!("Failed with {code} after {took}"),
        None => format!("Ended after {took}"),
    }
}

/// A time as a person reads it: `45s`, `4m 12s`, `1h 1m`.
fn lasted(took: Duration) -> String {
    let seconds = took.as_secs();
    match seconds {
        ..60 => format!("{seconds}s"),
        60..3600 => format!("{}m {}s", seconds / 60, seconds % 60),
        3600.. => format!("{}h {}m", seconds / 3600, seconds % 3600 / 60),
    }
}

/// An app's stable name as words: `claude-code` reads Claude Code.
fn named(app: &str) -> String {
    let words: Vec<String> = (app.split(['-', '_', '.']))
        .filter(|word| !word.is_empty())
        .map(|word| {
            let mut letters = word.chars();
            (letters.next())
                .map(|first| first.to_uppercase().chain(letters).collect())
                .unwrap_or_default()
        })
        .collect();
    words.join(" ")
}

#[cfg(test)]
#[path = "terminals_tests.rs"]
mod tests;
