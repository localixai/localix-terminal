//! The index's tables, made whole in one step. The file is derived from what the sources say,
//! so it is never migrated: a file of another version (or one that is no database) is removed
//! and made again, and the sources read anew.

use std::path::Path;

use crate::store::Connection;

/// This build's version of the tables. A change to [`TABLES`] is a new number.
pub(super) const VERSION: i64 = 2;

/// The listings; their words, searched with FTS5 and kept in step with them by triggers in
/// each write's own transaction; and where each source was last read to. A listing that is
/// the same thing as another's (one origin, one kind) and less trusted is `shadowed`: kept,
/// and found by its id, but not browsed, searched or counted. Browsing pages through the
/// partial indexes of what is not shadowed, in the order a page shows: the most trusted first,
/// those that say what they do before those that do not, then by title.
const TABLES: &str = "
CREATE TABLE listings (
    n INTEGER PRIMARY KEY,
    id TEXT NOT NULL UNIQUE,
    source TEXT NOT NULL,
    what TEXT NOT NULL,
    name TEXT NOT NULL,
    title TEXT NOT NULL,
    summary TEXT NOT NULL,
    author TEXT NOT NULL,
    category TEXT NOT NULL,
    homepage TEXT NOT NULL,
    version TEXT NOT NULL,
    recipe TEXT NOT NULL,
    trust INTEGER NOT NULL,
    origin TEXT NOT NULL,
    shadowed INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX listings_source ON listings (source, what, category);
CREATE INDEX listings_origin ON listings (origin, what) WHERE origin <> '';
CREATE INDEX listings_browse ON listings
    (trust DESC, summary = '', title COLLATE NOCASE, id) WHERE shadowed = 0;
CREATE INDEX listings_what ON listings
    (what, trust DESC, summary = '', title COLLATE NOCASE, id) WHERE shadowed = 0;
CREATE INDEX listings_kind ON listings
    (what, category, trust DESC, summary = '', title COLLATE NOCASE, id) WHERE shadowed = 0;
CREATE INDEX listings_category ON listings
    (category, trust DESC, summary = '', title COLLATE NOCASE, id) WHERE shadowed = 0;
CREATE VIRTUAL TABLE words USING fts5(
    name, title, summary, author, category,
    content = 'listings', content_rowid = 'n',
    tokenize = 'unicode61 remove_diacritics 2', prefix = '2 3'
);
CREATE TRIGGER listings_put AFTER INSERT ON listings BEGIN
    INSERT INTO words (rowid, name, title, summary, author, category)
    VALUES (new.n, new.name, new.title, new.summary, new.author, new.category);
END;
CREATE TRIGGER listings_gone AFTER DELETE ON listings BEGIN
    INSERT INTO words (words, rowid, name, title, summary, author, category)
    VALUES ('delete', old.n, old.name, old.title, old.summary, old.author, old.category);
END;
CREATE TRIGGER listings_changed AFTER UPDATE OF name, title, summary, author, category
ON listings BEGIN
    INSERT INTO words (words, rowid, name, title, summary, author, category)
    VALUES ('delete', old.n, old.name, old.title, old.summary, old.author, old.category);
    INSERT INTO words (rowid, name, title, summary, author, category)
    VALUES (new.n, new.name, new.title, new.summary, new.author, new.category);
END;
CREATE TABLE marks (
    source TEXT PRIMARY KEY,
    mark TEXT NOT NULL,
    read_at INTEGER NOT NULL,
    more INTEGER NOT NULL
);
";

/// `connection` made ready: its tables made when it has none, as they are when it is of this
/// version; an error when it is of another.
pub(super) fn ready(connection: &mut Connection) -> Result<(), String> {
    let failed = |why: rusqlite::Error| why.to_string();
    connection
        .execute_batch("PRAGMA synchronous = NORMAL; PRAGMA temp_store = MEMORY;")
        .map_err(failed)?;
    let version: i64 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(failed)?;
    if version == VERSION {
        return Ok(());
    }
    let tables: i64 = connection
        .query_row("SELECT count(*) FROM sqlite_master", [], |row| row.get(0))
        .map_err(failed)?;
    if version != 0 || tables != 0 {
        return Err(format!("the index is of version {version}, not {VERSION}"));
    }
    let made = connection.transaction().map_err(failed)?;
    made.execute_batch(TABLES).map_err(failed)?;
    made.execute_batch(&format!("PRAGMA user_version = {VERSION}"))
        .map_err(failed)?;
    made.commit().map_err(failed)
}

/// The file at `path` opened and made ready; one that cannot be is removed, with what SQLite
/// keeps beside it, and made again.
pub(super) fn open(path: &Path) -> Result<Connection, String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|why| format!("{}: {why}", parent.display()))?;
    }
    let opened = || -> Result<Connection, String> {
        let mut connection = Connection::open(path).map_err(|why| why.to_string())?;
        ready(&mut connection).map(|()| connection)
    };
    opened().or_else(|why| {
        log::info!("catalog: {} made again ({why})", path.display());
        for end in ["", "-journal", "-wal", "-shm"] {
            let mut file = path.as_os_str().to_os_string();
            file.push(end);
            let _ = std::fs::remove_file(file);
        }
        opened().map_err(|why| format!("{}: {why}", path.display()))
    })
}

#[cfg(test)]
#[path = "schema_tests.rs"]
mod tests;
