//! The catalogue kept on disk and searched: one SQLite file of its own, apart from the desk's
//! database, derived from what the sources say (`schema.rs`). A source's batch is written in
//! one transaction with the mark it leaves (`write.rs`), and what two sources both list shown
//! once (`shadow.rs`); a search is `search.rs`, what is held counted `held.rs`; reading the
//! sources that are due is `refresh.rs`.

mod held;
mod refresh;
mod schema;
mod search;
mod shadow;
mod write;

use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use super::model::{Listing, What};
use super::source::Batch;
use crate::store::{Connection, OptionalExtension, params};

pub use held::Held;
pub use refresh::{Progress, refresh};
pub use search::{Found, Query};

/// The catalogue's index. Shared between threads: each call holds its connection while it
/// runs.
pub struct Index {
    connection: Mutex<Connection>,
}

/// Where a source was last read to.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Kept {
    mark: String,
    read_at: u64,
    /// Its pass was cut short: it is read again at once.
    more: bool,
}

impl Index {
    /// The index in the file at `path`, made anew when it is of another version.
    pub fn open(path: &Path) -> Result<Self, String> {
        Ok(Self::of(schema::open(path)?))
    }

    /// An index kept in memory alone.
    pub fn memory() -> Result<Self, String> {
        let mut connection = Connection::open_in_memory().map_err(|why| why.to_string())?;
        schema::ready(&mut connection)?;
        Ok(Self::of(connection))
    }

    fn of(connection: Connection) -> Self {
        Self {
            connection: Mutex::new(connection),
        }
    }

    fn connection(&self) -> MutexGuard<'_, Connection> {
        crate::lock(&self.connection)
    }

    /// What `source`, ranked at `trust`, read in `batch` at `now`, in one transaction: its
    /// listings written, those gone removed (all it had but these, when the batch is whole),
    /// and its mark kept.
    pub fn put(&self, source: &str, trust: u8, batch: &Batch, now: u64) -> Result<(), String> {
        let mut connection = self.connection();
        let written = connection.transaction().map_err(|why| why.to_string())?;
        write::put(&written, source, trust, batch)?;
        Self::marked(&written, source, &batch.mark, now, batch.more)?;
        written.commit().map_err(|why| why.to_string())
    }

    /// Every listing of `source` and its mark removed, in one transaction: what it shadowed is
    /// shown again.
    pub fn forget(&self, source: &str) -> Result<(), String> {
        let mut connection = self.connection();
        let written = connection.transaction().map_err(|why| why.to_string())?;
        write::forget(&written, source)?;
        written.commit().map_err(|why| why.to_string())
    }

    /// That `source` was read at `now` and had nothing new: its mark stays.
    pub fn touch(&self, source: &str, now: u64) -> Result<(), String> {
        let connection = self.connection();
        let mark = Self::kept_by(&connection, source).map(|kept| kept.mark);
        Self::marked(&connection, source, &mark.unwrap_or_default(), now, false)
    }

    fn marked(
        connection: &Connection,
        source: &str,
        mark: &str,
        now: u64,
        more: bool,
    ) -> Result<(), String> {
        connection
            .execute(
                "INSERT INTO marks (source, mark, read_at, more) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT (source) DO UPDATE SET
                 mark = excluded.mark, read_at = excluded.read_at, more = excluded.more",
                params![source, mark, now, more],
            )
            .map(|_| ())
            .map_err(|why| why.to_string())
    }

    fn kept_by(connection: &Connection, source: &str) -> Option<Kept> {
        let read = connection.query_row(
            "SELECT mark, read_at, more FROM marks WHERE source = ?1",
            params![source],
            |row| {
                Ok(Kept {
                    mark: row.get(0)?,
                    read_at: row.get(1)?,
                    more: row.get(2)?,
                })
            },
        );
        read.optional().ok().flatten()
    }

    fn kept(&self, source: &str) -> Option<Kept> {
        Self::kept_by(&self.connection(), source)
    }

    /// The mark `source` left and when it was last read, in seconds of the epoch.
    pub fn mark(&self, source: &str) -> Option<(String, u64)> {
        self.kept(source).map(|kept| (kept.mark, kept.read_at))
    }

    /// What `query` finds.
    pub fn search(&self, query: &Query) -> Result<Found, String> {
        search::search(&self.connection(), query)
    }

    /// The listing of `id`.
    pub fn get(&self, id: &str) -> Option<Listing> {
        search::get(&self.connection(), id)
    }

    /// The sources of the other listings of what `id` is, the most trusted first.
    pub fn also_in(&self, id: &str) -> Vec<String> {
        shadow::also_in(&self.connection(), id)
    }

    /// How many listings that are shown each category has (of `what`, of `source`, when
    /// given), the biggest first and Other last.
    pub fn facets(&self, what: Option<What>, source: Option<&str>) -> Vec<(String, u64)> {
        held::facets(&self.connection(), what, source)
    }

    /// Every source the index holds, by its id.
    pub fn sources(&self) -> Vec<Held> {
        held::sources(&self.connection())
    }

    /// How many listings that are shown there are of each kind, every kind named.
    pub fn counts(&self) -> Vec<(What, u64)> {
        let connection = self.connection();
        let count = |what: What| {
            connection
                .query_row(
                    "SELECT count(*) FROM listings WHERE what = ?1 AND shadowed = 0",
                    params![what.name()],
                    |row| row.get::<_, u64>(0),
                )
                .unwrap_or(0)
        };
        What::ALL
            .into_iter()
            .map(|what| (what, count(what)))
            .collect()
    }
}

#[cfg(test)]
#[path = "index_tests.rs"]
mod tests;
