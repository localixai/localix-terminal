# Windows, workspaces, tabs and panes

What the person sees is one hierarchy, held once and the same everywhere: in the desk's memory, in
the store, in what the window draws and in what agents are told. This is its model, its owner and
how each layer follows it. It keeps the rules of `docs/architecture.md`; read that first.

## Hierarchy

```text
app
├─ project                       a folder (docs/projects.md)
└─ window                        a native window: the workspace it shows
   └─ workspace                  the window × a project: the tabs of that project in that window
      └─ tab                     one row of the side's list, in its place: a session or the
         │                       board (and, in a project with no terminal, its documents)
         └─ pane tree            the tab's split: branches, stacks and leaves
            ├─ branch            side by side or one above the other; each child with its share
            ├─ stack → panes     a session's documents, one over the other, one of them in sight
            └─ leaf → pane       one pane, in focus or not
               pane              web | terminal | board | file: its address or folder and
                                 program, its title, its agent; the key agents use (w1, t2, b1, f1)
```

In short: project → session → its files and pages. A **session** is a terminal's tab: the
terminal, and on its right the documents (files and pages) opened in it. Each session keeps its
own, and another session never shows them.

- A **window** shows one workspace at a time: the project shown. Switching the project shows
  another workspace of the same window; the others keep their tabs out of sight.
- Two windows may show one project: each has a workspace of its own, with tabs of its own.
- A **tab** is what the side lists and what ⌘1…⌘9 count. It holds a tree of panes; a tab of one
  pane is a leaf alone. A split is a tab of several panes, never a pair of tabs.
- A **pane** is the content: a page, a terminal or the board. Its key (`w1`, `t2`, `b1`) is how
  agents name it, across every window. A pane is in exactly one leaf of one tab. Settings is the
  window's own tab, outside the layout: it is no pane, has no key and is not kept.
- **Focus** is a pane of the tab shown: the one the keys go to, the one a split's toolbar serves.

## Identity

| Entity    | Identity across runs          | Handle in a run        |
| --------- | ----------------------------- | ---------------------- |
| window    | its row id                    | the native window's id |
| workspace | its row id (window, project)  | the same               |
| tab       | its row id                    | the same               |
| pane      | its row id                    | its key (`w1`, `t2`)   |

A key's letter is the pane's kind: `w` a page, `t` a terminal, `b` the board, `f` a file in an
editor. A file has one pane in its project, found by its path: opened again, by the person or
an agent, it is the same pane shown. The app, not the tree, says how a picked file is shown:
text in an editor; a picture written as text (SVG), and what is no text an editor takes, as a
page, which shows it (a picture, a film, a sound, a PDF) or says in a line that nothing does.
Nothing is handed to another app unasked: that is the menu's Open in Default App.

A pane restored after a restart is the same row with a new key. Moving a tab to another window
moves the row (its `workspace` changes); its panes, keys, pages and programs are the same ones.
Nothing is copied.

## Owner

One kept owner, the **layout** (`desk/layout.rs`), writes windows, workspaces, tabs and pane trees.
The ledger stays the owner of panes (their kind, address, title, history). Nothing else writes
their tables; the window never keeps a copy of them as truth.

Every change is one operation of the layout, in one transaction with its notices:

| Operation          | Rows it writes                                  | Notices                         |
| ------------------ | ----------------------------------------------- | ------------------------------- |
| open window        | `windows`, its first `workspaces`               | `Window` upserted               |
| close window       | the window's workspaces and tabs closed         | `Window` deleted                |
| show project       | the window's shown workspace (made if new)      | `Window` upserted               |
| open tab           | `tabs`, its leaf in `pane_nodes`                | `Workspace` upserted            |
| show tab           | the workspace's shown tab                       | `Workspace` upserted            |
| move tab           | the tab's workspace and place; places around it | `Workspace` upserted, each side |
| close tab          | the tab closed; its panes' leaves gone          | `Workspace` upserted            |
| split              | the tab's tree: a branch over two leaves        | `Tab` upserted                  |
| unsplit            | the tree back to one leaf; the other pane a tab | `Tab`, `Workspace` upserted     |
| resize             | a branch's shares                               | `Tab` upserted                  |
| focus              | the tab's focused leaf                          | `Tab` upserted                  |

An operation that needs the answer (a new tab's id, where it went) is a call that returns it, as
the architecture says of commands; what follows it in other parts (the window drawing it, the
plugin told) is a consumer of the notice.

## Store

```sql
CREATE TABLE windows (
    id INTEGER PRIMARY KEY,
    shown INTEGER REFERENCES workspaces ON DELETE SET NULL,
    x REAL,
    y REAL,
    width REAL,
    height REAL,
    side_open INTEGER NOT NULL DEFAULT 1,  -- x .. side_width: unused; the frame and side are
    side_width REAL,                       -- the browser's, kept with the person's settings
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
CREATE TABLE pane_nodes (
    id INTEGER PRIMARY KEY,
    tab INTEGER NOT NULL REFERENCES tabs ON DELETE CASCADE,
    parent INTEGER REFERENCES pane_nodes ON DELETE CASCADE,
    place INTEGER NOT NULL,
    share REAL NOT NULL,                 -- of its parent; 1 for the root
    across INTEGER,                      -- a branch: 1 side by side, 0 one above the other
    pane INTEGER REFERENCES panes ON DELETE CASCADE,   -- a leaf: its pane
    focused INTEGER NOT NULL DEFAULT 0,
    CHECK ((across IS NULL) <> (pane IS NULL))
);
-- panes: the ledger's rows, what `tabs` held until now (kind, agent, title, url, cwd, command,
-- opened, shown, closed); `visits` and `screens` name a pane
```

- A node is a branch (`across` set, no pane) or a leaf (a pane, no `across`). A tab's root is its
  node with no parent.
- The schema moves by a step at the end of `STEPS`: `tabs` is renamed `panes`, the new tables are
  made, and every pane open becomes a tab of its own in the workspace of the one window and its
  project, in its old place (a closed one stays history, in no tab). A split kept in `state` (`split <project>`) becomes its shown
  tab's tree; the other tab goes. The `state` rows go with it.

## Feed

```rust
Entity::Window { id }        // kept: outlives the run
Entity::Workspace { id }     // kept
Entity::Tab { id }           // kept: its tree
Entity::Pane { owner, key }  // live: a pane of this run (what `Tab { key }` named until now)
```

Consumers read the owner for what changed:

- **the window** (a reactor): reads the layout of its window and draws it (side list, bar, the
  tree's views). It is poked, then pulls: no frame carries the layout.
- **the relay** (a reactor): tells the plugin and subscribers, as now.
- **the Event Center** (a projector): as now; it names panes by key.

## The window

The window draws a layout it is given and sends what the person did as an operation: a click on a
row shows a tab, a drag between rows moves it, ⇧⌘\ or a tab dragged onto the pages splits, a drag of the line resizes.
The operation's notice brings the new layout back. While a drag is under way the window moves its
own views (the line under the pointer, the dragged row, a window following the pointer); only the
drop is an operation.

The sidebar edge and split divider own one primary pointer while held. Updates are coalesced
to one per animation frame; release uses its own final coordinates. Escape, pointer cancellation,
lost capture or a lost window focus cancels cleanly and restores the starting size. Focus a
separator and use Left/Right to resize (Shift moves farther); Home restores the default width
or equal halves.

Only the final divider size is written, on the desk's writer queue. Its left share is kept in
the existing pane tree, then restored when the project or app opens again. A pending local size
is kept until the matching projection acknowledges it: an older snapshot cannot make the
divider jump back. Each half keeps at least a tenth of the space and, where there is room,
190 points.

Title-only sidebar updates patch existing nodes without measuring or animating their positions.
FLIP is reserved for a structural change. The board coalesces snapshots, ignores clock-only
redraws and defers replacement while a card is dragged or IME composition is active. The latest
snapshot is drawn once the gesture finishes, keeping drafts and focus.

A first window opens in the middle of the screen the system puts new windows on, at about
three quarters of it each way and at most 1200 × 760 points (`placement.rs`). Where
the person leaves a window, its size and how wide its side is are kept between launches
(`window.json` in the app's data); a kept place whose screen is gone is held to the screen that
is there.

## The window in use

One window is the one in use, as the system's key window is: the person's presses and the window
they bring forward make it so. What the person does acts on it; what opens with no window named
(an agent's page or terminal) opens in it; an agent's call on a pane acts in the window the pane is
in, and the window in use stays as it was. The window tells the desk which it is (its row's id), and
the desk places new panes there. Each window keeps its own project in sight; which project a tab
opening next is of, and the projects marked for news, are the app's.

Until windows are opened again across a restart, the desk gathers the tabs the other windows had
into the first as it starts, each in its project's workspace there, and closes those windows.

## Moving a tab to another window

A drag of a tab out of its window, past 10 points from the window's edge, detaches it (over the
window's pages it lays the window out in halves instead, as Split View does):

1. **Detach.** With more than one tab in the window, a window of the same size opens at once, not
   taking the keys, placed so that its first row is under the pointer; the tab's panes move into it
   as they are (no reload, no new program). The source keeps an empty slot where the tab was, and
   shows the tab beside it if the dragged one was shown. With one tab only, the window itself is
   what moves.
2. **Follow.** Each move of the pointer places the moving window so its first row stays under it.
   A window that moves itself tells its list the tab is out of it till it is let go.
3. **Over a list.** When the pointer is over the side's list of a window (its frame grown by 12
   points), that window comes to the front, the moving window turns transparent, and the list opens
   a slot where the tab would go. Over the source's own list the empty slot moves instead. Leaving
   the list, the moving window shows again.
4. **Drop.** Over a list: the tab moves into that window at the slot (one `move tab`), and the moving
   window closes. Elsewhere: the moving window stays where it is, as a window of its own (`open
   window` and `move tab`), and the source's slot goes.

Nothing is written before the drop: a detach shown and taken back leaves no change behind.

The panes move between windows as they are: a page's or a panel's view is put into the other
window's content (`Deck::take`, `Deck::take_in`), so a page is not loaded again and a terminal's
program goes on. The gesture is the windows' together, so the loop handles it before it picks a
window for an event (`host_tear.rs`); the window's part is where it is on the screen, where its
list is, and moving or clearing itself (`deck_tear.rs`). Let go, the layout is told once:
`Heard::Moved` with the window for a list, `Heard::Detached` for a window of its own, whose row the
app gives the window it was carried out into.

## Agents

The `window` tool keeps its calls. A pane's key names it in every window; `list` prints one line
for each pane the agent owns: its key, its kind in parentheses, `[shown]` when it is shown, then its
title and detail (`w1 (browser) [shown]: Title detail`); it says nothing of windows or tabs. `show`
of a pane shows its tab in its window and focuses it.

### A session and its documents

A call that comes from a terminal of the window is known by it: the server reads the process at
the other end of the connection (`wire/peer.rs`), and the desk finds the terminal whose program
that process descends from (`terminal/lineage.rs`). The agent sets nothing up and the call
carries nothing new; a call from another terminal app has no terminal here.

A file or a page is a **document**. It joins the session it was asked from, in the same change
as the pane's ledger row (`desk/beside.rs`): the terminal's tab holds its documents as a stack
(`layout/stack.rs`) on the terminal's right, one of them in sight, the terminal on the left.

| who asks                                    | the session the document joins                   |
| ------------------------------------------- | ------------------------------------------------ |
| an agent, from a terminal of its project    | that terminal's                                  |
| the person (the tree, ⌘T, a link's new tab) | the one they look at                             |
| anyone, from no session                     | the one the project shows, else its last         |

- The side lists **sessions**, not their documents: a session is one row, by its terminal. Its
  documents are a row of tabs over their pane (`browser/deck_docs.rs`): a press shows one, its
  close button closes it, and the row is there only while the session shown has documents.
  A tab of the row is as wide as its name, up to a tab of the side's, and never narrows: more
  than fit scroll sideways (a wheel turned up and down moves the row too), an edge with more
  past it fades, and the document in sight is brought into view when it changes.
- A document carried along the row is placed where it is let go (`place KEY BEFORE|end`,
  `Heard::Placed`): the session's stack keeps the order (`Tree::place`,
  `desk/layout_moves.rs`), the document in sight stays in sight, and the row is told its
  documents again. A document never leaves its row this way: the order of sessions is the
  side's.
- A document's menu (a right click on its tab in the row) is the tab's menu, placed where the
  person clicked: the row says its points are its own (`Cmd::DocsClicked`) and the window
  moves them to where the row lies, as it does the side's and the toolbar's. Its "other" tabs
  and those "to the right" are the documents of its row; a session's are the rows of the
  side, never its own documents (`host_tab_menu.rs` `beside`). A document offers no split:
  it is in its session's half already.
- **A document is never a row of the side, from the moment its tab exists.** Its pane is owned
  and placed in its session as the window makes the tab, before its page loads
  (`desk/open.rs`), and the window is told the layout before it is asked to show the tab
  (`bring`), so the document comes up in its half at once. For the moment between the tab and
  the layout that holds it, the window lists it under the session in sight
  (`browser/session.rs` `stands_for`).
- An agent's document is the one in sight unless the person is in a document of that session
  themselves: then it waits behind, a tab of the row. The terminal keeps the keys either way,
  and the window is not taken from what the person looks at. What the person opens is the pane
  in use, since they opened it to read.
- A document opened again is shown where it is, in its own session. `show` of a document
  follows the same rules.
- **A session closing closes its documents** (`desk/sessions.rs`): each as its own close would,
  so a file's unsaved text stays its draft. A document closing shows the next of its session,
  and the last one leaves the terminal whole.
- A session made whole (⇧⌘\) keeps its documents out of sight, and the next split brings them
  back. A session takes no other pane: a tab dragged onto it stays a tab.
- A document asked from no session (another terminal app's agent, the person with the board
  or nothing in sight, a call whose terminal has closed) joins a session all the same: the one
  its project shows, else its last (`desk/sessions.rs`). Only **a project with no terminal**
  keeps its documents in one tab of their own, a stack as a session's is, and its first
  terminal takes them as they were: a project with a session has no tab of documents that is
  not one.
- A terminal and the board are tabs of their own, as before: a terminal is a session, and the
  board is one per project. An agent that opens either from a terminal leaves the window to what
  it shows. A worker the person ran from the board is known by its terminal
  (`desk/workers.rs`), and its documents join that session.
- The tabs a run before left are brought to this shape as the app starts: a document split
  beside a terminal is that terminal's first document, on the side it stood, and one alone in
  a tab joins a session of its project (one tab of documents where it has no terminal).

A session is always a tab of the caller's own project in the window it works in. A pane of
another project is no session, a file tab holds only a file under its project's folder as it
really is, and a tab is listed, read, shown and closed by its own project's calls alone.

The window follows without waiting (`browser/session.rs`): it lays a tab out from the layout as
last told, so a session picked in the side, or a project switched to, is drawn at once with its
own document in sight, and the layout hears of it after. Nothing is opened, read or loaded
again by a switch: a session's panes stay as they were, out of sight.
