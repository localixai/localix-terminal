//! Which of several listings of one thing is shown. Two sources that list the same plugin or
//! skill give it one origin (`source::origin`); of the listings of one kind and one origin,
//! the most trusted is shown and the rest are shadowed, the source and then the id deciding
//! between equals. It is worked out again for each origin a write touches, in that write's
//! transaction, so it is never out of step with the listings.

use std::collections::BTreeSet;

use crate::store::{Connection, OptionalExtension, params};

/// The origins a write touched, each with the kind of its listing.
pub(super) type Touched = BTreeSet<(String, String)>;

/// Of the listings of `what` from `origin`, the one shown. Each statement says the origin is
/// not empty, so SQLite takes the index of the origins, which holds only those.
const KEPT: &str = "SELECT id FROM listings WHERE origin = ?1 AND origin <> '' AND what = ?2
    ORDER BY trust DESC, source, id LIMIT 1";

/// Every listing of `what` from `origin` shadowed but `kept`, changed only where it differs.
const SHADOWED: &str = "UPDATE listings SET shadowed = (id <> ?3)
    WHERE origin = ?1 AND origin <> '' AND what = ?2 AND shadowed IS NOT (id <> ?3)";

/// The listings of each origin in `touched` shadowed or shown as their trust says.
pub(super) fn recompute(connection: &Connection, touched: &Touched) -> Result<(), String> {
    let failed = |why: rusqlite::Error| why.to_string();
    let mut kept = connection.prepare_cached(KEPT).map_err(failed)?;
    let mut shadowed = connection.prepare_cached(SHADOWED).map_err(failed)?;
    for (origin, what) in touched.iter().filter(|(origin, _)| !origin.is_empty()) {
        let shown: Option<String> = (kept.query_row(params![origin, what], |row| row.get(0)))
            .optional()
            .map_err(failed)?;
        if let Some(shown) = shown {
            shadowed
                .execute(params![origin, what, shown])
                .map_err(failed)?;
        }
    }
    Ok(())
}

/// The sources of the other listings of what `id` is: of its kind and origin, the most
/// trusted first, each source once, its own left out.
pub(super) fn also_in(connection: &Connection, id: &str) -> Vec<String> {
    let sql = "SELECT o.source FROM listings l
        JOIN listings o ON o.origin = l.origin AND o.origin <> '' AND o.what = l.what
            AND o.source <> l.source
        WHERE l.id = ?1 AND l.origin <> ''
        ORDER BY o.trust DESC, o.source, o.id";
    let Ok(mut also) = connection.prepare_cached(sql) else {
        return vec![];
    };
    let Ok(rows) = also.query_map(params![id], |row| row.get::<_, String>(0)) else {
        return vec![];
    };
    let mut sources: Vec<String> = vec![];
    for source in rows.filter_map(Result::ok) {
        if !sources.contains(&source) {
            sources.push(source);
        }
    }
    sources
}

#[cfg(test)]
#[path = "shadow_tests.rs"]
mod tests;
