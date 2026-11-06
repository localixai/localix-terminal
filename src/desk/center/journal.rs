//! The Event Center's journal, `center_events`: every event the center showed, open or settled,
//! with when it first showed and what the person did with it. The center owns it: it writes the
//! events it gathered (one row an event, by its id), settles the rows it no longer gathers, and
//! keeps the person's marks. The marks outlive a restart; the open events are gathered again from
//! their owners, so the journal never shows what the owners do not hold.

use std::collections::{HashMap, HashSet};

use super::Event;
use crate::store::{Transaction, params};

/// How long a settled event is kept, in seconds: a month.
const KEPT: u64 = 30 * 24 * 3600;

/// What the person did with an event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Mark {
    Seen,
    Dismissed,
}

/// What the journal holds of an open event besides what its owner says.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct Kept {
    /// When the center first showed it.
    pub(super) first: u64,
    pub(super) seen: bool,
    pub(super) dismissed: bool,
}

/// The events gathered `now` written as the open ones: new rows for new ones, the rest as their
/// owners say them now, every other open row settled, and settled rows past [`KEPT`] gone. What
/// the journal keeps of each open one.
pub(super) fn record(
    tx: &Transaction<'_>,
    events: &[Event],
    now: u64,
) -> Result<HashMap<String, Kept>, String> {
    let text = |why: rusqlite::Error| why.to_string();
    for event in events {
        tx.execute(
            "INSERT INTO center_events (id, kind, project, tab, title, detail, at, first)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT (id) DO UPDATE SET kind = excluded.kind, project = excluded.project,
                 tab = excluded.tab, title = excluded.title, detail = excluded.detail,
                 at = excluded.at,
                 -- An id seen again after it settled, or of another project, is a new event
                 -- (ids count afresh each run): unseen, not dismissed, first shown now.
                 first = CASE WHEN center_events.settled IS NULL
                     AND center_events.project IS excluded.project
                     THEN center_events.first ELSE excluded.first END,
                 seen = CASE WHEN center_events.settled IS NULL
                     AND center_events.project IS excluded.project
                     THEN center_events.seen ELSE 0 END,
                 dismissed = CASE WHEN center_events.settled IS NULL
                     AND center_events.project IS excluded.project
                     THEN center_events.dismissed ELSE 0 END,
                 settled = NULL",
            params![
                event.id,
                event.kind.name(),
                crate::projects::of_key(&event.group),
                event.tab,
                event.title,
                event.detail,
                event.at,
                now
            ],
        )
        .map_err(text)?;
    }
    let open: HashSet<&str> = events.iter().map(|event| event.id.as_str()).collect();
    let mut kept = HashMap::new();
    let mut settle = vec![];
    {
        let mut rows = tx
            .prepare_cached(
                "SELECT id, first, seen, dismissed FROM center_events WHERE settled IS NULL",
            )
            .map_err(text)?;
        let read = rows
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    Kept {
                        first: row.get(1)?,
                        seen: row.get(2)?,
                        dismissed: row.get(3)?,
                    },
                ))
            })
            .map_err(text)?;
        for row in read {
            let (id, marks) = row.map_err(text)?;
            if open.contains(id.as_str()) {
                kept.insert(id, marks);
            } else {
                settle.push(id);
            }
        }
    }
    for id in settle {
        tx.execute(
            "UPDATE center_events SET settled = ?2 WHERE id = ?1",
            params![id, now],
        )
        .map_err(text)?;
    }
    tx.execute(
        "DELETE FROM center_events WHERE settled IS NOT NULL AND settled <= ?1",
        params![now.saturating_sub(KEPT)],
    )
    .map_err(text)?;
    Ok(kept)
}

/// The events `ids` marked as the person did.
pub(super) fn mark(tx: &Transaction<'_>, ids: &[String], mark: Mark) -> Result<(), String> {
    let text = |why: rusqlite::Error| why.to_string();
    let column = match mark {
        Mark::Seen => "UPDATE center_events SET seen = 1 WHERE id = ?1",
        Mark::Dismissed => "UPDATE center_events SET dismissed = 1 WHERE id = ?1",
    };
    for id in ids {
        tx.execute(column, [id]).map_err(text)?;
    }
    Ok(())
}
