# Working on llx-desk

How every agent works in this repository. These rules hold for every change, small or large.

## Read first

- [ARCHITECTURE.md](ARCHITECTURE.md): the module map and its rules. The lints and
  `tests/architecture.rs` check them.
- [CONTRIBUTING.md](CONTRIBUTING.md): build, lint and test commands, and what a change carries
  (README, CHANGELOG).
- [docs/architecture.md](docs/architecture.md), [docs/windows.md](docs/windows.md) and
  [docs/projects.md](docs/projects.md): owners, the change feed, and the window's hierarchy.
- [docs/ui.md](docs/ui.md): how the window looks, moves and reads. It applies to every change a
  person sees: native AppKit code, the strip, the side, menus, Settings and notices.

## Always

- **One mechanism, never copies.** Extend what exists: the one change feed, the one `window`
  surface and its kinds, the one shortcut table, the one token sheet (`frame.css`). Never add a
  parallel path, tool, list or stylesheet that would drift from the first.
- **Test first.** A bug gets a test that fails for the right reason. A feature gets a test written
  from what a person or an agent sees.
- **Each file opens with a `//!`** that says what it is, and holds at most 300 lines of working
  code. Split it by responsibility.
- **Comments are prose.** They say what a thing is and why, in plain words. They don't narrate the
  change that made it.
- **English only** in code, comments, docs and commits.
- **Never name outside projects** an idea was borrowed from, in code, docs or commits. Describe the
  pattern itself.
- **Every visible change follows [docs/ui.md](docs/ui.md)**, in each state it lists:
  - light and dark;
  - glass and solid (Off, Reduce Transparency);
  - windowed and full screen;
  - key and inactive;
  - Increase Contrast and Reduce Motion.
- **Behaviour changes** update `README.md` and `CHANGELOG.md` (under `## Unreleased`).
- **Commit only when asked.** Never push without being asked.

## Style: quiet, native, nothing loud

The window is a calm macOS tool, not a web app. Every control earns its place by being quiet.

- **A secondary action is an icon alone**: a line glyph in `--ink-2`, no fill, no border, no
  shadow at rest; pointed at, a grey fill (`--ov-2`) and the text colour; pressed, `--ov-3`. Its
  words are its tooltip and `aria-label`. A column's fold button and a card's hover move are the
  model.
- **A control that is only for pointing shows when its card or row is pointed at**, and takes the
  place of what it covers (a card's age); it never sits over the content.
- **The accent is for the one primary action of a place** (Accept, a dialog's default button) and
  for selection and focus. Never for a hover button, a decoration or a second action.
- **No round blobs, pills with text, drop shadows or brand colours** on controls. One shape: the
  token radius (`--r-1`, `--r-2`). One glyph style: the line icons the strip and board draw.
- **Words in a control are for push buttons** (Accept, Send Back, Comment) and menus, in title
  case. A menu that the button opens says so with `aria-haspopup`.
- **An empty place is one quiet line** of what would be there, no glyph, frame, picture or
  motion; only the window with no tab offers its first commands with their keys, and under them
  the marks of the agents installed ([docs/ui.md](docs/ui.md), Words).
- **One look per kind of control**: a new button reuses an existing class (`.fold`, `.quick`,
  `.btn.plain.ico`, `.btn.go`) rather than a variant of its own.

## Laying a thing out

Most of what a person sends back is a detail of placement. Settle each of these before the first
build, not after the first screenshot.

- **Reuse before drawing.** Look for the primitive first: the tokens, `.btn` and the text field
  (`.field`) in `frame.css`, the icon button, the dialog (`dialog.rs`, `dialog.css`,
  `dialog.js`), the offer row, a menu's row. A page never defines its own copy of a shared
  control.
- **A page never draws a dialog, a sheet, a veil or a menu of its own.** A tab's kind asks the
  person through `kind::Tab::ask` (a title, a sentence, points, fields to type into), which is
  the window's one dialog; a menu is the system's (`m` and its entries, as the board's page
  asks). If the primitive lacks what the question needs, extend the primitive for every
  caller; do not build beside it.
- **A page's script starts from the pages' kit** (`src/page/kit.js`: `post`, `act`, `el`,
  `svgPath`, `systemMenu`, `menuUnder`, `chosen`) and its module from `page.rs` (`escaped`,
  `json`, the hash check `digest`). A helper two pages need goes into the kit; neither page
  keeps a copy.
- **A row that holds more than fits scrolls sideways** (`across` in `src/page/across.js`,
  `.across` in `frame.css`): no bar of its own, an edge with more past it fades, the chosen
  one stays in view. It never wraps, narrows its items or puts the rest under a "More" menu.
- **Before writing any control's CSS or markup, search for it**: `grep` `frame.css`, the
  `src/browser/*.css` sheets and the other pages for the class or the look. Say in the
  hand-back which primitive each new control reuses, and why where one was extended.
- **A thing takes the size of what it holds.** A card or a dialog has a fixed width and a
  greatest height; what is long scrolls inside it, and its title and buttons stay put. Nothing
  stretches to the window's height or width because it can.
- **Align to what is already there.** A row under a centred column is centred under it. A glyph
  lines up with the text above it, not with its own button's edge. Lefts, rights and baselines
  are shared with a neighbour, never close to one.
- **Space on the 4-point steps** the tokens use, the same gap between like things, and more
  space between groups than within one.
- **One accent, no ornament.** A glow, a wash of colour, particles, a pulsing ring or a second
  coloured element is decoration: leave it out. Colour comes from the tokens, so it follows the
  theme and the inactive window as the system's own controls do.
- **One thing asks at a time.** What announces something does not show beside another surface
  that says the same (a dialog and its offer row), and what comes unasked does not take the
  keyboard: a key meant for a terminal must not press a button.
- **What is under an overlay is out of reach**: it takes no click, no key and no pointer (its
  cursor must not show through), and all of it comes back as the overlay shuts.
- **Every control has every state**: at rest, pointed at, pressed, focused by the keyboard, off;
  a name for assistive tools; and Escape and Return where a person expects them.

## Seeing a change

Look at it yourself before saying it is done. A visible change that was never seen on the
screen is not finished, and the person should not be the first to see it.

- **Capture the window.** Bring the app to the front (`open -a ~/Applications/"LLX Desk.app"`),
  read the window's place from System Events, and `screencapture -x -R x,y,w,h` it to the
  scratch directory. If another app is in front, the capture shows that app: check which process
  is frontmost, and read the picture before trusting it. `llx-desk window screenshot` captures a
  tab's page, not the window's own parts.
- **Measure what a picture cannot settle.** `agent-desktop snapshot --app "LLX Desk" -i
  --compact --include-bounds` gives each control's name, role and rectangle: use it to check
  that a thing is centred, aligned, the size meant, and named for assistive tools. Its own
  `screenshot` does not work on this machine; its `click` does.
- **Use what is there**: these two tools and the end-to-end run (`scripts/e2e.sh`, which
  starts this tree's app beside the person's and pictures each scene in dark and light into
  `target/e2e`), nothing built for the occasion. No Swift scripts, no second capture harness.
  A scene a change adds is a step of that run, with its check.
- **Walk the states** the change can be in, at least dark and light, key and inactive, and
  pointed at; say plainly which of the listed states were not looked at.
- **A system prompt is the person's to answer** (a folder's access, a permission). Never press
  it; say that it is up and what each answer does.
- **A temporary switch** made to show a state (a preview of an update, a forced stage) is
  named in the hand-back while it exists and is removed before a commit.

## What the app must not assume

The app is started by the Finder, the Dock, a login item or `serve`, with almost no environment.
Each of these has already cost a round.

- **The `PATH` is the system's alone.** Whatever the app starts for the person (an agent in a
  tab, a card's worker, a helper) is looked for where their login shell looks
  (`browser::commands_path`), never on the process's own `PATH`.
- **Starting the app touches nothing of the person's unasked**: no read under Documents,
  Desktop or Downloads, and no other program run that might read there. macOS asks the person
  each time, and again after every rebuild, since a new signature is a new app to it. Read the
  app's own records and the agents' config files instead, and start a program only to repair.
- **Both ways in do the same.** What happens when the app is opened must also happen under
  `serve`, and the other way round; check the one that was not used while developing.
- **Nothing waits for a chance redraw.** What must be shown is shown when its cause happens,
  with the window in front or behind.
- **A debug build never rewrites the person's setup** (the agents' entries, the command's
  link).

## A change is done when

One pass, and the person has nothing to send back for a detail that could have been seen.

1. The test written first passes, with `cargo fmt --check`, `cargo clippy --all-targets -- -D
   warnings` and `cargo test` clean, and the plugin's `bun test` where the plugin changed.
2. What a person sees was looked at in the running app (above), measured where placement
   matters, in the states it has.
3. It was used as a person would use it: the click, the key, the wrong input, the second time.
4. `README.md`, `CHANGELOG.md` and the doc that states the rule (`docs/ui.md`,
   `docs/windows.md`, `ARCHITECTURE.md`) say what is now true.
5. Nothing temporary is left: no preview switch, no debug line, no tab or terminal opened for
   the check.
6. The hand-back says what was verified and how, what was not, and what is left uncommitted.

## Running a build on this machine

The app is the server: `~/.local/bin/llx-desk` links into `~/Applications/LLX Desk.app`.

1. Build and bundle: `cargo build --release && scripts/bundle-macos.sh`.
2. Replace the bundle whole. `ditto` it to `LLX Desk.app.new`, then move that over the old one.
   Never copy over the link or the binary.
3. Restart: run `llx-desk quit`, wait until the old process is gone (`pgrep -f "LLX
   Desk.app/Contents/MacOS"`; an `open` sent sooner fails with error -600), then
   `env -i HOME="$HOME" USER="$USER" PATH=/usr/bin:/bin open -g -a ~/Applications/"LLX Desk.app" --args serve`.
   The bare environment matters: an app opened from an agent's shell hands that session's
   variables to every terminal the person opens in it. Under `serve` the window opens when the
   app is brought forward: `open -a ~/Applications/"LLX Desk.app"`.
4. Check that `llx-desk status` names the new build. Another session may start an older app after
   the quit. If it does, open the new one again: it takes over.
