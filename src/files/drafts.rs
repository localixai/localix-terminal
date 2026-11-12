//! What a person left unsaved in a file, kept by the file's path until they save it or make
//! the text the disk's again: closing a tab or quitting the app loses nothing, and asks nothing.

use std::path::Path;

use super::text::Stamp;
use crate::store::{Connection, Transaction, params};

/// What was left unsaved in a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Draft {
    pub text: String,
    /// The length and hash of the bytes the disk held when it was written.
    pub based: (u64, u64),
}

impl Draft {
    /// Whether the disk, holding what `now` names, changed since the draft was written.
    pub fn moved(&self, now: &Stamp) -> bool {
        self.based != now.bytes()
    }
}

/// The hash as the store keeps it: the same bits, signed.
fn kept(hash: u64) -> i64 {
    i64::from_ne_bytes(hash.to_ne_bytes())
}

/// What was left unsaved in the file at `path`.
pub fn draft(db: &Connection, path: &Path) -> Result<Option<Draft>, String> {
    let mut read = db
        .prepare_cached("SELECT text, len, hash FROM drafts WHERE path = ?1")
        .map_err(|why| why.to_string())?;
    let mut rows = read
        .query_map(params![path.to_string_lossy()], |row| {
            let (len, hash): (i64, i64) = (row.get(1)?, row.get(2)?);
            Ok(Draft {
                text: row.get(0)?,
                based: (
                    u64::try_from(len).unwrap_or(0),
                    u64::from_ne_bytes(hash.to_ne_bytes()),
                ),
            })
        })
        .map_err(|why| why.to_string())?;
    rows.next().transpose().map_err(|why| why.to_string())
}

/// How many files keep what is unsaved in them at once.
pub const MOST: u32 = 64;

/// `text` kept as what is unsaved in the file at `path`, written while the disk held `base`
/// (none: the file was gone).
pub fn keep(
    tx: &Transaction<'_>,
    path: &Path,
    text: &str,
    base: Option<&Stamp>,
) -> Result<(), String> {
    let (len, hash) = base.map_or((0, 0), Stamp::bytes);
    tx.execute(
        "INSERT INTO drafts (path, text, len, hash, turn)
         VALUES (?1, ?2, ?3, ?4, (SELECT COALESCE(MAX(turn), 0) + 1 FROM drafts))
         ON CONFLICT (path) DO UPDATE SET text = ?2, len = ?3, hash = ?4, turn = excluded.turn",
        params![
            path.to_string_lossy(),
            text,
            i64::try_from(len).unwrap_or(i64::MAX),
            kept(hash)
        ],
    )
    .map_err(|why| why.to_string())?;
    // A file whose tab was closed, or that went from the disk, keeps its text with no one to
    // forget it: past `MOST` files, the ones written longest ago go.
    tx.execute(
        "DELETE FROM drafts WHERE path IN
         (SELECT path FROM drafts ORDER BY turn DESC LIMIT -1 OFFSET ?1)",
        params![MOST],
    )
    .map(drop)
    .map_err(|why| why.to_string())
}

/// Nothing is unsaved in the file at `path` any more.
pub fn forget(tx: &Transaction<'_>, path: &Path) -> Result<(), String> {
    tx.execute(
        "DELETE FROM drafts WHERE path = ?1",
        params![path.to_string_lossy()],
    )
    .map(drop)
    .map_err(|why| why.to_string())
}

#[cfg(test)]
#[path = "drafts_tests.rs"]
mod tests;
