//! What a program says of its own state (OSC 7501, the program status protocol): at work, waiting
//! for the person, done, stopped on an error. A coding agent reports it only to a terminal that
//! answered its query, so the session answers it (`REPLY`) as it answers a cursor query. The
//! words are a program's and untrusted: base64 of plain text, kept as plain words for the person,
//! never acted on. The records are the terminal's, not the screen's: the alternate screen leaves
//! them, a reset (RIS) takes them all. What is at work goes when the program does; what is done or
//! failed stays until the person has seen it. What wants the person is a line of its record
//! (`lines.rs`), kept in place until they type in the terminal, however the program flips.

#[path = "status/lines.rs"]
mod lines;
mod parse;

use std::collections::HashMap;

pub(super) use parse::{BODY_MOST, Report, Said, read};

/// The terminal's answer to `OSC 7501 ; ?`: the states and kinds of asking it reads.
pub(super) const REPLY: &[u8] =
    b"\x1b]7501;?:states=idle,working,done,blocked,error:kinds=permission,question,auth\x1b\\";

/// The most records a terminal keeps; past it, the one told of longest ago goes.
pub(super) const MOST: usize = 256;

/// What a program is doing, by its report. Ordered by how much it wants the person.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Doing {
    /// Waiting for input, at rest.
    Idle,
    Working,
    /// Finished, and the person has not seen it yet.
    Done,
    /// Stopped on an error.
    Error,
    /// It cannot go on without the person.
    Blocked,
}

/// What a blocked program waits for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Asks {
    /// A yes to do something (a command, a file written).
    Permission,
    Question,
    /// A sign-in.
    Auth,
}

/// One record of a terminal's program: its root (`id` empty) or one of its parts (a background
/// agent, `a1`; a path of them, `a1/b`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Status {
    pub id: String,
    pub state: Doing,
    /// What it waits for, while blocked.
    pub asks: Option<Asks>,
    /// The program's stable name (`claude-code`), its own or its nearest parent's.
    pub app: Option<String>,
    pub title: Option<String>,
    pub msg: Option<String>,
    /// How far, 0 to 100, while at work or blocked; none: not known.
    pub progress: Option<u8>,
    /// The number of the change that made it: the same record told again keeps it.
    pub changed: u64,
    /// The number of its line (`lines.rs`): the same while it waits for the person, another
    /// once they typed in the terminal since.
    pub round: u64,
}

/// A record and when it was last told, as the terminal keeps it, with its line.
#[derive(Debug)]
struct Kept {
    status: Status,
    told: u64,
    /// The record as its line shows it: its last report that wants the person (waiting,
    /// failed, done) in this round; none: no line.
    shown: Option<Status>,
    /// The person typed in the terminal since its line began.
    attended: bool,
}

/// The records of one terminal's program.
#[derive(Debug, Default)]
pub(super) struct Records {
    kept: Vec<Kept>,
    /// Counts the changes: a reader knows by it when to pass the records on.
    changes: u64,
    /// Counts the reports, so the one told of longest ago is known.
    told: u64,
    /// Counts the lines' rounds and the changes of the lines.
    rounds: u64,
    lined: u64,
    /// The changes of the lines a reader was last told of, and whether it has not read them
    /// since (`lines.rs`, `news`).
    told_lined: u64,
    unread: bool,
}

impl Records {
    /// Takes in a report; whether the records changed. Each report replaces its record whole;
    /// a clear takes the record and its children, the root's clear all of them.
    pub(super) fn apply(&mut self, report: Report) -> bool {
        self.told += 1;
        let Some(state) = report.state else {
            let id = report.id;
            let under = format!("{id}/");
            let before = self.kept.len();
            (self.kept).retain(|kept| {
                !id.is_empty() && kept.status.id != id && !kept.status.id.starts_with(&under)
            });
            return self.changed_if(self.kept.len() != before);
        };
        let status = Status {
            id: report.id,
            state,
            asks: report.asks,
            app: report.app,
            title: report.title,
            msg: report.msg,
            progress: report.progress,
            changed: 0,
            round: 0,
        };
        let told = self.told;
        let at = match (self.kept.iter()).position(|kept| kept.status.id == status.id) {
            Some(at) => {
                let kept = &mut self.kept[at];
                kept.told = told;
                let again = Status {
                    changed: kept.status.changed,
                    ..status.clone()
                };
                if again == kept.status {
                    return false;
                }
                kept.status = Status {
                    changed: self.changes + 1,
                    ..status
                };
                at
            }
            None => {
                if self.kept.len() >= MOST
                    && let Some(oldest) = (self.kept.iter().enumerate())
                        .min_by_key(|(_, kept)| kept.told)
                        .map(|(at, _)| at)
                {
                    let gone = self.kept.remove(oldest);
                    self.lined += u64::from(gone.shown.is_some());
                }
                let status = Status {
                    changed: self.changes + 1,
                    ..status
                };
                let (shown, attended) = (None, false);
                (self.kept).push(Kept {
                    status,
                    told,
                    shown,
                    attended,
                });
                self.kept.len() - 1
            }
        };
        self.relined(at);
        self.changed_if(true)
    }

    /// The program is gone (it ended, or the shell came back in front): what was at work, waited
    /// or rested goes; what is done or failed stays for the person. Whether any went.
    pub(super) fn drop_transient(&mut self) -> bool {
        self.keep_only(|state| matches!(state, Doing::Done | Doing::Error))
    }

    /// The person has seen the terminal: what is done or failed goes, and a line whose program
    /// went on with it. Whether any went.
    pub(super) fn seen(&mut self) -> bool {
        let went = self.keep_only(|state| !matches!(state, Doing::Done | Doing::Error));
        self.attended();
        went
    }

    /// A reset of the terminal (RIS): every record goes. Whether there were some.
    pub(super) fn reset(&mut self) -> bool {
        self.keep_only(|_| false)
    }

    /// How many times the records changed.
    pub(super) fn changes(&self) -> u64 {
        self.changes
    }

    /// The records, the root first, then by id, each with the app it is of.
    pub(super) fn rows(&self) -> Vec<Status> {
        let apps = self.apps();
        let mut rows: Vec<Status> = (self.kept.iter())
            .map(|kept| Self::row(&kept.status, kept, &apps))
            .collect();
        rows.sort_by(|a, b| a.id.cmp(&b.id));
        rows
    }

    /// The record that wants the person most: blocked, then failed, done, at work, at rest; of
    /// two alike, the one changed last.
    pub(super) fn most_urgent(&self) -> Option<Status> {
        let kept =
            (self.kept.iter()).max_by_key(|kept| (kept.status.state, kept.status.changed))?;
        Some(Self::row(&kept.status, kept, &self.apps()))
    }

    /// `status` of record `kept` as it is read: with its round, and the app it is of (its own,
    /// else its nearest parent's that names one, else the root's).
    fn row(status: &Status, kept: &Kept, apps: &HashMap<&str, &str>) -> Status {
        let mut at = status.id.as_str();
        let app = loop {
            if let Some(app) = apps.get(at) {
                break Some((*app).to_string());
            }
            if at.is_empty() {
                break None;
            }
            at = at.rsplit_once('/').map_or("", |(parent, _)| parent);
        };
        Status {
            app,
            round: kept.round(),
            ..status.clone()
        }
    }

    /// The app each record names of itself, by its id: read once for all the records, so
    /// finding each one's is a walk up its parents, not a search of every record.
    fn apps(&self) -> HashMap<&str, &str> {
        (self.kept.iter())
            .filter_map(|kept| Some((kept.status.id.as_str(), kept.status.app.as_deref()?)))
            .collect()
    }

    fn keep_only(&mut self, kept: impl Fn(Doing) -> bool) -> bool {
        let before = self.kept.len();
        let mut unlined = 0;
        self.kept.retain(|record| {
            let keeps = kept(record.status.state);
            unlined += u64::from(!keeps && record.shown.is_some());
            keeps
        });
        self.lined += unlined;
        self.changed_if(self.kept.len() != before)
    }

    fn changed_if(&mut self, changed: bool) -> bool {
        if changed {
            self.changes += 1;
        }
        changed
    }
}

#[cfg(test)]
#[path = "status_tests.rs"]
mod tests;
