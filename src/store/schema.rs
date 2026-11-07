//! Shipped SQLite migrations, append-only and applied in order by Store.

/// The schema, step by step. Each step runs once, in a transaction, in this order.
pub(super) const STEPS: &[&str] = &[
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
    // window there was; a split kept in `state` becomes its shown tab's tree. (The columns of
    // `windows` from `x` to `side_width` are unused: a window's frame and side are the browser's,
    // kept with the person's settings. Steps are never edited, so they stay.)
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
