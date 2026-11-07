# llx-desk architecture

How the desk is built, and the rules every change keeps: read it before adding a kind of tab, a
kind of thing that changes, something that reacts to a change, or a table. `ARCHITECTURE.md` maps
the modules; this says why they fit as they do.

The design is not new. Each part is a published pattern, named where it is used: event
notification over a transactional outbox, projectors and reactors that each keep a cursor,
idempotent consumers that reconcile from the owner, and clients that are poked and then pull.

## Terms

| Term        | Meaning                                                                                  |
| ----------- | ---------------------------------------------------------------------------------------- |
| Owner       | The one part that writes a kind of thing (a board, the layout, a pane, a terminal) and announces its changes. |
| Kept owner  | An owner whose state is in the store and outlives the app: the projects, their ledgers, the layout (windows, workspaces, tabs, their panes), the boards, the Event Center's journal. |
| Live owner  | An owner whose state is in memory and goes with the app: panes' keys, terminals, notes on their way, asks. |
| Notice      | A past-tense fact `{entity, op}` about a change: what changed, never what it now holds.   |
| Entity      | The kinds of thing that change, one enum (`feed::Entity`) every consumer matches whole.   |
| Change feed | The ordered log of notices in the store (`change_events`), one cursor a consumer (`change_cursors`). |
| Consumer    | A named reader of the feed with its own cursor and thread: a projector or a reactor.      |
| Projector   | A consumer that keeps derived data: the Event Center, a board's file. It can be deleted and rebuilt. |
| Reactor     | A consumer that does something about a change: the relay that tells subscribers.          |
| Source      | A part with events for the person: a type in `desk/center/sources.rs`.                    |
| Tool surface| The one set of calls every agent uses, the `action`s of the `window` tool: `open`, `interact`, `screenshot`, `close`, `show`, `list`. |

## Principles

| #  | Principle                         | In one line                                                                         |
| -- | --------------------------------- | ----------------------------------------------------------------------------------- |
| P1 | One owner a kind                  | A kind of thing has one owner; every other part reads it or hears its notices.       |
| P2 | Owners announce, consumers react  | A change records its notice with it; what follows lives in a consumer, never in the owner. |
| P3 | The notice with the change        | A kept owner writes its notice in the same transaction as its rows; a live owner under the same lock. |
| P4 | Notices carry refs, not state     | A consumer reads what it needs from the owner when it handles a notice.             |
| P5 | Repeats are harmless              | A consumer gives the same result for a notice twice, late, or after a newer one.    |
| P6 | Derived data is rebuildable       | A projector's output can be deleted and is made again from the owners (`reconcile`). |
| P7 | One mechanism, many kinds         | A new kind is a variant of an enum the compiler holds every part to, not a new path or tool. |
| P8 | One tool surface                  | A new kind of tab is a `kind` of `open` and an `op` of `interact`, never a tool of its own. |
| P9 | Poke, then pull                   | The window and the plugin are told that something changed and read it; no frame is trusted for state. |
| P10| The window knows no domain        | A page draws what it is sent (a section, a tone, a button's role); it holds no table of kinds. |

## The change feed

```text
 owner                          one transaction (kept) or one lock (live)        consumers, each at its cursor
 ┌──────────────┐  its rows    ┌────────────────────────────────────┐   read    ┌───────────────────────────┐
 │ board        │ ───────────▶ │ boards                             │  ◀──────  │ center   (projector)      │
 │ layout, pane │  its notice  │ change_events {seq, entity, op,    │  pages    │ board-file (projector)    │
 │ terminal, ask│ ───────────▶ │                run}                │  of 100   │ relay    (reactor)        │
 └──────────────┘              │ change_cursors {name, seq}         │  ──────▶  └───────────────────────────┘
                               └────────────────────────────────────┘  cursor moved after each notice
```

- **Producing.** A kept owner calls `feed.write(|write| { …its rows…; write.record(&entity, op) })`:
  the rows and the notice are committed together or not at all, so a notice exists exactly when its
  change does. A live owner calls `feed.record(&entity, op)` under the lock of its change, so notices
  are in the order of the changes.
- **Consuming.** `feed.consume(consumer)` starts it on a thread of its own: it reconciles, then reads
  pages past its cursor and moves the cursor after each notice. A handler that fails is tried again
  with a doubling pause; after five attempts the notice is set aside and the consumer reconciles. A
  consumer behind what the feed keeps jumps to the oldest kept notice and reconciles.
- **Restarts.** The cursors are in the store, so each consumer goes on from where it was. Every
  notice carries the run of the app that wrote it, and each entity says whether it outlives a
  run (`Entity::outlives_the_run`): a notice of a pane or a terminal of an earlier run names a key
  given again since, and is passed over. A notice of a board or of the layout (a window, a
  workspace, a tab) is read.
- **Retention.** Notices every running consumer read a day ago go, and past 10 000 the oldest go
  anyway; a cursor nobody moved in a month goes. The feed is not an archive: the owners' tables are
  the truth, and the Event Center's journal is the history.
- **What it is not.** Not event sourcing (state is in tables, not rebuilt from the log); notices
  carry no copies of state (they drift from their owner); no chains of business steps through
  events (a flow that needs an order of steps is a call).

## The store

One SQLite database (`store.rs`), `~/.local/share/llx-desk/llx-desk.sqlite` (`LLX_DESK_DATA` moves it; a
test build keeps it in its own directory; checks use one in memory). One connection, one writer:
every write is a transaction under the store's lock. The schema is the list `STEPS` in
`src/store/schema.rs`, applied in
order and counted in `user_version`: a new table is a step at the end, never an edit of a step
already shipped. A database a newer build wrote is left as it is, and the desk runs on one in
memory, keeping nothing.

| Table            | Owner                | What it holds                                                     |
| ---------------- | -------------------- | ----------------------------------------------------------------- |
| `change_events`  | the feed             | Notices, with their run.                                          |
| `change_cursors` | the feed             | Each consumer's place.                                            |
| `projects`       | the desk (`projects.rs`) | Every project: its folder (canonical, the key), its name, when it was made and last opened. Everything below is its, and goes with it. |
| `panes`          | the ledger           | A project's panes: kind, agent, title, address or folder and program, opened, shown, closed. Closed: the history; open when the app quit: what opens again, the same row. |
| `visits`         | the ledger           | The pages a project's panes showed: what the address field suggests. |
| `screens`        | the ledger           | A terminal's last screen, shown dimmed above the shell that takes its place. |
| `windows`        | the layout           | Each window and the workspace it shows. Legacy geometry columns remain for migration compatibility; the browser keeps frame and side settings. |
| `workspaces`     | the layout           | A window × a project: the tab it shows. |
| `tabs`           | the layout           | A workspace's tabs, in their places; closed, they go. |
| `pane_nodes`     | the layout           | Each tab's tree: branches (side by side or one above the other) and leaves (a pane, in focus or not), each with its share. |
| `boards`         | the boards           | A project's board, whole, written out to `.localix/board.json` by `board-file`; its old file is taken in once. |
| `center_events`  | the Event Center     | Every event it showed, of its project, open or settled, with what the person did: seen, dismissed. |
| `answers`        | the offers           | What the person answered to each offer: taken, put off how often, until when. |
| `state`          | the desk             | A few named values of its own: never a copy of an owner's state. |

## The Event Center

A projector (`desk/center.rs`): on every notice it gathers what waits for the person from each
source, writes the open events to its journal, settles the rest, and shows what the person has not
dismissed. An event says how it is shown (`Kind::look`: its section, the section's place, its tone,
whether the bell counts it), and each button says what it is for (`Role`: the row's own click, its
one action, its dismissal), so the window draws it as it comes. Its buttons take the paths the
person's own clicks take (the board's Accept, a tab's x): an event is never a second way to do
anything. Only what wants the person rings the bell; what ended well is listed quietly, and one
line an ask, a tab or a session, replaced rather than added to, keeps it from becoming noise.

## Adding things

**A kind of thing that changes.** A variant of `feed::Entity`, with its ref (ids, never content),
and its line in `Entity::outlives_the_run`. The build then names every consumer that must say what
it does with it. Its owner records its notices (P3).

**A kept owner.** A step appended to `src/store/schema.rs` for its table, its rows written in `feed.write` with its
notice, and a reader for consumers. Its state is the truth; nothing else writes its table.

**A source of events for the person.** A type in `desk/center/sources.rs` implementing `Source`
(it reads only its own owner and takes its own locks), its place in `SOURCES`, and, for a new kind
of event, a variant of `center::Kind` with its line in `Kind::look`. The window needs nothing.

**A consumer.** A type implementing `feed::Consumer` with a name of its own: `handle` re-reads the
owner and is harmless twice; `reconcile` makes its output again from the owners. It is started where
the desk is put together (`Desk::new`).

**A kind of tab.** A `kind` of `open`, an arm of the actions on a tab, its fields refused on other
kinds, and its fields described once in the `window` tool's schema (`args.rs`). No command of its
own (P8).

## Rules

- A notice is recorded by its owner only, with its change (P3); a consumer never records the notice
  of another owner's change.
- A consumer holds no owner's lock while it waits for another, and no lock across a send.
- A projector's output is never read as truth by an owner.
- Every enum of kinds is matched whole: the crate denies a `_` arm on its own enums.
- A page receives data, not a vocabulary: no table of kinds or labels in HTML or JavaScript.
- Plugins and pages read what the desk publishes (its answers, its files written out); they do not
  parse its internals.
