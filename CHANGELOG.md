# Changelog

## Unreleased

### Added

- A wait on a terminal ends when the shell's command does, and says how: "The command ended with
  exit code 1 after 3.2s", where it could only say the output settled. A text waited for that
  never shows is no longer waited out once the command is over, and the screen's read says
  whether the shell's command still runs or how the last one exited. A command at work is
  waited for to its end or the call's limit ("The command is still running after 10s"), not
  given up on at its first quiet moment; one that takes keys (an editor, a REPL) or stays
  quiet for three seconds (a question) ends the wait as before. zsh and bash in a tab say so
  by themselves: the shell starts on a file of the app's that reads the person's own files
  first (for bash: `/etc/profile`, then the first of `.bash_profile`, `.bash_login`,
  `.profile`, as a login shell reads them), then marks each command's start and end; nothing
  of theirs is changed, and `LLX_DESK_SHELL_MARKS=0` in the environment leaves the shell as it
  is. A bash with a DEBUG trap of its own is left unmarked, and so is a line that is one
  subshell alone (`( … )`). Any other shell that marks its commands (OSC 133) is read the same
  way.
- What waits for you is said outside the Event Center's list too, all three read from that one
  list:
  - a tab that waits carries a dot on its icon while it is out of sight, and its second line says
    what waits (`Claude asks: Which database…`);
  - while you are in another app, each new thing is a notice in Notification Center, with no
    sound; a click shows the tab it is about (the list, where it has none), and the notice is
    taken back once the thing no longer waits. macOS asks once whether to allow them, when the
    first one comes; System Settings → Notifications turns them off. What already waited when
    the app was started is counted and not told again, and a notice an earlier run left is
    taken back;
  - the Dock's icon counts what is new, until the list is seen.
- A session that waits for you in a terminal is that terminal's line in the Event Center: it has
  Show, the coding agent's mark and name, and two agents of one project each keep their own line
  (one used to replace the other).
- The Claude Code plugin says more of what waits for you, each as one line that goes when the
  session goes on: a question Claude asks you (`Claude asks: Which database…`), and a turn an
  error ended (however short). A turn you stopped yourself says nothing.
- The Event Center has its button in the bar again, before history's: a bell alone, with the
  crumbs' dot while something new wants the person. With no room it waits in "»" with the rest.
- What a program asks its terminal to tell the person (a notification: OSC 9, OSC 777
  `notify`) waits in the Event Center under "Programs Say", with the tab to show, until they
  type in that terminal. The words are the program's, shown as plain text and told to no agent.

- What an agent opens from a terminal of the window lands beside that terminal, in its tab: the
  agent on the left, its page, terminal or board on the right, and the keys stay in the terminal.
  The next thing it opens takes that place, and the one before becomes a tab right after; `show`
  of a tab brings it there too, so the window is not taken from the terminal. Each terminal has
  its own place. A split you make whole (⇧⌘\) stays whole until you split that terminal again,
  a tab you arranged yourself is never replaced, and an agent with a name on the board (a
  worker) opens tabs of its own. The call is known by the terminal its program descends from,
  with any agent and no setup. **Settings → Agents → What an agent opens** turns it to plain
  tabs.
- Claude Code in a terminal of LLX Desk opens the pages the person asks for ("open google") as
  tabs of the window, not in another browser it has the tools of, unless the person names it.
  It is told so by a row at the start of the session, which holds where an organization's
  policy passes over what a plugin adds to the system prompt.
- A card waiting in Backlog or Todo is run by an agent the person picks among those installed
  (Claude Code, Codex, Gemini CLI, GitHub Copilot, OpenCode, Hermes): ▶ on a Todo card or on the
  open card, or Run With in a card's menu. The card goes to Doing as that agent's worker
  (`codex-c3`), past the limit, and the agent starts in a terminal of the project, titled
  `c3 · codex`, on a first prompt that names its card, its name on the board and how to hand it in.

### Changed

- A terminal takes in a flood of output about four times as fast while a call waits on it: the
  wait is woken once a frame, not with every kilobyte, and the program in front is read on a
  timer. The window is sent the output 60 times a second (it was 30).
- A wait on a terminal whose program asks for a password (input it hides) ends at once and says
  so, as the screen's read does, where it sat out its whole time before.
- A program that dies with the mouse reported or the cursor hidden no longer leaves the shell's
  prompt taking clicks as typing: when the shell says the command ended, both are put right.
- Lines an agent types into a program that takes pastes (an agent's prompt, a REPL's editor)
  arrive as one paste, and the line breaks that end them are Enter after it. Before, each line
  was Enter and a prompt sent its first line alone. One line is typed as ever.

- `llx-desk mcp` speaks the MCP revision an agent asks for: every one from 2024-11-05 to
  2025-11-25 after the `initialize` handshake, and 2026-07-28 with none, each request naming its
  revision (`server/discover` says which are served; one not served is refused with the list to
  try). It tells the agent what the window is for (pages, terminals, the board), names the tool
  "LLX Desk Window" and says that it changes things outside the agent. A call the agent cancels
  is not answered, and one it follows with a progress token is told every few seconds that it
  still runs, so a long wait is not given up on. Arguments that are not an object, an id that is
  neither text nor a number, a cursor into the one-page tool list and a batch of requests are
  each refused with the protocol's own error.
- The window tool's description fits the 4096 bytes some agents keep of one (it was 5809): each
  fact is said once, in the field it belongs to or in its kind's guide, and every field says what
  it takes. A `tabs` list is declared as the ids or the tabs it holds.
- The Claude Code plugin's version is the one a session loads: read from the app's folder, the
  folder's own, so an app that was updated is not told its plugin is older.
- A card's hover move (Move to Todo, Reopen, Unblock, Run) is a quiet grey icon with its words as a
  tooltip, in the age's place, instead of a filled button with text and a shadow.
- Open Folder… and the question before quitting are sheets on the window (an alert of their own
  only while the window is away), so pages, terminals and agents keep running while they wait.
  Quitting again while the question is up keeps that one question; its answer goes to both.
  Screenshots are encoded off the window's thread. Refreshes during a command are drawn once, the
  page's scripts are sent only when what they say changed, and views that already stand where a
  layout puts them are left alone. A load into a closed tab is refused at once, and a closed tab
  leaves no project or detail entry behind.
- Sidebar and split-divider resizing use one captured pointer and demand-driven animation frames,
  with Escape/cancellation cleanup and keyboard resizing. Title-only tab updates preserve their
  DOM nodes without remeasuring the list; FLIP is reserved for actual row changes.
- Board snapshots coalesce to one paint per frame and clock-only frames no longer rebuild cards.
  Card drags and IME composition defer replacement until safe; draft text, focus and scrolling stay.
  Terminal fitting coalesces resize and font changes, with a fallback for concealed windows.
- Split the large Rust modules along their responsibilities: board data and allocation, tool
  calls and events, pane restoration and layout projection, native UI commands and view construction,
  tab transfer and sizing, page reads and controls, terminal waits and SQLite migrations. Public
  entry points and the stored schema are unchanged. Removed redundant dispatch wrappers and an
  unreachable board interaction branch; connection shutdown now releases the pending-call lock
  before notifying its waiters.
- Normal tests now enforce documented, named Rust modules under 300 working lines in `src/`,
  domain-model independence from adapters, and unchanged existing SQL migrations.
- The Claude Code plugin asks the program that answered last first, so a `llx-desk` missing from
  the PATH no longer costs a failed start every few seconds, and redraws its line only when
  something changed. A hung app is no longer asked again under each of the program's places, and
  closing the tabs as a session ends waits 1.5 s at most.
- CI also runs the Bun tests, shellcheck and `cargo deny`; a test pins the `status` and `list`
  lines and the board columns the plugin reads. Releases build with an exact Rust and carry a build
  provenance attestation. `install.sh` runs only once it is downloaded whole, and can pin the
  team that signs the Mac app (`TEAM_ID`).

- Settings' Theme and Window Transparency are pop-up buttons, as the fonts are, rather than
  segmented controls. The shortcuts search is a field in the controls' own fill and hairline, no
  longer a solid, sticky bar. The terminal builds its screen-reader tree when VoiceOver is running.

- Type follows one scale on every page of the app: each size names its own line height in whole
  points (13/16, 12/15, 11/14, 10/13, 15/20, 22/26), instead of a ratio inherited from the page
  (`1.35`, `1.3`, `1.38`, `1.4`, `1.45` made lines of 16.2 and 17.55 pt). Settings' title is
  semibold, headings balance their lines, and fonts are never faked bold or italic.
  `tests/typography.rs` keeps the scale.

- Every page the app builds takes the first click of a window that is not the key one, so a
  button acts at once, not on the second press; a test requires it.

### Security

- A key in what an agent reads (a terminal's screen, a page's text, a script's answer) is shown
  as its kind, `[hidden: GitHub token]`, not its value: GitHub, AWS, Google, Stripe, Slack,
  GitLab, npm and model API keys, signed tokens and private keys, known by their shape. The
  window shows the person what it always did.
- A page's own scripts can no longer speak for the app or the person. Only the app's own calls
  reach the probe, by a per-run key. Each call carries a random 64-bit id and a 128-bit word from
  the system's random source, and its reply counts only with both and only from the tab it went
  to. A comment counts only while the person is commenting in that window, and only the app's
  checks take a script's presses as the person's.
- The probe keeps the builtins it uses from when it is installed, and builds its replies from
  plain copies. A page that changes prototypes or gives Object and Array a `toJSON` cannot alter a
  reply or reach the comment card.
- The strip and Settings run their one script by its SHA-384 and allow no inline script. The
  agents' marks reach the strip as JSON data.

### Removed

- The mini player, the small floating preview of the tab. The main window's yellow button and ⌘M
  (Window ▸ Minimize) put the window away: nothing of it on screen, its pages still drawing, its
  tabs kept. Only the Dock icon or `llx-desk open` brings it back, never an agent.
  The bar has no mini button and no Event Center button any more; ⇧⌘E and the Window menu open
  the Event Center, under the bar's end.

### Fixed

- GitHub Copilot and Hermes are offered only by their own commands, `copilot` and `hermes`. A
  shell helper that suggests one command and a runner that does one task and leaves were taken
  for them, and started a program that is not the agent.
- The window tool in Codex is found when its `command` is a literal string (a path with a quote
  or a backslash) or has a comment after it; it read as "not added" and was added again.
- An agent's configuration is read where the agent itself keeps it: in the folder its own
  variable names (`CODEX_HOME`, `GEMINI_CLI_HOME`, `COPILOT_HOME`, `XDG_CONFIG_HOME` for OpenCode,
  `HERMES_HOME`) as your login shell sets it, and for Hermes in the profile in use. The agent's
  `mcp add` runs with the same variables. It was always read under the home directory, so a moved
  one read as "not added" forever.
- Settings with comments in them (Gemini CLI, OpenCode) are read; they counted as empty.
- The window tool already added to an agent is left as it is when the app opens or you press Add:
  the agent's `mcp add` wrote the entry anew, without the timeouts and tool lists you set in it.
- GitHub Copilot gives a call of the window tool two minutes (it gave 30 seconds, less than a
  terminal may be waited on); an entry added before is added once more with the timeout.
- After adding the tool, Settings and `llx-desk setup` name the agent's own reload command
  (`/mcp reload` in Gemini CLI and Copilot) in place of starting it again.
- On Linux, a terminal opened with a command (an agent from "+") names that program in front, not
  `sh`: the shell that only runs the command is looked past, as macOS's replaces itself.
- The app and the command line opening the database at the same moment no longer leave the app
  keeping nothing for its whole run: the second waits for the first, and a schema step is never
  run twice.
- What the window tells as the app quits (a tab closed, a project shown) is written before it
  ends, and the window never waits on the disk to close a tab, show a project or move a tab.
- A program's cursor-position query split across two reads of its output is answered.
- The Event Center no longer shows an older list over a newer one when a change and a click
  arrive together, and a page's changes are worked out without holding up other tabs.

- Interactive terminals no longer inherit `NO_COLOR`, `FORCE_COLOR` or `CLICOLOR_FORCE` from
  automation clients. They advertise their own terminal identity, 256-color/truecolor support
  and `CLICOLOR=1`, including for macOS tools such as `ls`. Shell profiles and explicit per-command
  color preferences still take precedence. Color output and screen replay are tested in the real
  xterm panel for ANSI, 256-color and RGB modes.
- Split View updates its paired sidebar row even when no other tab data changed, and saves the
  divider's final proportions in the pane tree. Restored layouts cannot undo a newer local drag
  while its write is being projected back.
- Viewport types and the view-layout facade are available on Linux as well as macOS; an
  accidental macOS-only module gate and missing GTK layout stubs prevented the Linux browser
  from compiling. GTK retains ownership of its child layout.
- Malformed UI geometry messages with a fifth non-numeric argument are refused, rather than
  silently accepted as four coordinates.
- An exhausted board card counter refuses new cards without changing the board, rather than
  reusing its last id. Tab ids must contain decimal digits, not a signed number.
- Invalid JSON-RPC ids (objects, arrays and booleans) are rejected as invalid requests, rather
  than being treated as notifications.
- Claude Code's long-turn notification and its removal explicitly name the session's project,
  like the plugin's other window calls. A mocked-host regression test covers both calls and
  ensures aborted and short turns send no notification.

### Also in this release

- **A full app.** The window is an ordinary window of the app, not a panel: a click makes LLX Desk
  the app in front, with its menu, and the green button zooms it on the current desktop. A window an agent
  opens still comes to the front without taking the keys. `llx-desk open` has the system open the
  app and is done, so closing the terminal it ran in leaves the app running.
- **Apple Silicon only on macOS.** The app is built for Apple Silicon; there is no Intel or
  universal build.
- **A signed, notarized disk image.** A release carries `LLX-Desk-macos.dmg` (the app and a link
  to Applications) beside the zip `install.sh` fetches, both made by `scripts/dmg.sh`, signed with
  a Developer ID and notarized by Apple, so Gatekeeper opens them on any Mac. A macOS job in CI
  builds the app and smoke-tests it on every push, and the release builds it on a Mac runner.
- **Programs in a terminal can ask for the camera, the microphone and the rest.** macOS counts a
  program run in a terminal as part of LLX Desk: the app now carries what the system asks the
  person for each (microphone, camera, location, Bluetooth, contacts, calendars, reminders,
  photos, other apps, the Desktop, Documents and Downloads folders, volumes, administrator
  rights), and is signed with the matching entitlements, so the system asks instead of refusing.
- **A Homebrew cask.** `packaging/homebrew/llx-desk.rb` is the cask for a tap, filled in with a
  release's version and checksum.
- The terminal's cursor is in the text's colour again (an outline in a pane not in use), and the
  terminal draws no overview ruler beside its scrollbar.
- A tab the person opened and then closes is no news to an agent; one an agent opened still is.
- **Quieter at rest.** The window refreshes less often, a page's title is written to the store
  off the interface's thread, and a terminal out of sight paints less often.
- **A tab out into a window of its own.** Drag a tab past the edge of its window and a window opens
  with it at once, following the pointer; the page or terminal moves as it is, nothing reloads.
  Over another window's list of tabs it turns clear and the list opens a slot: let go there and the
  tab goes into that window. Let go anywhere else and the new window stays. With one tab in the
  window, the window itself moves. Back over its own list, or Escape, takes it back.
- **Windows.** ⌘N opens a new window on the project shown, with your shell in it; each window has
  its own tabs, side and split, and an agent's tabs open in the window you work in. Closing a window
  closes its tabs. Agents see every window's tabs. After a restart the tabs of the other windows are
  gathered into the first.
- **Split View.** Two of a project's tabs side by side, any kind in either half: ⇧⌘\, any tab's
  menu (Open in Split View / Close Split View), or a tab dragged from the side onto the pages
  (the window splits as it moves). A split is one tab of two panes: the side lists the two as a
  pair, the half in use lit. Another tab picked shows it whole, the split kept: either of its tabs
  picked again (still listed as a pair), the window splits as it was; so too on coming back to the project, and after
  the app restarts.
- **Tabs drag.** A tab dragged along the side goes where it is let go of, the others sliding aside;
  ⌘1–⌘9 follow the order. The rail's button and tabs are one tile size, lit under the pointer.
- In a split window a click in a half makes it the one in use: it has the page's toolbar; a tab
  dropped on it takes its place. The line between the halves drags to resize and double-clicks to
  even them. A split window shows one project.
- **A compact bar, and a toolbar of its own for a page.** The bar is one row, a compact
  titlebar's height: crumbs beside the window's buttons, 📁 the project ⌄ / the tab ⌄ (the
  project's menu: Open Folder…, every project, Close Project; the tab's: the project's tabs), and
  the window's buttons. Back, forward, reload, the address field, the page's size and comments
  moved to a toolbar over the page, there only when a page is shown. A terminal's menu (New
  Terminal Here, Show in Finder) works where its shell is now.
- The app's icon: the bird on a near-black plate, larger, without the glow and the rim that made
  it look dim in the Dock.
- **Nothing of one project reaches another.** A page a page opens (`window.open`) is of its
  opener's project, never of the one shown; an agent's first page is of its project from the
  start; a subscriber always names its project (`events/subscribe` without one is refused) and a
  tab of no one's is told to no one; a project's terminals are never ended to make room for
  another's; Show on another project's event (and the person's acts from the Event Center) go to
  that project first; an event whose id comes back in another project or after it settled is
  new, not dismissed; an owner is its project's id (a folder's name never makes two alike); a
  session's end takes its asks with it; a crumb's terminal stays inside the project (no `..`);
  what the last run left open is history once the project has a tab again. Settings are the
  window's own, listed in every project.
- **The window stays when every tab is closed.** It offers, centred and quiet as a menu, what opens
  a tab: New Page, New Terminal, Board, Open Folder…, each with its key as Settings sets it. Every
  other empty place says what would be there in one quiet line, with no glyph, frame or button:
  the tabs ("No Tabs"), the history, the Event Center, a search in Settings, a board column ("Cards
  ready for an agent").
- **The glass's levels are even steps**, Standard as a system window's (half the desktop through),
  Low and High a step either side, Off opaque.
- **The side shuts to a rail** of the tabs' pictures, its button where it was, instead of
  leaving the window. An agent showing a tab of a project the window does not show gives it its tab back there,
  without moving the window.
- **Every agent connected by itself.** Each time the app opens it puts its integrations in place
  for whatever agent is installed: the Claude Code plugin, and the window tool in Codex, Gemini
  CLI, GitHub Copilot, OpenCode and Hermes, each with the agent's own `mcp add` (OpenCode's JSON,
  as it has no such command). One the person removes stays removed. Cursor is no longer one.
- Settings' icon is a gear; a shortcut being recorded shows the keys held as they go down.
- `window --said --follow` ends on its own when the session that started it is gone: no follower
  is left behind waiting for the app.
- **The mini player only when you ask.** The window opens as an ordinary window and stays one
  when you click another app; the mini (the small floating preview) comes from its button in the
  bar, the yellow button or ⌘M, as a video's player shrinks to a corner. `LLX_DESK_KEEP_EXPANDED`
  is gone with the old behaviour.

- **Projects are the scope of everything.** A project is a folder (the nearest one with `.git`),
  kept in the database with everything that is its own: its tabs, the pages they showed, its
  terminals' last screens, its board and its events. The window shows one project at a time,
  named left of the address: click it (⌃⌘P) for the projects used last, Open Folder… (⌘O) and
  Close Project (⌃⌘W). The tab list, ⌘1…⌘9, the history (⌘Y), Reopen Closed Tab and the
  address field's suggestions keep to the project shown; an agent working in another project
  never moves the window, its project gets a dot. A project comes back as it was left, after a
  restart too: its pages at their addresses, a shell in each terminal's folder with the last
  screen dimmed above. `llx-desk open DIR` opens the window on a folder as a project. Every call
  names its project (`--project DIR`, `LLX_DESK_PROJECT`, else the folder it runs in); `--as NAME`
  is an agent of that project. The plugin passes its session's folder. A new database
  (`llx-desk.sqlite`) and protocol 3: nothing of the old owner strings is carried over.
- The tab's menu fits its kind: Reload, Duplicate, Copy Address, Open in Default Browser (also
  ⇧⌘O) for a page; Copy Folder, Show in Finder for a terminal; Close Other Tabs and Close Tabs
  Below for all.

- **LLX Desk is the app, and the app is all there is to install.** It is the server: the window,
  the terminals, the board and the database live in it, and it serves one API on a unix socket.
  The `llx-desk` command, `llx-desk mcp` and the Claude Code plugin are thin faces of that API with
  nothing of their own. On macOS it is a regular app (Dock icon, menu bar built from the shortcuts,
  ⌘Q); the first call of an agent starts it through LaunchServices, in the background, and nothing
  an agent does takes your keys. Opened, it links `llx-desk` into `~/.local/bin` and brings the
  window to you, with what the project had open (nothing left open, the window's start of a new
  tab; no terminal opens unasked). The Dock icon brings the window back;
  the main window's close button quits as ⌘Q does (as does the Dock's Quit, or logging out),
  asking first while terminals run a program. `install.sh` installs the app (macOS:
  `~/Applications`; Linux: the program, the command and a desktop entry, `llx-desk open`).
- **Settings → Integrations** and `llx-desk setup`: the command line, the Claude Code plugin
  (installed from the app's own bundle, of its version, and refreshed when the app updates), and
  the window tool as an MCP server in Codex, with one click; any other MCP host
  takes the entry *Copy MCP Entry* gives (`llx-desk setup mcp`).
- **The API describes itself**: `api/describe` gives every method with its params as JSON Schema,
  the events, and the window tool's description and schema, from one table the dispatch reads
  too. `llx-desk mcp` is back as an MCP server on stdio whose one tool is that window tool. Only
  the user's own processes are served.
- Removed: the plugin's download of a program of its own version (it runs the app's), the
  `window/closed` event (the window no longer closes but to the mini, or by quitting), and the
  handling of servers of the older protocol.
- **No daemon apart from the app**: `llx-desk status` and `llx-desk quit` replace `serve status`
  and `serve stop` (`status` exits 3 when the app is not running); the methods are `app/status`
  and `app/quit`, the event `app/quitting`; the log is `app.log`. `serve` is the app's own
  launch, no longer listed.
- **The theme is the person's alone**: system, light or dark, for the window and its pages; no
  page or terminal changes it any more.
- **A bare bar**: no capsules round the buttons or the address; icons and words on the glass, a
  button lighting as the pointer comes. Only a button that is on and the address being typed in
  have a ground.
- **One glass for the window's chrome**: the bar, the tab list, a terminal and the board are the
  window's own glass in every tab. A page no longer runs under the bar, blurred by a material of
  its own that read in another colour than the tab list beside it: it starts below the bar.
- **Offers, at the moment they fit**: run Claude Code or Codex in a terminal of the window (opened
  as an agent, or typed in its shell) while its integration is off, and a bar under the strip
  offers it: *Add Plugin* / *Add to Codex*, or *Not Now*, which asks again after a day, then three,
  a week, a month. Taken, never again. The bar is one component any part of the app can offer
  through. A terminal shows the mark of the agent in front, typed in its shell too.
- **Only what fits the tab, and a bar that stays still**: back, forward, reload, the page size
  and comments are a page's; over a terminal, the board or the settings they fade out where they
  stand, their room kept, so nothing in the bar moves as the tabs change, and the menu bar greys
  them. Over a terminal, the board or
  the settings the address field takes an address for a new page.
- **The history opens every page again**, the browser too when none is open (it did nothing once
  the last page closed). A page the person opens (New Page, the history, an address typed over a
  terminal, Duplicate) is one ask of the desk, a page of the project's, wherever it comes from.
- **Shortcuts**: ⌘1…⌘9 show the project's tabs by place, and nearly everything the window does has
  a key (⇧⌘[ ⇧⌘] tabs, ⌘T new page, ⌥⌘T terminal, ⌘W close, ⇧⌘T reopen, ⌘L address, ⇧⌘A search
  tabs, ⌘R ⌘[ ⌘] page, ⌃⌘S sidebar, ⌘Y history, ⇧⌘E events, ⌘, Settings…). Each is the strip's
  own click, and the buttons' hints name their keys.
- **Settings**, a tab of the window: theme (system, light, dark), window transparency (off, low,
  standard, high), interface font, code font and the terminals' font size, all applied at once;
  every shortcut can be recorded anew, taken off, reset, or all reset, and a key another shortcut
  has asks before it moves. Kept in `~/.config/llx-desk/settings.json`.
- A terminal takes a Mac terminal's keys: ⌘K clears it (a shell draws its prompt again at the
  top), ⌘← ⌘→ go to the line's start and end, ⌥← ⌥→ move by words, ⌘⌫ ⌥⌫ delete to the start and
  the word before, ⌘↑ ⌘↓ scroll to the top and bottom, ⌘A selects all.
- The side's search is a line of the column, no field drawn around it, with Settings, "+" and
  the button that hides the side beside it (hidden, the bar's button brings it back); a tab's hint names the key that shows it (⌘1…⌘8 by place, ⌘9 the last).
- A terminal running a coding agent shows the agent's mark in the side, its name, and under it the
  title the session sets (Claude Code's topic, for one), spinners left off. Any terminal shows its
  program's title there.
- The core names no agent's variables: what a terminal leaves out of its caller's environment is
  what the integration lists in `LLX_DESK_SCRUB` (the Claude Code plugin's launcher lists Claude
  Code's session variables).
- Leaks and waste found in a review of the whole: a terminal whose program ended, or whose tab the
  person closed, gives its place to a new one at the limit (now 16) instead of refusing it, and
  two starts never take the last place together; a session is closed once, so no reaped id is
  signalled; the change feed keeps its consumers' cursors once a page, not once a notice, and
  the store commits without waiting for the disk (WAL, `synchronous=NORMAL`); the Event Center
  writes its journal only when what it gathers changed; the board's actions from the window run
  off its thread, in order; a card's history keeps its first step and the latest 200; an owner
  that never calls again holds at most 200 notes; comment pictures are pruned as screenshots are;
  the local file server stops writing to a client that stopped reading and serves no file over
  64 MB; an idle terminal's reader wakes once a second, not 30 times; a connection over the
  daemon's limit is told why; the environment goes with a call only when it opens a tab or names
  a program, read from the call, not from words in it.

- The desk knows no model and no agent. `goal` and its decision model (Jev over OpenRouter, Claude
  Haiku through Claude Code), the check of a card handed in, `llx-desk key`, `llx-desk mcp` and the
  plugin's MCP server are gone: the desk makes no network call and holds no key. Every agent drives
  it from its shell with `llx-desk window`, and an integration (the Claude Code plugin today) is its
  own side of it. A card handed in goes to Review as it is; only the person accepts.
- A page is acted on by the refs of its outline (`[e12]`): `click` (one ref or several, in order),
  `fill` (`{"e4": "text"}`, `{{secret:NAME}}` typing a saved secret) and `select`
  (`{"e7": "Large"}`). A ref that is gone or covered stops the rest and says so.
- A call with a field the `window` tool does not know (`goal`) is refused with the fields it takes,
  and nothing is done, where it used to be ignored.
- `llx-desk window --board` prints the project's open cards from `.localix/board.json`, with no daemon: what
  an integration gives an agent as it starts. The Claude Code plugin's `board` skill loads with it.
- The Claude Code plugin works through the CLI: its skills allow `Bash(llx-desk:*)`, its worker
  calls `llx-desk window`, and a worker's call on the board without its name as `who` is refused
  with the name to pass.
- `LLX_DESK_PROJECT` names the project a call belongs to, when it is not the one around the
  current directory.
- The Event Center: a bell in the window's toolbar counts what waits for you, and opens a list of
  it, each with its buttons: a card handed in (Accept, Open Board), a blocked card (Unblock), a
  terminal whose program ended (Show, Close); any of them dismissed until
  it changes. It is gathered from what the desk holds, so it is never out of step with the board
  or the tabs.
- The agent asks the person on a tab (`ask` on `act`/`interact`, any kind of tab): sign in, a code,
  a decision. It waits in the Event Center until they press Done, which reaches the agent as their
  note; one a tab, taken back with `""`, gone with its tab. The bell counts only what wants the
  person (an ask, a card to accept, a blocked card, a program that failed);
  what ended well is listed without counting.
- After a long turn (30 seconds or more), the Claude Code plugin leaves one line in the Event
  Center that the session waits for you, with the first line of Claude's answer; the next turn
  takes it back. `ask` with no tab says the same for any agent.
- The board has a Discarded column, last: what the person gives up, from any column. The agent
  hears it as a move, cannot take or change a discarded card, and reads it named in the board's
  snapshot.
- Every menu of the window is the system's: a panel's page names its own items (a card's Open,
  Move To, Discard, Delete; the terminal's Copy and Paste) and the window shows them, with lines
  and submenus, handing the pick back to the page. The board's card shows its status as a pop-up
  button and is deleted from a trash icon in its header.
- "+" opens a New Page even when the window holds only a board or a terminal.
- The Event Center and the history menu open under their buttons, their right edges on the
  buttons', not under the whole strip. Clear All empties the list at once: the window was not told
  when nothing was left to show.
- A tab the person closes no longer wakes the agent: it reads so with its next call.
- A new page no longer flashes white over a dark window: until it has drawn, the window shows
  through it in the system's light or dark, and the page draws over it, the first page of the
  window too.
- `interact` refuses a field the tab does not take (`op` on a page, `input` on a board) with what
  it takes and how to do it there, where it used to do nothing and say the page had not changed.
- A subagent passes its name on the board itself, as its task gives it.
- The Claude Code plugin's mod uses no classic hook and no prompt content, which an organization's
  security default keeps from the plugins a person installs: a worker's name is kept as it is
  spawned, a worker that ends with its card in Doing is sent word to finish it, and the project's
  tabs close as the session ends, all from the mod's own events.
- A terminal no longer carries the session of the agent that opened it (`CLAUDECODE`, `AI_AGENT`,
  `CLAUDE_CODE_SESSION_ID`, `CLAUDE_PROJECT_DIR`…): a `claude` started in it is a session of its own,
  of the directory it runs in, not a child of the agent bound to its project.
- An `op` on a page that names a field the call carries (`"op": "script"` beside its `script`) is
  let be.
- A page the desk cannot read (an SVG, an image) answers at once that it is one, where it used to
  wait five seconds for a reader that never comes. `wait_for` finds a control's name and a field's
  value too, as the outline shows them.
- One change feed under the desk: what changes is recorded once, by whoever owns it, and every
  part that cares (the notes to the agent, the Event Center, the board's file) reads it at its own
  pace. It is kept in a SQLite database (`~/.local/share/llx-desk/desk.db`, moved with
  `LLX_DESK_DATA`) with the boards and the Event Center's journal, written in the same transaction
  as the change it announces, so after a restart each reader goes on from where it was. The
  architecture is described in `docs/architecture.md`.
- The Event Center keeps a journal: every event it showed, with what you did with it (seen,
  dismissed) and when it settled, kept a month; what you saw and dismissed outlives a restart.
- A board: a kanban board of cards in the window, `b1`, one per project, kept in the desk's
  database and written out to `.localix/board.json`. The agent plans cards (`add`), takes one into Doing (`take`), hands it in
  with a note of what was done (`review`), or blocks it with what it waits for; the person drags
  cards, comments, edits, accepts a card into Done or sends it back to the top of Todo with what is
  wrong, and that reaches the agent as comments do. Each worker has at most one card in Doing, and
  only the person moves a card to Done. The board is a kind of tab: `open` with `kind` board,
  `interact` with an `op`; no new command.
- Backlog, a column before Todo: a card the person writes down lands there, and the agent puts an
  idea there with `add` and `"to": "backlog"`; nobody takes a card from it, and the person moving one
  to Todo makes it work.
- Subagents on the board: `who` names the worker, and in Claude Code the plugin sets it from the
  subagent's task, so a subagent acts on its own cards alone. The plugin adds a `worker` agent that
  works one card and a `board` skill; a card sent back that a running subagent had goes to it; a
  subagent that stops with its card in Doing is asked once to hand it in, release it or block it,
  and the main agent hears of it if it stops again (the main agent too, with the plugin's
  `finish_cards` setting); a session starts knowing the board; the line
  above the prompt shows who has which card and what waits for review.
- Terminals are on the window's glass too, as the board is (a solid ground where transparency is
  reduced, and off macOS).
- A clear page on the window's glass (the board, a terminal) is pictured on the window's background in the
  mini preview.
- The window's bar and side keep the window's own light or dark look over every tab: a dark page
  no longer paints them its colour. They follow the system's appearance, or the theme Settings
  name, when it changes.
- **What an agent sees of a page is its own.** The outline of a page no longer lists the window's
  other tabs (`- tabs: ... (this is wN)`), which were other projects' too: `list` names an agent's
  tabs. A page is opened in the window as it is (the old `show` override of one call that closed a
  window open the other way and opened it again is gone).
- **`reopen` follows the project the person last went to**: a project an agent only called is not
  marked as used; what the window shows is.
- **Calls that name what they do not take are refused, with the reason, rather than dropped.** In a
  `tabs` entry of `open`, a field no call has (a typo in `command`) is refused with nothing opened.
  On the CLI, `--follow` and `--json` without `--said`, a JSON argument with `--end`, `--said` or
  `--board`, `status` or `quit` with any argument, `llx-desk secret NAME` typed on a terminal, and
  `--as` or `--project` followed by another option, exit with code 3 and say why. `--said --json`
  still follows, as before.

## 0.1.5

- Page sizes: the size button in the window's toolbar frames the page shown as a device (iPhone SE,
  iPhone 15 Pro, Pixel 8, iPad Mini, Laptop, Desktop), in the middle of the window, or lets it fill
  the window again; Rotate turns a size. Each tab keeps its own. The agent's `size` takes the same
  names, `mobile`, `tablet`, `WIDTHxHEIGHT` or `window`, and frames the page instead of resizing the
  person's window. A size larger than the window is drawn smaller to fit it, as a browser's device
  view does, and the page still lays out at that size; screenshots of it are cut right. `desktop`
  is now the Desktop size, 1440 × 900.
- The agent no longer sizes the person's window: the `resize` action is gone (it could push the
  window past the screen's edge). A call to it says to give the page a `size` instead.
- Comments work in a modal dialog (a cart, a sign-in): the card opened under the dialog, where the
  page's inert rest took no typing. It now opens inside the dialog, on top of it, and Escape in
  it closes the card alone; a mark of a part of a dialog that closed goes with it.
- A comment comes to Claude Code as one message: its picture's path came as a turn of its own.
  `window --said --follow` writes each note at once, and `--json` prints each one as a JSON string
  on a line of its own, which the plugin reads whole.
- A narrow window keeps its toolbar in order: the address field gives way first, then the buttons
  that no longer fit go into "»", the system's menu of them, as Safari's do. They no longer slide
  over the address field.

## 0.1.4

- The mini no longer goes missing on the desktop after a full-screen app leaves full screen and
  comes back: macOS left it on that app's Space alone. Going to another Space now puts it on
  every Space again.
- A closed window is let go of: each one stayed in the window server, invisible, for the rest of
  the daemon's life. AppKit's own observers of the window, from before it became a panel, made
  its teardown stop halfway (and the log say an autorelease pool was drained twice).

## 0.1.3

- Closing the window takes a moment again, not seconds: the window's own thread no longer waits
  on itself for its list of tabs. The pages of a closed window are now forgotten too, and their
  owners hear `tab/closed`.

## 0.1.2

- While the person comments on the pages, the agent's actions that would move a page (a click,
  typing, keys, a load, a goal's steps) wait for them, up to 60 s, and then answer that the person
  is commenting. Scripts are not held. Send, turning comments off, or collapsing the window gives the
  turn back; Send now ends commenting too.
- The strip says whose turn it is, in quiet words at the end of the address field: "Agent is
  working", "Paused while you comment", "Agent is waiting for you".
- A new look for the strip: monochrome, buttons in shared capsules, thin icons drawn as SVG instead
  of text characters, the address field always centred, larger project pills that count their tabs,
  and tabs in the same style.
- The Claude Code plugin hands the OpenRouter key from its settings to the MCP server, so `goal`
  works with a key given only there. An unfilled setting is no key.

## 0.1.1

- Linux: a terminal draws as soon as it opens and after it is shown again. It showed one character
  until the window was resized: xterm paused while hidden and never heard it was back in sight.
- A page's `keys` type into the focused field: a character goes in at the caret with the `input` the
  page hears, Backspace and Delete erase, Ctrl+A selects all. They reported keys pressed and typed
  nothing.
- An argument of the wrong type is refused with what it takes (`"read": "h1"`), not read as absent.
- The Claude Code plugin asks for the OpenRouter key when it is enabled: optional, kept in the system
  keychain, and needed only for `goal`. A key in the environment still comes first.
- Linux: the history and "+" are GTK's own menus, under the button and over the page. The history
  list drawn in the strip took the pages' place there, and closing it left the window blank; "+"
  had no menu and always opened a page. The address field's suggestions no longer take the window
  on Linux either.
- The daemon's `daemon/stopping` reaches every subscriber before it exits.

## 0.1.0

- Release builds for macOS (Apple silicon and Intel) and Linux x86_64, from a tag, each archive listed
  in the release's `SHA256SUMS`. `install.sh` installs the latest in one line; the Claude Code plugin
  runs a program of its own version, downloading the release build the first time when none is on
  `PATH`.
- The tabs' row reads on a light page while the Mac is in the dark appearance: it takes the ink of the
  page under the strip, as the toolbar and the projects did; it was light words on a light bar.
- A page's native dialogs never block it and are never agreed to on its say-so: `alert` is noted,
  `confirm` and `prompt` are answered Cancel unless the call says `dialog: "accept"` (`dialog_text` for a
  prompt), in a goal too. What a page tells besides its content follows the call's report under "The
  page said", and reaches a goal's decision model: a dialog and its answer, words announced in a live
  region that may be gone by the next look (a "Saved" toast, each time it shows), a field that did not
  keep what was typed. A live region with controls in it (a feed marked `aria-live`) is content, not an
  announcement.
- A field the page refused reads `[invalid: why]` in the snapshot: `aria-invalid`, or a form the browser
  would not send for it (its own bubble is never seen in the window), with the page's message.
- An action waits for the requests it started (a search's results, a save) up to 2 s, instead of
  returning while they are out; a request repeated at a steady pace (a poll) no longer keeps a page busy,
  and the page's own site is its registrable domain (`api.shop.com` is `www.shop.com`'s).
- A click on a control an overlay still covers, or one not enabled yet, is tried again for 0.6 s before
  it is refused.
- Typing reads the field back: one whose script rewrote or refused the value (an input mask, a box that
  reads keys) is typed again a key at a time. A date field takes `Nov 5, 2026` or `05.11.2026` as
  `2026-11-05`. Zero-width characters inside words are dropped from what is read.
- A goal pays, orders or deletes only when its own words ask for it: such controls are held back
  otherwise, and the report says so, one line per reason. Only what the goal asks to do counts, not
  how to tell it is done ("done when the order shows" asks for no order), nor what it says not to do
  ("without booking"). A control is read by its verb ("Edit Buy oat milk" edits). It never clicks the
  same paying control twice once it acted, nor one an earlier goal in the tab clicked, unless the goal
  asks to do it again; the report names each step that may have paid, deleted or sent something.
  Enter in a field sends its form, so it is held back with the form's paying or deleting control,
  and counted as clicking it when it is not.
- A goal no longer offers to type the values it already typed into a password field (it reads as dots)
  again, nor trades them between fields; a field the page refused is offered its other values.
- The helper's report ends with what the model was asked: `Model: 5 decisions, $0.0006, 1.9 s.`
- Secrets: `llx-desk secret NAME < FILE` saves one (0600); a goal's `values` and a terminal's `input`
  name it as `{{secret:NAME}}`. The daemon types the value, and every answer shows `{{secret:NAME}}`
  where it would be, as typed, in an address, in JSON or in base64; a goal's decision model never sees
  it. What a goal's model is sent has credential shapes masked (provider keys, bearer tokens, a
  password in an address, a value assigned to a credential's name).
- The expanded window collapses to the mini when the person clicks in another app (macOS): the
  window never takes the app's focus, so a click elsewhere is what losing it means.
  `LLX_DESK_KEEP_EXPANDED=1` keeps it open beside their work.
- A benchmark of the goal helper (`bench/`): tasks on the demo sites run through the desk in a real
  window, scored by a check of the page's state (`51/54`), with steps, requests, cost and time, one row
  per run in `bench/SCORES.md` and each run in `bench/runs/`. Paid, so only with `LLX_DESK_BENCH=1`.
- A goal's helper presses keys (`PRESS_KEY`, from the one list a call's `keys` take, `view::KEYS`) and goes
  to an address among its `values` (`GO_TO`), through the same paths as a call's `keys` and `url`; on a
  terminal it presses keys too. It stops on the third try in a row of an action that changes nothing it
  can read, and chooses BLOCKED for an error its values cannot fix.
- `llx-desk key < FILE` saves the OpenRouter key a goal needs (0600), read when `OPENROUTER_API_KEY` is
  not set: an agent's host often starts the MCP server without the person's shell variables.
- The MCP server takes over a new build of itself while idle (`exec` on the same pipes) and tells the host
  `notifications/tools/list_changed`; a build that does not answer `--version` is refused.
- Pages the window no longer has (the person closed it) are forgotten, with `tab/closed`, instead of being
  listed with no page behind them.
- A terminal's `wait_for` waits for the program's output, not the echo of the typed input.
- `ArrowUp` and the other names a page takes are terminal keys too.
- One daemon per user: the lock and the socket are at a fixed per-user place that `LLX_DESK_HOME` and
  `TMPDIR` do not move (`LLX_DESK_HOME` now places only the log, screenshots and comments). A second
  `serve` exits 0 and prints nothing. `LLX_DESK_NO_WINDOW` is honoured by `serve`.
- Protocol v2 between the clients and the daemon: JSON-RPC 2.0 with `initialize`, ids and pipelined calls,
  an idempotency key per call, notifications by subscription, a screenshot as bytes on the connection, a
  request's panic as that request's error. The CLI is a v2 client; `window --said --follow` subscribes to
  comments instead of polling, and a comment is kept until acknowledged. An older daemon is reported, and
  `serve stop` still ends it.
- The owner of a `window` call from the command line is now found from `CLAUDE_PROJECT_DIR` when Claude
  Code sets it (as the MCP server does), else from the current directory, so both reach the same tabs.
- `llx-desk mcp`: a stdio MCP server with the tools `open`, `act`, `look`, `screenshot`, `tabs` and
  `comments`, one persistent connection to the daemon that is pinged, reconnected and subscribed again by
  itself, calls asked again with their idempotency key, a goal's steps as progress. The Claude Code plugin
  starts it (`.mcp.json`), and its skill prefers the tools, the command line being the fallback.
- Initial release.
