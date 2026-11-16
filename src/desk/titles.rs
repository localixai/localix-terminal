//! A terminal's title in the ledger (ledger.rs): a program that animates its title says it many
//! times a second, so it is written at most once a [`TITLE_EVERY`], the latest kept for the
//! next write or for when its row closes.

use super::super::Desk;
use crate::ledger;

/// How often a terminal's title is written to the ledger at most: a program that animates its
/// title says it many times a second.
const TITLE_EVERY: std::time::Duration = std::time::Duration::from_secs(1);

/// What a terminal's title writes have come to: when the last was done, and the latest title
/// since, kept for the next write or for when the row closes.
pub(in super::super) struct Titled {
    at: std::time::Instant,
    pub(super) pending: Option<String>,
}

impl Desk {
    /// Terminal `key` is titled `title` (its program says what it does): kept in the ledger, at
    /// most once a [`TITLE_EVERY`]; the latest title waits for the next write, or for its row to
    /// close. The window is told the past again when a write is news: its history menu lists the
    /// title of an open terminal from there.
    pub(in super::super) fn retitled(&self, key: &str, title: &str) {
        let now = std::time::Instant::now();
        {
            let mut titled = crate::lock(&self.titled);
            if let Some(last) = titled.get_mut(key)
                && now.duration_since(last.at) < TITLE_EVERY
            {
                last.pending = Some(title.to_string());
                return;
            }
            titled.insert(
                key.to_string(),
                Titled {
                    at: now,
                    pending: None,
                },
            );
        }
        self.write_title(key, title);
    }

    /// The latest title of terminal `key` still waiting is written.
    pub(super) fn flush_title(&self, key: &str) {
        let pending = (crate::lock(&self.titled).remove(key)).and_then(|titled| titled.pending);
        if let Some(title) = pending {
            self.write_title(key, &title);
        }
    }

    fn write_title(&self, key: &str, title: &str) {
        let Some((row, project)) = crate::lock(&self.ledger).get(key).copied() else {
            return;
        };
        let titled = self.feed.write(|write| {
            ledger::titled(write.tx(), row, title)
                .and_then(|news| Self::news_of(write, project, news))
        });
        if let Err(why) = titled {
            log::warn!("tab {key}'s title is not in the ledger: {why}");
        }
    }
}
