//! What the index holds, counted: the categories of what is shown, for a page's filters, and
//! each source with how much it lists and when it was read.

use rusqlite::params_from_iter;
use rusqlite::types::Value;

use crate::catalog::model::What;
use crate::store::Connection;

/// A source as the index holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Held {
    /// Its id.
    pub source: String,
    /// All it lists, those another source shadows too.
    pub listings: u64,
    /// When it was last read, in seconds of the epoch; 0 when never.
    pub read_at: u64,
}

/// The category that is always last.
const OTHER: &str = "Other";

/// How many listings that are shown (of `what`, of `source`, when given) each category has,
/// the biggest first and Other last.
pub(super) fn facets(
    connection: &Connection,
    what: Option<What>,
    source: Option<&str>,
) -> Vec<(String, u64)> {
    let mut sql = String::from("SELECT category, count(*) FROM listings WHERE shadowed = 0");
    let mut values = vec![];
    if let Some(what) = what {
        sql.push_str(" AND what = ?");
        values.push(Value::Text(what.name().to_string()));
    }
    if let Some(source) = source {
        sql.push_str(" AND source = ?");
        values.push(Value::Text(source.to_string()));
    }
    sql.push_str(" GROUP BY category");
    let Ok(mut counted) = connection.prepare_cached(&sql) else {
        return vec![];
    };
    let rows = counted.query_map(params_from_iter(values), |row| {
        Ok((row.get(0)?, row.get(1)?))
    });
    let mut facets: Vec<(String, u64)> = rows
        .map(|rows| rows.filter_map(Result::ok).collect())
        .unwrap_or_default();
    facets.sort_by(|(one, many), (other, more)| {
        ((one == OTHER).cmp(&(other == OTHER)))
            .then(more.cmp(many))
            .then(one.cmp(other))
    });
    facets
}

/// Every source the index has listings or a mark of, by its id.
pub(super) fn sources(connection: &Connection) -> Vec<Held> {
    let sql = "SELECT s.source,
            (SELECT count(*) FROM listings l WHERE l.source = s.source),
            coalesce((SELECT read_at FROM marks m WHERE m.source = s.source), 0)
        FROM (SELECT source FROM marks UNION SELECT DISTINCT source FROM listings) s
        ORDER BY s.source";
    let Ok(mut held) = connection.prepare_cached(sql) else {
        return vec![];
    };
    let rows = held.query_map([], |row| {
        Ok(Held {
            source: row.get(0)?,
            listings: row.get(1)?,
            read_at: row.get(2)?,
        })
    });
    rows.map(|rows| rows.filter_map(Result::ok).collect())
        .unwrap_or_default()
}

#[cfg(test)]
#[path = "held_tests.rs"]
mod tests;
