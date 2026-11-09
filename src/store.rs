//! What the desk keeps across restarts, in one SQLite database: the projects and what is theirs
//! (their tabs, the pages they showed, the terminals' last screens, the boards, the Event
//! Center's journal), the change feed, the person's answers to offers and the app's state.
//!
//! One connection, one writer: every write is a transaction of its own, taken under the store's
//! lock, and an owner records its notice in the same transaction as its change (see `feed`). The
//! schema is the list of steps in `store/schema.rs`, applied in order on open and counted in SQLite's
//! `user_version`: a new table is a step at the end, never an edit of one already shipped.

mod schema;
use schema::STEPS;

use std::path::Path;
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use rusqlite::TransactionBehavior;
pub use rusqlite::{Connection, OptionalExtension, Transaction, params};

/// How long a write waits for another connection to the same database (the CLI's) to let go.
const BUSY: Duration = Duration::from_secs(5);

/// How many prepared statements the connection keeps for reuse: the queries the desk runs often.
const STATEMENTS: usize = 64;

/// The database, shared by every part that keeps something in it.
pub struct Store {
    connection: Mutex<Connection>,
    /// Why this store is in memory when the database could not be opened: all it holds is gone
    /// when the app quits.
    instead: Option<String>,
}

impl Store {
    /// The database at `path`, made with its directory if there is none, its schema brought up to
    /// date.
    ///
    /// # Errors
    /// It could not be opened, or its schema is newer than this build knows.
    pub fn open(path: &Path) -> Result<Self, String> {
        let failed = |why: &dyn std::fmt::Display| format!("{}: {why}", path.display());
        Self::private(path).map_err(|why| failed(&why))?;
        let connection = Connection::open(path).map_err(|why| failed(&why))?;
        // The wait comes first: the CLI may hold the database as the app opens it, and even the
        // switch to WAL below needs it for a moment.
        connection.busy_timeout(BUSY).map_err(|why| failed(&why))?;
        Self::wal(&connection).map_err(|why| failed(&why))?;
        // In WAL a commit then waits for no disk sync: the database stays whole on any crash; a
        // power cut may lose the last moment's commits.
        connection
            .pragma_update(None, "synchronous", "NORMAL")
            .map_err(|why| failed(&why))?;
        Self::ready(connection).map_err(|why| failed(&why))
    }

    /// The database in WAL, which it keeps once any connection set it. On a new database the
    /// switch needs the file to itself: two processes switching at once each find the other's
    /// hold, and SQLite answers "locked" at once instead of waiting (waiting would never end for
    /// either). So the switch is tried again, briefly, until the busy wait has passed.
    fn wal(connection: &Connection) -> rusqlite::Result<()> {
        let start = std::time::Instant::now();
        loop {
            match connection.pragma_update(None, "journal_mode", "WAL") {
                Err(rusqlite::Error::SqliteFailure(why, _))
                    if why.code == rusqlite::ErrorCode::DatabaseBusy && start.elapsed() < BUSY =>
                {
                    #[expect(
                        clippy::disallowed_methods,
                        reason = "a backoff: SQLite gives no event to wait on"
                    )]
                    std::thread::sleep(Duration::from_millis(5));
                }
                done => return done,
            }
        }
    }

    /// The file at `path` made for its owner only, as is the directory when this makes it: what
    /// the database holds (terminal screens, boards, pages visited) is the person's. SQLite gives
    /// the WAL files the mode of the database. A directory that was there is left as it is.
    fn private(path: &Path) -> std::io::Result<()> {
        use std::os::unix::fs::{DirBuilderExt as _, OpenOptionsExt as _, PermissionsExt as _};
        if let Some(dir) = path.parent().filter(|dir| !dir.as_os_str().is_empty()) {
            if let Some(above) = dir.parent() {
                std::fs::create_dir_all(above)?;
            }
            std::fs::DirBuilder::new()
                .mode(0o700)
                .create(dir)
                .or_else(
                    |why| match why.kind() == std::io::ErrorKind::AlreadyExists {
                        true => Ok(()),
                        false => Err(why),
                    },
                )?;
        }
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .mode(0o600)
            .open(path)?
            .set_permissions(std::fs::Permissions::from_mode(0o600))
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

    /// A database in memory in place of the one that could not be opened for `why`: it works, and
    /// says it keeps nothing (`kept_in_memory`).
    ///
    /// # Errors
    /// SQLite gave no database in memory.
    pub fn memory_instead(why: &str) -> Result<Self, String> {
        Ok(Self {
            instead: Some(why.to_string()),
            ..Self::memory()?
        })
    }

    /// Why this store is only in memory, when it stands in for a database that could not be
    /// opened.
    pub fn kept_in_memory(&self) -> Option<&str> {
        self.instead.as_deref()
    }

    fn ready(mut connection: Connection) -> Result<Self, String> {
        let text = |why: rusqlite::Error| why.to_string();
        connection.busy_timeout(BUSY).map_err(text)?;
        connection
            .pragma_update(None, "foreign_keys", "ON")
            .map_err(text)?;
        connection.set_prepared_statement_cache_capacity(STATEMENTS);
        Self::migrate(&mut connection)?;
        let store = Self {
            connection: Mutex::new(connection),
            instead: None,
        };
        store.optimize();
        Ok(store)
    }

    /// The schema's steps not done yet, each in a transaction of its own that takes the write lock
    /// at its start and reads where the schema stands inside it: another process opening the same
    /// database at the same moment (the CLI, a second app) waits for it, then finds the step done.
    fn migrate(connection: &mut Connection) -> Result<(), String> {
        let text = |why: rusqlite::Error| why.to_string();
        loop {
            let tx = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(text)?;
            let done: usize = tx
                .pragma_query_value(None, "user_version", |row| row.get(0))
                .map_err(text)?;
            if done > STEPS.len() {
                return Err(format!(
                    "its schema is at step {done}, past the {} this build knows: a newer llx-desk \
                     wrote it: update llx-desk, \
                     or move the file aside",
                    STEPS.len()
                ));
            }
            let Some(step) = STEPS.get(done) else {
                return Ok(());
            };
            tx.execute_batch(step).map_err(text)?;
            tx.pragma_update(None, "user_version", done + 1)
                .map_err(text)?;
            tx.commit().map_err(text)?;
        }
    }

    /// SQLite's own upkeep of what it knows of the tables, for its query plans: cheap, and done
    /// as the store opens and as the app quits.
    pub fn optimize(&self) {
        let optimized = self.connection().execute_batch("PRAGMA optimize");
        if let Err(why) = optimized {
            log::warn!("store: the database was not optimized: {why}");
        }
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
        // Immediate: the write lock is taken at the start, so the busy timeout applies; a deferred
        // one that read first fails at once when another connection (the CLI's) committed between.
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|why| why.to_string())?;
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
