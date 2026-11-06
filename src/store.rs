//! What the desk keeps across restarts, in one SQLite database: the projects and what is theirs
//! (their tabs, the pages they showed, the terminals' last screens, the boards, the Event
//! Center's journal), the change feed, the person's answers to offers and the app's state.
//!
//! One connection, one writer: every write is a transaction of its own, taken under the store's
//! lock, and an owner records its notice in the same transaction as its change (see `feed`). The
//! schema is the list of steps in [`STEPS`], applied in order on open and counted in SQLite's
//! `user_version`: a new table is a step at the end, never an edit of one already shipped.

use std::path::Path;
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

pub use rusqlite::{Connection, OptionalExtension, Transaction, params};

/// The schema, step by step. Each step runs once, in a transaction, in this order.
const STEPS: &[&str] = &[
    // 1: the projects and everything that is theirs (docs/projects.md), the change feed (an
    // outbox: a notice is written with the change it announces) and each consumer's place in it,
    // the person's answers to offers, and the app's own state.
    "CREATE TABLE projects (
        id INTEGER PRIMARY KEY,
        path TEXT NOT NULL UNIQUE,
        name TEXT NOT NULL,
        created INTEGER NOT NULL,
        opened INTEGER NOT NULL
    );
    CREATE TABLE tabs (
        id INTEGER PRIMARY KEY,
        project INTEGER NOT NULL REFERENCES projects ON DELETE CASCADE,
        kind TEXT NOT NULL,
        agent TEXT,
        title TEXT NOT NULL DEFAULT '',
        url TEXT,
        cwd TEXT,
        command TEXT,
        place INTEGER NOT NULL,
        opened INTEGER NOT NULL,
        shown INTEGER,
        closed INTEGER
    );
    CREATE INDEX tabs_of_project ON tabs (project, closed);
    CREATE TABLE visits (
        id INTEGER PRIMARY KEY,
        project INTEGER NOT NULL REFERENCES projects ON DELETE CASCADE,
        tab INTEGER REFERENCES tabs ON DELETE SET NULL,
        url TEXT NOT NULL,
        title TEXT NOT NULL,
        at INTEGER NOT NULL
    );
    CREATE INDEX visits_of_project ON visits (project, at);
    CREATE TABLE screens (
        tab INTEGER PRIMARY KEY REFERENCES tabs ON DELETE CASCADE,
        text TEXT NOT NULL,
        at INTEGER NOT NULL
    );
    CREATE TABLE boards (
        project INTEGER PRIMARY KEY REFERENCES projects ON DELETE CASCADE,
        board TEXT NOT NULL,
        updated INTEGER NOT NULL
    );
    CREATE TABLE center_events (
        id TEXT PRIMARY KEY,
        kind TEXT NOT NULL,
        project INTEGER REFERENCES projects ON DELETE CASCADE,
        tab TEXT NOT NULL,
        title TEXT NOT NULL,
        detail TEXT NOT NULL,
        at INTEGER NOT NULL,
        first INTEGER NOT NULL,
        settled INTEGER,
        seen INTEGER NOT NULL DEFAULT 0,
        dismissed INTEGER NOT NULL DEFAULT 0
    );
    CREATE INDEX center_events_open ON center_events (settled);
    CREATE TABLE answers (
        offer TEXT PRIMARY KEY,
        taken INTEGER NOT NULL DEFAULT 0,
        later INTEGER NOT NULL DEFAULT 0,
        until INTEGER NOT NULL DEFAULT 0
    );
    CREATE TABLE state (
        key TEXT PRIMARY KEY,
        value TEXT NOT NULL
    );
    CREATE TABLE change_events (
        seq INTEGER PRIMARY KEY AUTOINCREMENT,
        entity TEXT NOT NULL,
        op TEXT NOT NULL,
        at INTEGER NOT NULL,
        run INTEGER NOT NULL
    );
    CREATE TABLE change_cursors (
        name TEXT PRIMARY KEY,
        seq INTEGER NOT NULL,
        moved INTEGER NOT NULL
    );",
];

/// The database, shared by every part that keeps something in it.
pub struct Store {
    connection: Mutex<Connection>,
}

impl Store {
    /// The database at `path`, made with its directory if there is none, its schema brought up to
    /// date.
    ///
    /// # Errors
    /// It could not be opened, or its schema is newer than this build knows.
    pub fn open(path: &Path) -> Result<Self, String> {
        let failed = |why: &dyn std::fmt::Display| format!("{}: {why}", path.display());
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|why| failed(&why))?;
        }
        let connection = Connection::open(path).map_err(|why| failed(&why))?;
        connection
            .pragma_update(None, "journal_mode", "WAL")
            .map_err(|why| failed(&why))?;
        // In WAL a commit then waits for no disk sync: the database stays whole on any crash; a
        // power cut may lose the last moment's commits.
        connection
            .pragma_update(None, "synchronous", "NORMAL")
            .map_err(|why| failed(&why))?;
        Self::ready(connection).map_err(|why| failed(&why))
    }

    /// A database in memory, gone with the store: for checks, and for a desk that could not open
    /// its own.
    ///
    /// # Errors
    /// SQLite gave no database in memory.
    pub fn memory() -> Result<Self, String> {
        let connection = Connection::open_in_memory().map_err(|why| why.to_string())?;
        Self::ready(connection)
    }

    fn ready(mut connection: Connection) -> Result<Self, String> {
        let text = |why: rusqlite::Error| why.to_string();
        connection
            .busy_timeout(Duration::from_secs(5))
            .map_err(text)?;
        connection
            .pragma_update(None, "foreign_keys", "ON")
            .map_err(text)?;
        let done: usize = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(text)?;
        if done > STEPS.len() {
            return Err(format!(
                "its schema is at step {done}, past the {} this build knows: a newer llx-desk \
                 wrote it",
                STEPS.len()
            ));
        }
        for (at, step) in STEPS.iter().enumerate().skip(done) {
            let tx = connection.transaction().map_err(text)?;
            tx.execute_batch(step).map_err(text)?;
            tx.pragma_update(None, "user_version", at + 1)
                .map_err(text)?;
            tx.commit().map_err(text)?;
        }
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    fn connection(&self) -> MutexGuard<'_, Connection> {
        crate::lock(&self.connection)
    }

    /// `write` done in one transaction: committed when it answers `Ok`, undone otherwise.
    ///
    /// # Errors
    /// What `write` answered, or why SQLite could not.
    pub fn write<T>(
        &self,
        write: impl FnOnce(&Transaction<'_>) -> Result<T, String>,
    ) -> Result<T, String> {
        let mut connection = self.connection();
        let tx = connection.transaction().map_err(|why| why.to_string())?;
        let answer = write(&tx)?;
        tx.commit().map_err(|why| why.to_string())?;
        Ok(answer)
    }

    /// `read` done on the database as it is now.
    ///
    /// # Errors
    /// What `read` answered.
    pub fn read<T>(
        &self,
        read: impl FnOnce(&Connection) -> Result<T, String>,
    ) -> Result<T, String> {
        read(&self.connection())
    }
}

#[cfg(test)]
#[path = "store_tests.rs"]
mod tests;
