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
      └─ tab                     one row of the side's list, in its place
         └─ pane tree            the tab's split: branches and leaves
            ├─ branch            side by side or one above the other; each child with its share
            └─ leaf → pane       one pane, in focus or not
               pane              web | terminal | board: its address or folder and program, its
                                 title, its agent; the key agents use (w1, t2, b1)
```

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

### Beside the terminal

A call that comes from a terminal of the window is known by it: the server reads the process at
the other end of the connection (`wire/peer.rs`), and the desk finds the terminal whose program
that process descends from (`terminal/lineage.rs`). The agent sets nothing up and the call
carries nothing new; a call from another terminal app has no terminal here and opens tabs.

What the project's own agent opens from a terminal lands in that terminal's tab, on its right,
in the same change as the pane's ledger row (`desk/beside.rs`):

| the terminal's tab                         | where the new pane lands                         |
| ------------------------------------------ | ------------------------------------------------ |
| the terminal alone                         | on its right; the terminal keeps the keys        |
| the terminal and a pane it opened before   | in that pane's place; that pane a tab right after |
| arranged by the person (another pane)      | a tab of its own                                 |
| made whole by the person (⇧⌘\)             | a tab of its own, until they split it again      |

`show` of a hidden pane follows the same table, so the window is not taken from the terminal.
The links are kept while both panes are open: which pane was opened from which terminal, and
the terminals the person made whole. A pane closing forgets its links, and a terminal closing
leaves what was beside it a tab. A worker the person ran from the board is known by its
terminal (`desk/workers.rs`) and opens beside it, as the project's agent does. Any other agent
with a name on the board (`--as`) opens tabs of its own, as does the person and what the app
opens again: several named agents of one terminal would take the one place in turn. Settings → Agents → What an agent opens turns the
whole of it to plain tabs.

The window follows: a tab opened aside does not take the window (it is drawn beside the tab in
sight), and a window still laid out as the layout kept it takes the layout's next split, where
one the person changed since is told to the layout instead.
