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
    // 2: windows, their workspaces (a window × a project), the tabs of each and each tab's tree
    // of panes (docs/windows.md). What `tabs` held is the panes: renamed, with the pages and the
    // screens that name one. Each pane open becomes a tab of its own in its place, in the one
    // window there was; a split kept in `state` becomes its shown tab's tree.
    "ALTER TABLE tabs RENAME TO panes;
    DROP INDEX tabs_of_project;
    CREATE INDEX panes_of_project ON panes (project, closed);
    ALTER TABLE visits RENAME COLUMN tab TO pane;
    ALTER TABLE screens RENAME COLUMN tab TO pane;
    CREATE TABLE windows (
        id INTEGER PRIMARY KEY,
        shown INTEGER REFERENCES workspaces ON DELETE SET NULL,
        x REAL,
        y REAL,
        width REAL,
        height REAL,
        side_open INTEGER NOT NULL DEFAULT 1,
        side_width REAL,
        opened INTEGER NOT NULL,
        closed INTEGER
    );
    CREATE TABLE workspaces (
        id INTEGER PRIMARY KEY,
        window INTEGER NOT NULL REFERENCES windows ON DELETE CASCADE,
        project INTEGER NOT NULL REFERENCES projects ON DELETE CASCADE,
        shown INTEGER REFERENCES tabs ON DELETE SET NULL,
        opened INTEGER NOT NULL,
        UNIQUE (window, project)
    );
    CREATE TABLE tabs (
        id INTEGER PRIMARY KEY,
        workspace INTEGER NOT NULL REFERENCES workspaces ON DELETE CASCADE,
        place INTEGER NOT NULL,
        opened INTEGER NOT NULL,
        closed INTEGER
    );
    CREATE INDEX tabs_of_workspace ON tabs (workspace, closed, place);
    CREATE TABLE pane_nodes (
        id INTEGER PRIMARY KEY,
        tab INTEGER NOT NULL REFERENCES tabs ON DELETE CASCADE,
        parent INTEGER REFERENCES pane_nodes ON DELETE CASCADE,
        place INTEGER NOT NULL,
        share REAL NOT NULL,
        across INTEGER,
        pane INTEGER REFERENCES panes ON DELETE CASCADE,
        focused INTEGER NOT NULL DEFAULT 0,
        CHECK ((across IS NULL) <> (pane IS NULL))
    );
    CREATE INDEX pane_nodes_of_tab ON pane_nodes (tab);
    CREATE INDEX pane_nodes_of_pane ON pane_nodes (pane);

    INSERT INTO windows (id, opened)
        SELECT 1, COALESCE(MIN(opened), 0) FROM panes WHERE closed IS NULL HAVING COUNT(*) > 0;
    INSERT INTO workspaces (window, project, opened)
        SELECT 1, project, MIN(opened) FROM panes WHERE closed IS NULL GROUP BY project;
    INSERT INTO tabs (id, workspace, place, opened)
        SELECT panes.id, workspaces.id, panes.place, panes.opened
        FROM panes JOIN workspaces ON workspaces.project = panes.project
        WHERE panes.closed IS NULL;
    INSERT INTO pane_nodes (tab, parent, place, share, pane, focused)
        SELECT id, NULL, 0, 1.0, id, 1 FROM tabs;

    CREATE TEMP TABLE kept_splits AS
        SELECT json_extract(value, '$.shown') AS shown, json_extract(value, '$.beside') AS beside,
               COALESCE(json_extract(value, '$.right'), 1) AS right
        FROM state
        WHERE key LIKE 'split %'
          AND json_extract(value, '$.shown') IN (SELECT id FROM tabs)
          AND json_extract(value, '$.beside') IN (SELECT id FROM tabs)
          AND json_extract(value, '$.shown') <> json_extract(value, '$.beside');
    INSERT INTO pane_nodes (tab, parent, place, share, across)
        SELECT shown, NULL, 0, 1.0, 1 FROM kept_splits;
    UPDATE pane_nodes SET
        parent = (SELECT branch.id FROM pane_nodes AS branch
                  WHERE branch.tab = pane_nodes.tab AND branch.across IS NOT NULL),
        place = (SELECT CASE WHEN right THEN 0 ELSE 1 END FROM kept_splits
                 WHERE kept_splits.shown = pane_nodes.tab),
        share = 0.5
        WHERE pane IS NOT NULL AND tab IN (SELECT shown FROM kept_splits);
    UPDATE pane_nodes SET
        tab = (SELECT shown FROM kept_splits WHERE kept_splits.beside = pane_nodes.tab),
        parent = (SELECT branch.id FROM pane_nodes AS branch JOIN kept_splits
                  ON branch.tab = kept_splits.shown
                  WHERE kept_splits.beside = pane_nodes.tab AND branch.across IS NOT NULL),
        place = (SELECT CASE WHEN right THEN 1 ELSE 0 END FROM kept_splits
                 WHERE kept_splits.beside = pane_nodes.tab),
        share = 0.5,
        focused = 0
        WHERE pane IS NOT NULL AND tab IN (SELECT beside FROM kept_splits);
    DELETE FROM tabs WHERE id IN (SELECT beside FROM kept_splits);
    DROP TABLE kept_splits;
    DELETE FROM state WHERE key LIKE 'split %';

    UPDATE workspaces SET shown = (
        SELECT tabs.id FROM tabs JOIN pane_nodes ON pane_nodes.tab = tabs.id
        JOIN panes ON panes.id = pane_nodes.pane
        WHERE tabs.workspace = workspaces.id
        ORDER BY COALESCE(panes.shown, 0) DESC, tabs.place LIMIT 1);
    UPDATE windows SET shown = (
        SELECT workspaces.id FROM workspaces JOIN projects ON projects.id = workspaces.project
        WHERE workspaces.window = windows.id ORDER BY projects.opened DESC LIMIT 1);",
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
