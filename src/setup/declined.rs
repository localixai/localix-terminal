//! What the person took out of the integrations: kept in the store's `state` as
//! `integration.declined.<id>`, so the app leaves it out until they put it back.

use crate::store::{OptionalExtension as _, Store};

/// The store's key of "the person took integration `id` out".
fn key(id: &str) -> String {
    format!("integration.declined.{id}")
}

/// Whether the person took integration `id` out: the app leaves it out. A store that cannot be
/// read (a newer build's schema, a database busy for long) counts as taken out: what the person
/// removed is never written back on a guess.
pub(super) fn declined(id: &str) -> bool {
    read(id).unwrap_or_else(|why| {
        log::warn!(
            "setup: whether the person took {id} out is unknown, so it is left alone: {why}"
        );
        true
    })
}

fn read(id: &str) -> Result<bool, String> {
    let store = Store::open(&crate::paths::database())?;
    let key = key(id);
    store.read(|db| {
        db.query_row("SELECT 1 FROM state WHERE key = ?1", [&key], |_| Ok(()))
            .optional()
            .map(|found| found.is_some())
            .map_err(|why| why.to_string())
    })
}

/// The person took integration `id` out (`out`), or put it back.
pub(super) fn remember(id: &str, out: bool) {
    let written = Store::open(&crate::paths::database()).and_then(|store| {
        let key = key(id);
        store.write(|tx| {
            let done = match out {
                true => tx.execute(
                    "INSERT INTO state (key, value) VALUES (?1, '1')
                     ON CONFLICT (key) DO UPDATE SET value = '1'",
                    [&key],
                ),
                false => tx.execute("DELETE FROM state WHERE key = ?1", [&key]),
            };
            done.map(drop).map_err(|why| why.to_string())
        })
    });
    if let Err(why) = written {
        log::warn!("setup: what the person did to {id} is not remembered: {why}");
    }
}
