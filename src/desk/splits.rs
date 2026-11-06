//! A project's split window kept across restarts: the window tells each change of it by its
//! tabs' keys, the app keeps it by their rows, and once the project's tabs open again (each a
//! new row) the window is told to split as it was.

use std::collections::HashMap;

use super::Desk;
use crate::browser::Halves;
use crate::ledger::Row;
use crate::store::{OptionalExtension as _, Transaction};

/// A project's split as kept: the tab in use and the one beside (their rows), and whether that
/// one is on the right.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) struct Kept {
    pub(super) shown: Row,
    pub(super) beside: Row,
    pub(super) right: bool,
}

/// Where project `group`'s split is kept, in the app's state.
fn place(group: &str) -> String {
    format!("split {group}")
}

/// Project `group`'s split kept (or none), in `tx`.
pub(super) fn keep(tx: &Transaction<'_>, group: &str, kept: Option<Kept>) -> Result<(), String> {
    let key = place(group);
    let done = match kept {
        Some(kept) => {
            let value = serde_json::to_string(&kept).map_err(|why| why.to_string())?;
            tx.execute(
                "INSERT INTO state (key, value) VALUES (?1, ?2)
                 ON CONFLICT (key) DO UPDATE SET value = ?2",
                [&key, &value],
            )
        }
        None => tx.execute("DELETE FROM state WHERE key = ?1", [&key]),
    };
    done.map(drop).map_err(|why| why.to_string())
}

/// Project `group`'s split, when one is kept.
pub(super) fn kept(db: &crate::store::Connection, group: &str) -> Result<Option<Kept>, String> {
    let value: Option<String> = db
        .query_row(
            "SELECT value FROM state WHERE key = ?1",
            [place(group)],
            |row| row.get(0),
        )
        .optional()
        .map_err(|why| why.to_string())?;
    Ok(value.and_then(|value| serde_json::from_str(&value).ok()))
}

/// The halves of split `kept` by the keys its rows' tabs have now (`keys`), when both are open.
pub(super) fn halves<'a>(kept: &Kept, keys: &'a HashMap<Row, String>) -> Option<Halves<'a>> {
    Some(Halves {
        shown: keys.get(&kept.shown)?,
        beside: keys.get(&kept.beside)?,
        right: kept.right,
    })
}

impl Desk {
    /// The person split the window in project `group` (`halves`, by its tabs' keys), or made it
    /// whole there: kept by the tabs' rows.
    pub(super) fn split_told(&self, group: &str, halves: Option<Halves<'_>>) {
        // The app ending, its tabs go: the split stays kept for when they open again.
        if self.ending.load(std::sync::atomic::Ordering::SeqCst) {
            return;
        }
        let row = |key: &str| crate::lock(&self.ledger).get(key).map(|(row, _)| *row);
        let kept = halves.and_then(|halves| {
            Some(Kept {
                shown: row(halves.shown)?,
                beside: row(halves.beside)?,
                right: halves.right,
            })
        });
        if let Err(why) = (self.feed.store()).write(|tx| keep(tx, group, kept)) {
            log::warn!("the split of {group} was not kept: {why}");
        }
    }
}

#[cfg(test)]
#[path = "splits_tests.rs"]
mod tests;
