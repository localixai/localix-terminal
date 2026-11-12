//! What changes the listings: a source's batch written, a source forgotten. Each is done in
//! the caller's transaction, with every origin it touched shadowed again (`shadow.rs`), and
//! each listing's category put in the Store's own list as it is written (`category.rs`).

use super::search;
use super::shadow::{self, Touched};
use crate::catalog::category;
use crate::catalog::source::Batch;
use crate::store::{Connection, OptionalExtension, params};

/// What `source`, ranked at `trust`, read in `batch`: its listings written, those gone removed
/// (all it had but these, when the batch is whole). Its mark is the caller's.
pub(super) fn put(
    connection: &Connection,
    source: &str,
    trust: u8,
    batch: &Batch,
) -> Result<(), String> {
    let failed = |why: rusqlite::Error| why.to_string();
    let mut touched = Touched::new();
    {
        let mut gone = connection
            .prepare_cached(
                "DELETE FROM listings WHERE id = ?1 AND source = ?2 RETURNING origin, what",
            )
            .map_err(failed)?;
        for id in &batch.gone {
            let rows = gone.query_map(params![id, source], |row| Ok((row.get(0)?, row.get(1)?)));
            for row in rows.map_err(failed)? {
                touched.insert(row.map_err(failed)?);
            }
        }
        let mut was = connection
            .prepare_cached("SELECT origin, what FROM listings WHERE id = ?1")
            .map_err(failed)?;
        let mut put = connection.prepare_cached(search::PUT).map_err(failed)?;
        for listing in &batch.listings {
            let recipe = serde_json::to_string(&listing.recipe)
                .map_err(|why| format!("{}: {why}", listing.id))?;
            let before = (was.query_row(params![listing.id], |row| Ok((row.get(0)?, row.get(1)?))))
                .optional()
                .map_err(failed)?;
            touched.extend(before);
            touched.insert((listing.origin.clone(), listing.what.name().to_string()));
            let placed = category::of(&listing.category, &listing.name, &listing.summary);
            put.execute(params![
                listing.id,
                source,
                listing.what.name(),
                listing.name,
                listing.title,
                listing.summary,
                listing.author,
                placed,
                listing.homepage,
                listing.version,
                recipe,
                trust,
                listing.origin,
            ])
            .map_err(failed)?;
        }
    }
    if batch.whole {
        let ids: Vec<&str> = (batch.listings.iter())
            .map(|listing| listing.id.as_str())
            .collect();
        let ids = serde_json::to_string(&ids).map_err(|why| why.to_string())?;
        removed(
            connection,
            "DELETE FROM listings WHERE source = ?1
             AND id NOT IN (SELECT value FROM json_each(?2)) RETURNING origin, what",
            params![source, ids],
            &mut touched,
        )?;
    }
    shadow::recompute(connection, &touched)
}

/// Every listing of `source` removed, and its mark: what it shadowed is shown again.
pub(super) fn forget(connection: &Connection, source: &str) -> Result<(), String> {
    let mut touched = Touched::new();
    removed(
        connection,
        "DELETE FROM listings WHERE source = ?1 RETURNING origin, what",
        params![source],
        &mut touched,
    )?;
    connection
        .execute("DELETE FROM marks WHERE source = ?1", params![source])
        .map_err(|why| why.to_string())?;
    shadow::recompute(connection, &touched)
}

/// The listings `sql` deletes (returning each one's origin and kind), their origins added to
/// `touched`.
fn removed(
    connection: &Connection,
    sql: &str,
    values: &[&dyn rusqlite::ToSql],
    touched: &mut Touched,
) -> Result<(), String> {
    let failed = |why: rusqlite::Error| why.to_string();
    let mut removed = connection.prepare(sql).map_err(failed)?;
    let rows = removed.query_map(values, |row| Ok((row.get(0)?, row.get(1)?)));
    for row in rows.map_err(failed)? {
        touched.insert(row.map_err(failed)?);
    }
    Ok(())
}
