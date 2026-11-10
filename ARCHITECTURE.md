# Architecture

llx-desk is one crate: a library and the `llx-desk` binary, which is the app. The app is the server:
it owns the window, the terminals, the board and the database, and serves one API on a unix socket.
Everything else is a thin face of that API with nothing of its own: the CLI, `llx-desk mcp`, and the
Claude Code plugin (which runs the CLI). The desk knows no model and no agent. This file maps the
modules; `docs/architecture.md` is the architecture they follow (owners, the change feed, consumers,
the store) and how to add to it.

```text
llx-desk window '<json>'   llx-desk mcp   plugin hooks      the adapters (cli.rs, mcp.rs), via client.rs
        \                  |              /
         JSON-RPC 2.0 lines over a unix socket (wire.rs); server/api.rs is every method, once
        v                  v             v
LLX Desk.app                  the server (server.rs): lock, socket, one thread per connection;
        |                     started by the first call through LaunchServices (bundle.rs)
        v
desk/                         the window tool's actions over a project's tabs; the app's life
   |        |         |
   v        v         v
browser/  terminal/  board/          setup/  what the app puts in place: the command, the plugin, MCP
```

## Module boundaries

The root of each layer wires its parts together; it does not accumulate every handler.
Public paths stay stable through re-exports; implementation modules stay private.

| Boundary | Modules |
| --- | --- |
| Board data and rules | `board/model.rs` is the serializable vocabulary; `board/cards.rs` allocates cards; `board.rs` owns worker transitions. Neither depends on the desk or the UI. |
| Tool calls and events | `desk/call.rs` renders and masks answers; `tab.rs` validates tab keys and kind-specific fields; `actions.rs` dispatches; `heard.rs` adapts window events; `writes.rs` writes what they change on one writer thread, in order. |
| Application flows | `desk/board_ops.rs` applies worker operations; `restore.rs` reopens ledger rows; `layout_moves.rs` mutates tab placement; `layout_drawn.rs` projects the kept layout into browser keys. |
| Page reads and actions | `desk/web_read.rs` labels untrusted snapshots and slices UTF-16 text; `web_controls.rs` resolves and acts on refs; `web.rs` orchestrates a call. |
| Native UI | `browser/host_commands.rs` is the event-loop protocol; `host_events.rs` handles native events; `host_dispatch.rs` carries out commands; `host_keys.rs` and `host_newtab.rs` adapt user actions. |
| Deck and tabs | `deck_build.rs` constructs views; `deck_layout.rs` places them; `deck_drop.rs` previews drops; `deck_render.rs` sends state as data. `tabs_transfer.rs` moves existing views and `tabs_layout.rs` sizes them. |
| Platform and scripts | `browser/platform.rs` is the platform facade; `platform_views.rs` handles native view behavior; `webview_scripts.rs` waits for scripts across navigation. |
| Terminal and storage | `terminal/session.rs` owns lifecycle; `session/input.rs`, `output.rs` and `wait.rs` own input, output and waits. `store.rs` manages connections; `store/schema.rs` contains unchanged, append-only SQL steps. |

### Rules the tests check

`cargo test --test architecture` reads the module tree from `src/lib.rs` (a file reached by
`#[path]` is in the module that declares it) and checks: every file in `src/` is in that tree,
opens with a `//!` and holds at most 300 working lines; each top-level module uses only the
others its row of the module map allows (`tests/architecture/module_map.rs`, written from this
file: the foundations (`paths`, `store`, `wire`, `view`…) use nothing of the crate, the kept owners
(`projects`, `ledger`, `layout`, `board`, `feed`) nothing above them, the window (`browser`) no
domain, and the adapters reach the app through `client`), with each real dependency the map does
not draw named there as an exception with its reason; the top-level modules form no cycle; no
working file holds an inline test module (tests sit in `x_tests.rs`); and nothing pulls in an async
runtime (no such dependency in `Cargo.toml`, no `async` or `await` in `src/`). Paths are read from
code only, never from comments or strings: `crate::x`, grouped `crate::{x, y::z}` and `super::`
that climbs to the root. Store tests fingerprint existing migrations. These checks run with the
normal tests in CI.

## Layer map

- **`projects.rs`** is the scope everything is kept in: a project is a folder (the nearest one with a
  `.git` around where the caller works, `projects::root_of`), a row of `projects` found by its
  canonical path, never by its name; Home (`~`) is the project of what has no folder. Two of one
  name are told apart by the folder above (`labels`). Under a project: its tabs (the ledger), its
  board, its Event Center events, and its agents (`--as NAME`: an agent's own tabs and notes within
  the project). Every call names its project (`project`: a folder; `agent`: a name within it), and
  the desk resolves it before anything is done (`desk/scope.rs`).
- **`ledger.rs`, `desk/ledger.rs`, `desk/restore.rs`** are the ledger of a project's panes: every pane the window opened
  for it (a page, a terminal, the board) is a row of `panes`, from `own` (the one place a pane becomes
  someone's) to `gone` (the one place it ends); a closed one is the project's history (the newest
  200 kept), one still open when the app quit is what it opens again (`reopen_project`: the same
  rows, pages at their address, the person's shell in each terminal's folder with its last screen
  above, dimmed, from `screens`; a program is never run again). The pages each tab showed are `visits`, which the
  address field suggests. The window is told each project's past as it changes
  (`Browser::ledger`), and tells the desk what a page showed (`Heard::Went`): the window keeps none
  of it.
- **`layout.rs`, `desk/layout.rs`** are the layout (`docs/windows.md`): the windows, each window's
  workspace of a project, each workspace's tabs in their places and each tab's tree of panes (a
  split is one tab of two panes). `desk/layout.rs` is its one owner: a pane placed in a tab of its
  own as it opens (in the same transaction as its ledger row), taken out as it closes, put beside
  another, a project or a tab shown; each change with its notices (`Window`, `Workspace`, `Tab`).
  It outlives the app, so a project opens again with the same tabs, places and splits.
- **`browser/`** is the window: a `tao` event loop on the main thread, a `wry` WebView per page, a
  probe script injected in each page for observing and acting (`probe/`: its start, which keeps the
  builtins it uses as they were at install and answers only the app's calls, the snapshot, the page's own
  requests, what the page tells, frames, the outline, refs found again, actions, typing, keys, comments,
  evaluate), the strip, comments, screenshots. What a page tells besides its content
  (its dialogs, answered Cancel unless a call armed OK, live-region announcements, a field that refused
  a value) comes with the probe's replies and is kept per tab across its documents (`notices.rs`).
  A call names a random id and word (`secret.rs`), and its reply counts only with both and only
  from the tab it went to (`calls.rs`).
  The window is an ordinary one, put away only when the person asks (`presence.rs`). `host` takes the main thread and runs the server on another; a `Browser` handle sends it
  commands. Panels are tabs whose page the caller paints (terminals, the board, Settings).
  The window shows one project at a time (`scope.rs`): its tabs are the ones listed, what the person
  opens is of it, its history is the one the menu lists, ⌘1…9 are its tabs. The strip's page is
  drawn in three views of its own (`deck.rs` `Part`): the bar (crumbs of where the shown tab is: its
  project, its menu every project and Open Folder; the tab, its menu the project's tabs), the side, and the browser's toolbar over a page only (back,
  forward, reload, the address, the page's size and comments). The switcher's menu
  (`host_project.rs`) lists the projects the app knows, Open Folder… and Close Project. A tab an agent opens or shows in another project opens out of sight there (the app
  says which project a page it opens is of, `Browser::opening`), and that project is marked in the
  switcher until the person goes to it: the window never jumps from what the person looks at.
  The side lists the project's tabs; a terminal that runs a coding agent (`agents.rs`, by its
  command) shows the agent's mark, and under its name the title its program sets (OSC 0/2, kept
  aside by `terminal/screen.rs`). The person's settings (`settings.rs`: theme, transparency,
  fonts, the shortcuts they changed) are kept in `settings.json` under the config directory and
  shown in a tab of the window's own (`host_settings.rs`, `settings.html`). Every shortcut
  (`shortcuts.rs`) is a click the strip already knows, fired by the app's hidden menu
  (`app_menu.rs`, macOS), so a key, a menu item and a button never do different things. The app
  is a regular Mac app inside its bundle (`platform::dock`): Dock icon, menu bar (LLX Desk, Edit,
  and a menu for each group of shortcuts), and nothing it does for an agent activates it; only the
  person's own acts do (`appkit::activate`). What the person does to the app itself reaches the desk
  as `Heard`: `Quit` (⌘Q, the Dock's Quit and logging out, through `applicationShouldTerminate:`;
  closing the main window, as ⌘Q does) and `Reopened` (the Dock icon, opening it again).
  `desk/lifecycle.rs` decides: a quit asks first (`Browser::confirm`, the system's alert) while
  terminals run a program, then stops the server (`Desk::on_quit`); a reopen brings the window on
  the project used last, as it was left (its ledger), else the person's login shell in it. The
  main window's minimize button does not minimize: it puts the window away, out of sight and
  still drawing, until the person opens the app again (`nspanel.rs`; another window's is the
  system's), and the green button uses the system's zoom on
  the current desktop rather than a separate full-screen Space. `appkit.rs` holds AppKit's common messages
  once. A split (one tab of two panes, `split.rs`: which tab is beside the shown one, on which
  side, and the halves' share) lays two panes out side by side, the one shown with the toolbar; a
  fourth view, the line between them, is dragged to resize. A page or a terminal takes its own clicks, so the window
  hears of presses through the system (`presses.rs`, a local event monitor) and a press in the
  other half shows its tab (`host_split.rs`). Another tab shown makes the window whole, and its project
  keeps the split in the layout: one of its two tabs shown again, it splits as it was. Each split the
  person makes or undoes, and each tab they drag, is told to the layout (`Heard::Split`,
  `Heard::Moved`); the layout tells every window its tabs back (`Cmd::Arranged`), and the window draws
  them so, keeping no order or split of its own. A split is listed in the side as a pair.
  The app has several windows (`windows.rs`, docs/windows.md): each a deck with its own panes, the
  first the main one. One is the window in use, as the system's key window is:
  the loop picks the window an event is for before it acts (a pane's command goes to the window the
  pane is in, a press makes its window the one in use), so a deck's code acts on one window. Pages
  take their ids from the app's places (`popup.rs`) and share the app's first page's WebKit
  configuration, in whichever window they are. ⌘N opens a window on the project, with the person's
  shell (`Asked::Window`); one closed takes its tabs with it. A tab dragged out of a side's list
  (`host_tear.rs`, `deck_tear.rs`) is carried in a small window that follows the pointer
  (`chip.rs`): past its window's edge it becomes a window of its own, over another window's list it
  goes into that list. The window's layout (the strip, the side, the pages) is `deck_layout.rs`;
  the menus a right click opens, a tab's and a panel's, `host_tab_menu.rs`.
- **`board.rs`, `board/`** are the board: its cards and the rules every move keeps (one card in Doing
  per worker, a worker acts on its own cards, Done is the person's), the person's actions from the
  window (`person.rs`), the board as text for the agent (`text.rs`), where it is kept
  (`store.rs`: the project's row in the desk's database, written out to the project's
  `.localix/board.json` by `desk/board_file.rs`) and its page (`page.html`, `page.js`), painted whole into a panel. `desk/board.rs` acts on a board's tab (`interact` with an `op`);
  `desk/board_told.rs` gathers what the person does into one note, told as a comment is. A card the
  person runs by an agent (`desk/board_run.rs`) is handed to its worker and the agent started as a
  terminal of theirs, as "+" starts one.
- **`terminal/`** is a session: a program on a PTY, its screen kept by `vt100`, painted into a panel
  when a window shows it. What is typed is queued and written by a thread of its own, so a
  program that reads nothing never holds the window. Output is untrusted. The model (`vt100`, what agents read) gets only
  text and CSI sequences, but the window's xterm paints the program's raw bytes: the page's own
  configuration (no clipboard addon, a link handler limited to http and https, `Said::Open`, the
  CSP) is the boundary there. A
  terminal starts with the caller's environment, but for the variables its integration names
  its own in `LLX_DESK_SCRUB` (the Claude Code plugin's launcher lists Claude Code's session
  variables), so the core knows no agent's names.
- **`desk/`** is the `window` tool. A `Call` (its project and agent, directory, environment) and a
  JSON object in,
  text out. Its arguments are checked against its schema (`args.rs`) before anything is done: a field
  it does not know, or one a tab does not take, is refused with what it takes. A page is acted on by
  the refs of its outline (`[e12]`: `click`, `fill`, `select`, in `web.rs`), never by coordinates. The registry says who owns which tab (`w1`, `t2`) and holds what the person said about it;
  nothing acts on a tab its caller does not own. A board is a kind of tab like a page or a terminal:
  `open` gives it, `interact` acts on it with an `op`, so the tools stay one surface.
- **`store.rs`, `store/schema.rs`** are what outlives the app: one SQLite database (`paths::database`), one writer,
  its schema the list of steps it applies in order: `projects`, and under each its `panes` (with
  their `visits` and terminals' `screens`), its board (`boards`) and its Event Center events
  (`center_events`); the layout (`windows`, `workspaces`, `tabs`, `pane_nodes`); the person's
  `answers` to offers; the change feed. Everything of a project
  goes with it (`ON DELETE CASCADE`).
- **`feed.rs`** is the change feed, the desk's one way to say that something changed. A kept owner
  (the boards and the layout: `Window`, `Workspace`, `Tab`) writes its notice, `{entity, op}`, in
  the transaction of its rows; a live owner (the registry of notes, the panes (`Pane { owner,
  key }`), the terminals, the asks) records it under the lock of its
  change. Each consumer has a name, a cursor kept in the store and a thread of its own, reads every
  notice at least once, re-reads the owner for what it needs, and reconciles from the owners after a
  notice it could not handle. Notices of what does not outlive a run (a pane, a terminal) are passed
  over after a restart. The entity kinds are one enum every consumer matches whole. Its consumers:
  `desk/relay.rs` tells subscribers (`comment/added`, `tab/closed`) through the server's hub; `desk/center.rs` is the Event Center; `desk/board_file.rs` writes boards out.
- **`desk/center.rs`, `desk/center/`** are the Event Center: what waits for the person, gathered from
  its sources (`event.rs`: what an event is and how it is shown; `sources.rs`: the agent's asks, a
  card in Review or blocked, a program that ended) whenever the feed says something changed, kept in its journal (`journal.rs`:
  `center_events`, open and settled, with the person's marks), and shown under the strip's bell with
  buttons that take the person's own paths. An event says how it is shown (its section, tone, and
  each button's role), so the window holds no table of kinds.
- **`view.rs`** is what a page shows and how it is acted on: an observation with each control's ref,
  and an action on a ref against the observation's epoch, so it never lands on a newer document.
- **`secrets.rs`** keeps what an answer must not show: saved secrets, named `{{secret:NAME}}` and put
  in by the server (a page's `fill`, a terminal's `input`); every answer masks them.
- **`served.rs`** serves a project's files on the loopback so a path opens as a site.
- **`wire.rs`** is the protocol: JSON-RPC 2.0, one message per line, `initialize` first. A response may
  announce bytes (`binary: {length, mime}`) that follow its line, written with it as one unit
  (`wire/pipe.rs`: a connection's one writer thread and its bounded queue). The methods
  (`server/api.rs`, one table that the dispatch, `api/describe` and every adapter read: name, summary,
  params as JSON Schema, whether it is work): `window/call`, `window/said`, `window/end`,
  `window/present`, `comment/ack`, `events/subscribe`, `events/unsubscribe`, `api/describe` (every
  method, the events, and the window tool's description and schema), `ping`, `app/status`,
  `app/quit`. Work runs on a thread of its own (answers come by id), a request with an
  `idempotencyKey` is done once (`server/idem.rs`, kept server-wide: the retry arrives on a new
  connection), a panic is that request's error. Only the user's own processes are served, and a client talks only to its
  user's own server (the peer's uid, `wire/peer.rs`). The server notifies by subscription (`server/hub.rs`): `comment/added` (kept by the
  desk until acknowledged, and told again on every subscribe), `tab/closed`, `app/quitting`.
  `client.rs` and `client/link.rs` are the other end: `client::ask` is the one request every adapter
  makes, with the caller (`client/params.rs`: its project, from `--project DIR`,
  `LLX_DESK_PROJECT` or its folder, and its agent, `--as NAME`); when nothing listens it starts the
  server: the app through LaunchServices, in the
  background, when this program is the app's (`bundle.rs`), else this program as `serve`. The CLI,
  `--said --follow` and `mcp.rs` (an MCP server on stdio whose one tool is the window tool as
  `api/describe` gives it) are its clients; `llx-desk window --board` reads a project's board from its
  file and starts nothing.
- **`cli/app.rs`** is the program opened as the app (LLX Desk.app from the Finder or the Dock;
  `llx-desk open` elsewhere): it sets up what is always on, then is the server with its window
  brought to the person at once. On macOS `llx-desk open` asks the system to open the bundle, as the
  Dock does, and returns once it has (`cli/app.rs`); the app is not that terminal's. `llx-desk
  open DIR` brings the window on that folder as a project
  (`window/present` with `go`). `scripts/bundle-macos.sh` builds the bundle (Info.plist, icon and
  entitlements in `packaging/macos`, the Claude Code plugin as a marketplace in `Contents/Resources/claude-code`);
  `scripts/dmg.sh` makes the release files from it, the disk image and the zip (notarized with
  `--notarize`, as the release workflow does). The app is for Apple Silicon only.
  `packaging/homebrew/llx-desk.rb` is the cask a tap takes, filled in per release.
- **`setup.rs`, `setup/`** are the integrations, reached from Settings → Integrations and from
  `llx-desk setup`: the command (`command.rs`: a link in `~/.local/bin` to the program, the login
  shell's profile when its PATH lacks it; a desktop entry on Linux), the Claude Code plugin
  (`claude.rs`: installed from the bundle's marketplace with `claude plugin`), and the window tool
  in the other agents of the window's that take MCP servers (`mcp_hosts.rs`: one table of Codex,
  Gemini CLI, Copilot, OpenCode, Hermes — each put in and taken out with its own `mcp add` /
  `remove`, OpenCode's JSON written as it has none; whether it is there read from its
  configuration file). The app puts every one in place itself when it opens (`opened`, `auto`),
  for each agent installed; what the person takes out is remembered in the store's `state`
  (`integration.declined.<id>`) and left out. The person's programs run as their terminal would
  (`shell.rs`: their login shell's PATH, a deadline each, an answer for one that asks). An
  integration names the agent it is for and what to offer.
- **`offers.rs`, `browser/offer.rs`** are what the app offers the person at the moment it fits:
  one quiet bar under the strip: the agent's mark, the button that does it, and a cross that puts it off (Not Now). `offers.rs` keeps their answers
  (`answers` in the store; the window asks through `browser::Answers`): taken is never offered again, put off waits a day, three,
  a week, then a month. `browser/offer.rs` says what to offer for the tab shown; today an agent in
  front in the terminal shown (`terminal/foreground.rs`: the terminal's foreground process, by the
  name it was started as, read when another group comes in front) whose integration is off. Another offer is
  a source there and an id of its own; the bar, the answers and the waits are shared.
- **`paths.rs`** is where the server lives: its lock and socket at one fixed place per user
  (`llx-desk-<uid>` in the user's temporary directory), its files in the home (`LLX_DESK_HOME`, which
  moves nothing else). A test build (feature `test-server`, turned on only by the crate's own dev-dependency)
  takes the place from `LLX_DESK_TEST_DIR` and never opens a window; `harness.rs` is its test support.

## Rules

- Synchronous code: threads where needed, every thread named and started with `thread::Builder`; no
  async runtime.
- A panic is a bug, and one request's panic is that request's failure, never the server's end
  (`catch_unwind` around each request; release builds unwind). No `unwrap`/`expect`/`panic!` in working
  code; the lints in `Cargo.toml` are hard and an exception is an `#[expect]` with its reason.
- State shared across threads is a `Mutex` or an atomic. A guard is never held across a send, a wait
  for the window, or a callback.
- The server starts only for a call that needs it. Ending tabs and fetching comments never start it.
- One mechanism, never copies: a method is a row of `server/api.rs`, an act of the person's a
  `Heard`, a shortcut a row of `shortcuts.rs` that the menu bar, Settings and the strip all read; an
  adapter holds no list of its own.
- Every file opens with a `//!` that says what it is, and holds at most 300 lines of working code.
- Unit tests sit beside the code (`x.rs` ends with `#[path = "x_tests.rs"] mod tests;`). Tests that
  open a real window run only with `LLX_DESK_DISPLAY_TESTS=1`; `tests/server.rs` drives the binary
  against an app of its own with no window (a test build, in a directory of the test's own).
