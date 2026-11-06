//! What the app offers the person, and what they answered: any part of the app may offer
//! something (an integration where its agent runs, a setting worth turning on), by an id of its
//! own. The window shows it at once, in a bar under the strip, with a button that does it and
//! Not Now. Taken, it is never offered again; put off, it waits a while, longer each time
//! (a day, three, a week, then a month). Kept in the store (`answers`): the person's answers
//! outlive the app. The window asks through `browser::Answers`, which [`Kept`] is.

use std::sync::Arc;

use rusqlite::{Connection, OptionalExtension, Transaction, params};

use crate::store::Store;

/// How long an offer put off waits, by how many times it was: the last holds after.
const WAITS: [u64; 4] = [24 * 3600, 3 * 24 * 3600, 7 * 24 * 3600, 30 * 24 * 3600];

/// What the person answered to one offer.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Answer {
    /// They took it: it is not offered again.
    pub taken: bool,
    /// How many times they put it off.
    pub later: u32,
    /// Not offered before this time (seconds since the epoch).
    pub until: u64,
}

impl Answer {
    /// Whether the offer may be shown `now`.
    pub fn due(self, now: u64) -> bool {
        !self.taken && self.until <= now
    }

    /// Put off `now`: it waits longer than the time before.
    #[must_use]
    pub fn later(self, now: u64) -> Self {
        let at = usize::try_from(self.later).unwrap_or(usize::MAX);
        let wait = WAITS.get(at).or(WAITS.last()).copied().unwrap_or_default();
        Self {
            later: self.later.saturating_add(1),
            until: now.saturating_add(wait),
            ..self
        }
    }
}

/// The answer to offer `id` (none yet: the default).
///
/// # Errors
/// The store could not be read.
pub fn read(db: &Connection, id: &str) -> Result<Answer, String> {
    let found = db
        .query_row(
            "SELECT taken, later, until FROM answers WHERE offer = ?1",
            [id],
            |row| {
                Ok(Answer {
                    taken: row.get(0)?,
                    later: row.get(1)?,
                    until: row.get(2)?,
                })
            },
        )
        .optional()
        .map_err(|why| why.to_string())?;
    Ok(found.unwrap_or_default())
}

/// The answer to offer `id` kept.
///
/// # Errors
/// The store refused it.
pub fn write(tx: &Transaction<'_>, id: &str, answer: Answer) -> Result<(), String> {
    tx.execute(
        "INSERT INTO answers (offer, taken, later, until) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT (offer) DO UPDATE SET taken = excluded.taken, later = excluded.later,
             until = excluded.until",
        params![id, answer.taken, answer.later, answer.until],
    )
    .map_err(|why| why.to_string())?;
    Ok(())
}

/// The person's answers, in the store.
pub struct Kept(pub Arc<Store>);

impl crate::browser::Answers for Kept {
    fn due(&self, id: &str) -> bool {
        let answer = self.0.read(|db| read(db, id));
        answer.is_ok_and(|answer| answer.due(crate::board::now()))
    }

    fn answered(&self, id: &str, taken: bool) {
        let kept = self.0.write(|tx| {
            let answer = read(tx, id)?;
            let answer = match taken {
                true => Answer {
                    taken: true,
                    ..answer
                },
                false => answer.later(crate::board::now()),
            };
            write(tx, id, answer)
        });
        if let Err(why) = kept {
            log::warn!("offers: the answer was not kept: {why}");
        }
    }
}

#[cfg(test)]
#[path = "offers_tests.rs"]
mod tests;
