//! The line each record of a program's state has for the person: its last report that wants
//! them (waiting for them, failed, done), kept in place while the program flips between that and
//! work, and let go of once the person typed in the terminal and the program went on. A line is
//! one round: the program asking again after the person typed is a round, and a line, of its
//! own. So a program that flips its state as fast as it likes changes no line, and tells the
//! Event Center, its journal and the system's notices nothing new.

use super::{Doing, Kept, Records, Status};

/// Whether a record in `state` wants the person.
fn wants(state: Doing) -> bool {
    matches!(state, Doing::Blocked | Doing::Error | Doing::Done)
}

impl Kept {
    /// The round of its line; none: 0.
    pub(super) fn round(&self) -> u64 {
        self.shown.as_ref().map_or(0, |shown| shown.round)
    }
}

impl Records {
    /// Record `at` was told anew: its line as it is now.
    pub(super) fn relined(&mut self, at: usize) {
        let Some(kept) = self.kept.get_mut(at) else {
            return;
        };
        let status = &kept.status;
        let before = kept.shown.clone();
        if wants(status.state) {
            // A round begins with the first line, and again once the person has typed since.
            let round = match &kept.shown {
                Some(shown) if !kept.attended => shown.round,
                _ => {
                    self.rounds += 1;
                    self.rounds
                }
            };
            kept.shown = Some(Status {
                round,
                ..status.clone()
            });
            kept.attended = false;
        } else if kept.attended {
            // At work or at rest after the person typed: what wanted them is past.
            kept.shown = None;
        }
        let same = |a: &Status, b: &Status| {
            let plain = |status: &Status| Status {
                changed: 0,
                ..status.clone()
            };
            plain(a) == plain(b)
        };
        let changed = match (&before, &kept.shown) {
            (Some(a), Some(b)) => !same(a, b),
            (None, None) => false,
            _ => true,
        };
        self.lined += u64::from(changed);
    }

    /// The person typed in the terminal: each line's round is over, and one whose program no
    /// longer wants them goes.
    pub(super) fn attended(&mut self) {
        for kept in &mut self.kept {
            kept.attended = true;
            if !wants(kept.status.state) && kept.shown.take().is_some() {
                self.lined += 1;
            }
        }
    }

    /// The lines, the root's first, then by id, each with the app its record is of.
    pub(super) fn lines(&self) -> Vec<Status> {
        let apps = self.apps();
        let mut lines: Vec<Status> = (self.kept.iter())
            .filter_map(|kept| Some(Self::row(kept.shown.as_ref()?, kept, &apps)))
            .collect();
        lines.sort_by(|a, b| a.id.cmp(&b.id));
        lines
    }

    /// How many times a line came, changed or went.
    #[cfg(test)]
    pub(super) fn lined(&self) -> u64 {
        self.lined
    }

    /// Whether the lines changed since a reader last read them, and it was not told so since:
    /// told once, then not again until it reads them, however often they change meanwhile.
    pub(in crate::terminal) fn news(&mut self) -> bool {
        let news = self.lined != self.told_lined && !self.unread;
        if news {
            self.unread = true;
            self.told_lined = self.lined;
        }
        news
    }

    /// The lines, read: what changes after is news again.
    pub(in crate::terminal) fn read_lines(&mut self) -> Vec<Status> {
        self.unread = false;
        self.told_lined = self.lined;
        self.lines()
    }
}

#[cfg(test)]
#[path = "lines_tests.rs"]
mod tests;
