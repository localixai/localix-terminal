//! A search of the index. No words lists everything, the most trusted sources first, those
//! that say what they do before those that do not, and each by its title, so browsing opens on
//! the curated lists. Words are each looked for as the start of a word, as a person types
//! them; what is found is ranked by how well it matches (a name and a title above a summary),
//! a listing named as typed first, the source's trust deciding between equals. What a person
//! types is never read as FTS5's own syntax. A listing another source lists more trusted is
//! never found: only its id finds it.

use rusqlite::params_from_iter;
use rusqlite::types::Value;

use crate::catalog::model::{Listing, What};
use crate::store::{Connection, OptionalExtension, params};

/// What is looked for.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Query {
    /// The words typed, as typed.
    pub text: String,
    /// Only listings of this kind.
    pub what: Option<What>,
    /// Only listings of this category, one of `category::ALL`.
    pub category: Option<String>,
    /// Only the listings of this source (its id).
    pub source: Option<String>,
    pub limit: usize,
    pub offset: usize,
}

/// What a search found: one page of it, and how many there are in all.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Found {
    pub total: u64,
    pub listings: Vec<Listing>,
}

/// A listing written, or changed only when something of it did. A listing whose origin is
/// none is the same as no other, so nothing shadows it.
pub(super) const PUT: &str = "
INSERT INTO listings (id, source, what, name, title, summary, author, category, homepage,
    version, recipe, trust, origin)
VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
ON CONFLICT (id) DO UPDATE SET
    source = excluded.source, what = excluded.what, name = excluded.name,
    title = excluded.title, summary = excluded.summary, author = excluded.author,
    category = excluded.category, homepage = excluded.homepage, version = excluded.version,
    recipe = excluded.recipe, trust = excluded.trust, origin = excluded.origin,
    shadowed = CASE WHEN excluded.origin = '' THEN 0 ELSE listings.shadowed END
WHERE listings.source IS NOT excluded.source OR listings.what IS NOT excluded.what
    OR listings.name IS NOT excluded.name OR listings.title IS NOT excluded.title
    OR listings.summary IS NOT excluded.summary OR listings.author IS NOT excluded.author
    OR listings.category IS NOT excluded.category OR listings.homepage IS NOT excluded.homepage
    OR listings.version IS NOT excluded.version OR listings.recipe IS NOT excluded.recipe
    OR listings.trust IS NOT excluded.trust OR listings.origin IS NOT excluded.origin";

/// The columns a listing is read from, of the table `l`.
const COLUMNS: &str = "l.id, l.what, l.source, l.name, l.title, l.summary, l.author, l.category,
    l.homepage, l.version, l.recipe, l.origin";

/// The order of browsing, as the browsing indexes keep it.
const BROWSED: &str = "l.trust DESC, l.summary = '', l.title COLLATE NOCASE, l.id";

/// The most words a search looks for, and the longest of them.
const WORDS_MOST: usize = 8;
const WORD_LONGEST: usize = 64;

/// What is typed as an FTS5 query: each word, as FTS5's tokenizer splits words, quoted and
/// looked for as the start of a word; none when nothing typed is a word.
pub(super) fn words(text: &str) -> Option<String> {
    let words: Vec<String> = (text.split(|c: char| !c.is_alphanumeric()))
        .filter(|word| !word.is_empty())
        .take(WORDS_MOST)
        .map(|word| {
            let word: String = word.chars().take(WORD_LONGEST).collect();
            format!("\"{}\"*", word.to_lowercase().replace('"', "\"\""))
        })
        .collect();
    (!words.is_empty()).then(|| words.join(" "))
}

/// A listing read from a row of [`COLUMNS`]; none when what the row keeps no longer reads.
fn listing(row: &rusqlite::Row<'_>) -> rusqlite::Result<Option<Listing>> {
    let what: String = row.get(1)?;
    let recipe: String = row.get(10)?;
    let (Some(what), Ok(recipe)) = (What::parse(&what), serde_json::from_str(&recipe)) else {
        return Ok(None);
    };
    Ok(Some(Listing {
        id: row.get(0)?,
        what,
        source: row.get(2)?,
        name: row.get(3)?,
        title: row.get(4)?,
        summary: row.get(5)?,
        author: row.get(6)?,
        category: row.get(7)?,
        homepage: row.get(8)?,
        version: row.get(9)?,
        origin: row.get(11)?,
        recipe,
    }))
}

/// The listing of `id`.
pub(super) fn get(connection: &Connection, id: &str) -> Option<Listing> {
    let sql = format!("SELECT {COLUMNS} FROM listings l WHERE l.id = ?1");
    let read = connection.query_row(&sql, params![id], listing);
    read.optional().ok().flatten().flatten()
}

/// The conditions of `query` other than its words, on the table `l`, each with its value.
/// Only those asked for are written, so each asks the index that serves it.
fn only(query: &Query) -> (String, Vec<Value>) {
    let mut sql = String::from("l.shadowed = 0");
    let mut values = vec![];
    let given = [
        ("l.what", query.what.map(|what| what.name().to_string())),
        ("l.category", query.category.clone()),
        ("l.source", query.source.clone()),
    ];
    for (column, value) in given {
        if let Some(value) = value {
            sql.push_str(" AND ");
            sql.push_str(column);
            sql.push_str(" = ?");
            values.push(Value::Text(value));
        }
    }
    (sql, values)
}

/// The rows `sql` reads with `values`, as listings.
fn listings(
    connection: &Connection,
    sql: &str,
    values: Vec<Value>,
) -> Result<Vec<Listing>, String> {
    let mut page = connection
        .prepare_cached(sql)
        .map_err(|why| why.to_string())?;
    let rows = page.query_map(params_from_iter(values), listing);
    let listings = rows
        .map_err(|why| why.to_string())?
        .filter_map(|row| row.ok().flatten());
    Ok(listings.collect())
}

/// What `query` finds in `connection`'s index.
pub(super) fn search(connection: &Connection, query: &Query) -> Result<Found, String> {
    let limit = Value::Integer(i64::try_from(query.limit).unwrap_or(i64::MAX));
    let offset = Value::Integer(i64::try_from(query.offset).unwrap_or(i64::MAX));
    let (only, mut values) = only(query);
    let count = |sql: &str, values: &[Value]| -> Result<u64, String> {
        (connection.query_row(sql, params_from_iter(values), |row| row.get(0)))
            .map_err(|why| why.to_string())
    };
    let Some(words) = words(&query.text) else {
        let total = count(
            &format!("SELECT count(*) FROM listings l WHERE {only}"),
            &values,
        )?;
        let page = format!(
            "SELECT {COLUMNS} FROM listings l WHERE {only} ORDER BY {BROWSED} LIMIT ? OFFSET ?"
        );
        values.extend([limit, offset]);
        let listings = listings(connection, &page, values)?;
        return Ok(Found { total, listings });
    };
    values.insert(0, Value::Text(words));
    let found =
        format!("FROM words JOIN listings l ON l.n = words.rowid WHERE words MATCH ? AND {only}");
    let total = count(&format!("SELECT count(*) {found}"), &values)?;
    values.extend([Value::Text(query.text.trim().to_lowercase()), limit, offset]);
    // The name typed is the last value but the page's two; bm25's weights are the columns':
    // name, title, summary, author, category.
    let named = values.len() - 2;
    let page = format!(
        "SELECT {COLUMNS} {found}
         ORDER BY (lower(l.name) = ?{named} OR lower(l.title) = ?{named}) DESC,
             bm25(words, 10.0, 10.0, 2.0, 1.0, 1.0), l.trust DESC, l.title COLLATE NOCASE, l.id
         LIMIT ?{} OFFSET ?{}",
        named + 1,
        named + 2
    );
    let listings = listings(connection, &page, values)?;
    Ok(Found { total, listings })
}

#[cfg(test)]
#[path = "search_tests.rs"]
mod tests;
