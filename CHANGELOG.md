# Changelog

## Unreleased

- Page sizes: the size button in the window's toolbar frames the page shown as a device (iPhone SE,
  iPhone 15 Pro, Pixel 8, iPad Mini, Laptop, Desktop), in the middle of the window, or lets it fill
  the window again; Rotate turns a size. Each tab keeps its own. The agent's `size` takes the same
  names, `mobile`, `tablet`, `WIDTHxHEIGHT` or `window`, frames the page instead of resizing the
  person's window, and says the size the page has (smaller when the window is). `desktop` is now
  the Desktop size, 1440 × 900.
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
