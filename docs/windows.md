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
terminal (or several, split in the tab), and on their right the documents (files and pages)
opened in them, one stack for the tab. Each session keeps its own, and another session never
shows them.

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
| split              | the tab's tree: a pane on a side of another     | `Tab`, `Workspace` upserted     |
| split new          | a new pane's row and its leaf beside another    | `Tab`, `Workspace` upserted     |
| pane out           | the pane out of the tree, a tab of its own      | `Tab`, `Workspace` upserted     |
| resize             | a branch's shares, at one divider               | `Tab` upserted                  |
| equalize           | every branch's shares, by what each child holds | `Tab` upserted                  |
| focus              | the tab's focused leaf, a stack's pane in sight | `Tab` upserted                  |
| zoom               | the tab's `whole` and focused leaf; shown tab   | `Tab` (`Workspace`) upserted    |

A tab's **zoom** is its `whole` (`desk/layout_zoom.rs`): the pane in focus drawn alone over the
tab's part, the rest of its tree kept as it is, out of sight and running, as a tab out of sight
is. Focus moved to another pane of the tab takes the zoom to it. A split, a split new, a pane
closed or taken out, a resize and an equalize write the tab whole again in the same
transaction: no operation leaves a tab zoomed on a tree it changed.

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
row shows a tab, a drag between rows moves it, ⇧⌘\ or a tab dragged onto the pages splits
(`Heard::Split`), ⌘D asks for a new pane (`Heard::SplitNew`), Move Pane to New Tab takes one out
(`Heard::Whole`), a drag of a divider or a key resizes (`Heard::Resized`), ⌃⌘= makes the tab
even (`Heard::Equalized`), a click in a pane of a split tab or a key focuses it
(`Heard::Focused`), ⇧⌘↩ zooms the pane in use or draws its tab whole again (`Heard::Zoomed`).
The operation's notice brings the new layout back. While a drag is under way the window moves its
own views (the divider under the pointer, the dragged row, a window following the pointer); only the
drop is an operation.

### A tab's tree in the window

The window draws the tree of the tab its shown pane is in, the layout's own tree with its panes
named by their keys (`Arranging::tree`, `Drawn`; a pane with no key this run is left out, its
share to the rest of its branch). It lays it out by the layout's geometry (`layout/geometry.rs`,
the one the desk asks too): each pane its rectangle, a point of glass between two, each pane at
least 190 points where there is room and a tenth of its branch at the least; a branch's
children share its room by their shares, side by side (`across`) or one above the other, and a
pane beside a split of the same way is one more child of that branch, not a branch of its own.
Every pane of the tree is placed where its rectangle is; the browser's toolbar lies over the
page in use and a session's documents' row over its stack, wherever they stand.

Between two children of a branch is a divider: a view of the strip's page each (a pool, one
made at once and the rest as a tree needs them, on the window's loop, never in a drag's frame),
told which way it moves. What the person does to the tree is drawn at once and waits in the
window only until the layout has heard it (`browser/split.rs`):

- a split made or undone (`Own`): the tree they made, kept while the layout still draws the tab
  as it did when they made it, and let go once it draws it so or otherwise;
- a divider held or let go of (`Held`): its share, kept until the layout keeps it, so an older
  snapshot cannot make it jump back; each divider let go of is held on its own;
- the pane they put the keys in: told once, until the layout keeps it as the tab's focus.

The sidebar edge and each divider own one primary pointer while held. Updates are coalesced
to one per animation frame; release uses its own final coordinates. Escape, pointer cancellation,
lost capture or a lost window focus cancels cleanly and restores the starting size. Focus a
separator and use the arrows along it to resize (Left/Right, or Up/Down for a divider between
panes one above the other; Shift moves farther); Home restores the default width, or makes the
divider's branch even by what its children hold.

Only the final divider size is written, on the desk's writer queue: `Heard::Resized { path, at,
share }`, the branch at `path` giving its children before `at` that share. The desk resizes a
tree only when every pane of it has a key, so a path the window read names the same branch in
the store. It is restored when the project or app opens again.

### A new pane

Split Right (⌘D) and Split Down (⇧⌘D) ask the desk for a terminal on that side of the pane in
use, in its tab (`Heard::SplitNew { beside, side, asked }`; "+" → Split Right with… and Split
Down with… ask for an agent's program in it). The desk opens it in the folder of the terminal
beside: where its shell is now (`foreground::cwd` of the program in front), else the folder it
opened in; beside a page or a file, the project's. The terminal opens out of sight and is placed
in the tab in the same change as its ledger row (`desk/layout_split.rs` `split_in`), the pane in
focus; then the window shows it and gives it the keys, as the person's click would. The window
asks only for a split that fits: the pane in use is at least two panes' least and the line
between them (`geometry::fits`). Where it is not, Split Right and Split Down are greyed in the
menu bar and the "+" menu, and the key does nothing: no error, since it is no failure.

A terminal's folder is kept in the ledger as it closes and as the app quits (its shell's folder
then), so a restore opens it where it was.

### How many terminals

The app holds as many terminals at once as the machine has room for (`terminal/room.rs`), every
project's and every window's counted, with those whose tab the person closed while their program
runs on out of sight. What one costs was measured: its page, where a window shows it, is a WebKit
process of its own, about 47 MiB drawn; in the app it holds three descriptors (its PTY's end and
a copy for its reader and its writer), two threads and about 1.6 MiB. So a quarter of the
machine's memory and half the open-file limit each bound the count, the tighter holds, and never
under 16 (on a Mac with 24 GB and the raised limit of 10,240: 128).

- **Room is made only of a program that ended.** A new terminal over the bound takes the place of
  the oldest of its project whose program ended (its tab goes, as one closed). A program that
  runs is never ended for another, whatever became of its tab, and another project's terminals
  are never let go of for this one's.
- **Otherwise it is not opened** (`desk/term/room.rs`): nothing is touched, an agent's `open`
  says "not opened" with how many are open and what frees one, and the Event Center has one
  quiet line in its project, titled by the terminal's command ("Terminals Not Opened", not
  urgent, with Dismiss), kept for the last refusal there until a terminal opens in it again
  (`center/refused.rs`, told through the feed as `Room`).

### An agent goes on

A terminal opened again after a quit or a crash is the person's shell in its folder, its last
screen above, dimmed; its program is not run again, but the coding agent it ran goes on in its
session there:

- **Kept as it is seen.** Each time another program comes in front of a terminal, its command
  line is read through its agent's list (`browser/agents_resume.rs`, the options each agent's
  `--help` names: `agents_policies.rs`) and kept for the pane at once (`agent_sessions`, by
  `desk/agent_resume.rs`), written then and not left to the writer thread, so a crash keeps it
  as a quit does. The session's id comes from
  that line (`--resume`, `--session-id`, `codex resume <id>`, `opencode -s`), from the agent's
  own hooks (the Claude Code plugin calls `interact` with `session`, from its terminal, as the
  session starts and when it changes), or from "+", which starts Claude Code and Copilot with
  `--session-id` of the app's own. A run that prints and ends, a command of the agent's own
  (`mcp`, `exec`, after options too: `codex -m o3 exec`) and a board's worker are not kept; an
  agent that ends of itself, or a terminal the person closes, is let go of. The app quitting
  ends its agents and keeps them. Another agent or session in the terminal counts its tries
  afresh.
- **What is carried.** Only the options its list allows, each value held to its form (a name,
  a choice, a folder by its absolute path, read by its form alone: the disk is never asked,
  and nothing that does not print, a control or a mark that turns text around, is carried or
  typed), never a prompt, a value that looks like a key or names a
  saved secret, an inline setting, or a sandbox bypass. The permission skip the person chose
  is carried, and said. The command is built again from the agents' table and those words,
  read through the list again: a row of the store never becomes a command.
- **Typed when the shell is ready.** The terminal's line above the shell says "Resumed Claude
  Code session" (and "· permissions skipped", "· sandbox bypass not carried"); once the shell
  says its prompt is ready (OSC 133 A), the command is typed into it and runs, as the person
  would type it (`desk/agent_relaunch.rs`). It is typed only into the shell itself (the
  terminal's foreground group is the shell's), with no command at work, and at a prompt
  nothing was typed at since the shell said it; else nothing is typed and the line waits with
  Continue. A shell that says nothing is typed nothing unasked. The person's Retry or Continue
  is taken once, however many clicks, and types once the shell is in front with nothing at
  work, without waiting for a mark (fish, `exec tmux`).
- **When it does not.** A command that ends with an error within ten seconds is typed once
  more, then its terminal has a quiet line in the Event Center, "Did not resume", with Retry;
  the next launch types nothing for it. One that ends well at once ended: it is let go of, not
  a failed try. An agent the person starts in front of that terminal takes the line's place. An agent with no session known, or not installed,
  has "Was open here" with Continue, which types its folder's last session's command.
- **Never twice, never off.** A session live in one terminal is not resumed in another, and
  nothing is resumed while the person turned Resume agents off (Settings, Terminal); what is
  skipped so is let go of from the store.

### The keyboard on panes

The Pane menu (a group of the shortcut table: the menu bar's and a section of Settings) acts on
the tab shown, from its pane in use (`browser/deck_keys.rs`, `host_panes.rs`):

- ⌥⌘ and an arrow: the keys to the pane that way, by the geometry (`geometry::neighbour`): of the
  panes whose edge meets the one in use on that side, the one used last, then the one alongside
  most. Nothing wraps round the tab's edge. ⌥⌘] and ⌥⌘[: the next pane in the order they lie, and
  the one before, round. The panes used are the window's, latest first, never kept.
- ⌃⌘ and an arrow: the divider on that side of the pane in use moves a step that way (5 % of its
  branch, ⇧ 20 %); at the tab's edge on that side, the divider on its other side does, so the
  arrow's way is the way a divider goes. It is drawn at once and told as a divider let go of is.
- ⌃⌘=: every branch even by what its children hold (`Tree::equalize`), drawn at once.
- ⇧⌘↩ (Zoom Pane): the pane in use alone over the tab's part, at once, and back; greyed over a
  pane alone in its tab. The side lists a zoomed tab as one row, by the pane drawn, and the row
  and the bar's crumb carry how many panes are put by. ⌥⌘ and an arrow, ⌃⌘ and an arrow and ⌃⌘=
  draw the tab whole first and then act on it; ⌥⌘] and ⌥⌘[ go round all the tab's panes and take
  the zoom along; ⌘D splits it whole; ⌘W gives the keys to the pane its rule picks among all.
- ⌘W: in a tab of several panes, the pane in use closes; the one used last of the rest takes the
  keys, else its neighbour in its branch, which takes its share. The last pane closes the tab.
- Move Pane to New Tab (no key; a pane's menu too; ⇧⌘\ in a tab of several panes): the pane in
  use out of the tree, into a tab of its own right after it. A document goes nowhere, and a
  session of one terminal is made whole instead, its documents put by.

⌘D and ⌥⌘ with an arrow are the menu's over a file too: the editor's own bindings of them are
left out, so a pane's keys are the same whatever it shows.

⌃⌘ and an arrow is free on macOS 26: the system keeps ⌃ and an arrow for Spaces and Mission
Control, and Fn ⌃ and an arrow for its tiling, never ⌃⌘.

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
for each pane the agent owns: its key, its kind in parentheses, `[shown]` when it is shown, where
it is (its tab's place among its window's tabs of the project, as ⌘1…9 count them; the pane a
zoomed tab shows; the panes on each side of it, by the layout's geometry, or the document a stack
shows over it), then its title and detail (`t2 (terminal) [shown] in tab 1, t1 above it: Title
detail`); it says nothing of windows. `show` of a pane shows its tab in its window and focuses it;
with `zoom`, it zooms the tab on it (`true`) or draws it whole again.

`open` with `beside` (a pane of the caller's project) and `side` (`right`, else `down`) puts what
it opens in that pane's tab (`desk/layout_agent.rs`): a terminal on that side of it, opened out of
sight and split in as its ledger row is written (`split_in`); a page or a file asked from that
pane, so it joins the tab's documents. The person's focus and the tab the workspace shows stay as
they were. Refused, with nothing opened: a pane of another project or in no tab, a document under
a pane or beside a tab that holds no document, the board (a tab of its own), and a terminal that
would leave a child of a branch under a tenth of it. An agent gets no resize, no equalize and no
way between panes: those are the person's.

### A session and its documents

A call that comes from a terminal of the window is known by it: the server reads the process at
the other end of the connection (`wire/peer.rs`), and the desk finds the terminal whose program
that process descends from (`terminal/lineage.rs`). The agent sets nothing up and the call
carries nothing new; a call from another terminal app has no terminal here.

A file or a page is a **document**. It joins the session it was asked from, in the same change
as the pane's ledger row (`desk/beside.rs`): the terminal's tab holds its documents as a stack
(`layout/stack.rs`) on the terminal's right, one of them in sight, the terminal on the left. A
tab of several terminals has one stack for all of them, on the right of them all: a document
opened from any of its terminals joins it.

| who asks                                    | the session the document joins                   |
| ------------------------------------------- | ------------------------------------------------ |
| an agent, from a terminal of its project    | that terminal's                                  |
| the person (the tree, ⌘T, a link's new tab) | the one they look at                             |
| anyone, from no session                     | the one the project shows, else its last         |

- The side lists **sessions**, not their documents: a session is one row, by its terminal (of
  several, the one in focus, else its first). Its
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
- A session made whole (⇧⌘\) is zoomed on its terminal: it keeps its documents out of sight,
  and ⇧⌘↩, ⇧⌘\ again or the next split brings them back. A session takes more terminals (a new one split beside one of its, or a terminal alone
  in its tab dragged onto it), and its documents stay the tab's one stack; it takes no other
  pane: a board, or a terminal with documents of its own, dragged onto it stays a tab.
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
