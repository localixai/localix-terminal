# Projects: the scope of everything

A project is a folder. Everything the window holds belongs to exactly one: its tabs (pages,
terminals, the board), what an agent works in, the history, the Event Center's events, the
address field's suggestions. The window shows one project at a time; the others go on working
out of sight. This replaces the owner string (a path or a name) and its basename "group".

## Identity

- A project is identified by its folder: an absolute, canonical path (links resolved). Its name is
  the folder's name; two projects of the same name are told apart by their parent
  (`app — work`, `app — side`).
- Which project a path is in: the nearest folder above it with `.git`, else the path itself
  (`projects::root_of`, one function, in the core). Every client resolves it the same way.
- **Home** is the project of the home folder (`~`). It always exists and cannot be removed: what
  is opened without a project (the app opened from the Dock, a shell in `~`) is Home's.
- Projects are rows (below) and outlive the app: the switcher's recent list, each project's tabs to
  restore, its history.

## Hierarchy

```text
project ─┬─ pane (web | terminal | board)  open, or closed (the history); shown in a tab of a
         │    │                             window's workspace of the project (docs/windows.md)
         │    ├─ visits (web: each page it showed)
         │    └─ screen (terminal: its last screen, restored above a new prompt)
         ├─ board                           one per project
         ├─ events                          the Event Center's journal
         └─ agents                          who works in its tabs (live, in memory)
```

- An **agent** is who calls: an agent's session, or a worker given a name (`--as cart-button`).
  It is a name within a project, not a scope of its own: its tabs are the project's, its notes
  (what the person sends) are its own, `--end` closes its tabs. With no name, it is the project's
  main agent.
- **Settings** belong to no project: one window-level tab, shown in whichever project is shown.

## Store (SQLite)

A new schema; the old tables (`boards` by owner string) are not carried over.

```sql
projects (id INTEGER PRIMARY KEY, path TEXT NOT NULL UNIQUE, name TEXT NOT NULL,
          created INTEGER NOT NULL, opened INTEGER NOT NULL)
panes    (id INTEGER PRIMARY KEY, project INTEGER NOT NULL REFERENCES projects ON DELETE CASCADE,
          kind TEXT NOT NULL,          -- web | terminal | board
          agent TEXT,                  -- who opened it; NULL: the person
          title TEXT NOT NULL, url TEXT, cwd TEXT, command TEXT,
          place INTEGER NOT NULL,      -- its order in the project's list
          opened INTEGER NOT NULL, shown INTEGER, closed INTEGER)   -- closed NULL: open
visits   (id INTEGER PRIMARY KEY, project INTEGER NOT NULL REFERENCES projects ON DELETE CASCADE,
          pane INTEGER REFERENCES panes ON DELETE SET NULL, url TEXT NOT NULL, title TEXT NOT NULL,
          at INTEGER NOT NULL)
screens  (pane INTEGER PRIMARY KEY REFERENCES panes ON DELETE CASCADE, text TEXT NOT NULL,
          at INTEGER NOT NULL)          -- a terminal's last lines, as text
boards   (project INTEGER PRIMARY KEY REFERENCES projects ON DELETE CASCADE, board TEXT NOT NULL,
          updated INTEGER NOT NULL)
center_events (... project INTEGER REFERENCES projects ON DELETE CASCADE ...)
answers  (offer TEXT PRIMARY KEY, taken INTEGER, later INTEGER, until INTEGER)  -- offers.rs
state    (key TEXT PRIMARY KEY, value TEXT NOT NULL)   -- a few named values of the desk's
change_events, change_cursors                          -- the feed, as now
```

`tabs` is the layout's table, not the ledger's: a workspace's tabs, each holding a tree of panes
(`pane_nodes`; docs/windows.md). The ledger's rows are `panes`.

- A pane's row is written as it opens, changes (title, page, place) and closes; a closed pane stays
  as history, pruned after a while (a project keeps its last 200).
- A pane's key in the window and the protocol (`w3`, `t2`, `b1`) stays a handle of the run; the row
  id is the pane's identity across runs. A restored pane gets a new key.
- `settings.json` stays a file (the person may edit it); `offers.json` moves into `answers`.

## Protocol

- Every call names its project by path: `project` (absolute) replaces `owner`, with `agent`
  (optional) and `cwd`. The server canonicalizes it and finds or makes its row.
- The CLI, `llx-desk mcp` and the plugin resolve the project the same way: `--project DIR`, else
  `LLX_DESK_PROJECT`, else `root_of(cwd)` (`client::Caller`). The plugin passes its session's
  folder with every call it runs (`--project`), wherever its hooks run.
- `llx-desk open [path]`: the app, showing the project of `path` (as `code .` opens a folder).
- A note (`comment/added`) carries the project's path and the agent's name.
- The protocol's version moved on: a client of the owner string is told it speaks another.

## The window

- **Switcher** in the bar, left of the address: the project's name and a chevron. Its menu: recent
  projects (a dot where an agent works or something waits for the person), Open Folder… (⌘O),
  Close Project. Switch with ⌃⌘P; the row of project pills under the bar goes.
- **Scope**: the side and the tab row list the shown project's tabs; ⌘1…⌘9, Search Tabs, the
  history menu, the address field's suggestions and New Terminal/Agent ("+", in the project's
  folder) are the project's. The Event Center shows every project's events, each named by its
  project.
- **An agent in another project** never moves the window: its project's dot lights in the
  switcher, its events wait in the Event Center.
- **Close Project** (⌃⌘W) closes its tabs as the person's close does (a terminal's program runs on
  out of sight, in its history as any closed terminal); its history, board and events stay. The
  window goes to the project used last with tabs open, else Home.

## Restore

As a terminal that restores its sessions: the windows, tabs and recent output come back; programs
do not.

- On launch the project used last (the one the window showed last) opens with the tabs it had
  open; the tabs other windows had gather into the first window, each in its project's workspace
  there (windows are not opened again across a restart); another project's come back the first
  time the person goes to it (the switcher,
  `llx-desk open DIR`).
- A page reopens at its address.
- A terminal opens a new shell in its last folder, with its last screen shown above the new
  prompt, dimmed. The program that ran is not run again (an agent's tab says how to resume it).
- The board is as kept.

## Layers

- `projects.rs` (core): `root_of`, the `Project` row, find-or-make, opened, recent, labels.
- `ledger.rs` (core): a project's panes, visits and screens in the store; `desk/ledger.rs` keeps it
  as panes open, change and close, and tells the window each project's past.
- `desk/`: every call resolved to a project first; the registry keyed by project and agent; the
  feed's entities carry the project id or the owner (`Pane`, `Terminal`, `Board`, `Said`, `Ask`).
- `browser/`: the window knows the shown project (`scope.rs`) and each tab's project; the strip and
  the side draw the shown one's tabs; history and suggestions are told by the desk (the store), not
  kept in the window; the switcher is `host_project.rs`.
- The plugin and MCP: no owner logic of their own; they pass the folder.
