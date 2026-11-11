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
- **The accent is for the one primary action of a place** (Accept, a sheet's default button) and
  for selection and focus. Never for a hover button, a decoration or a second action.
- **No round blobs, pills with text, drop shadows or brand colours** on controls. One shape: the
  token radius (`--r-1`, `--r-2`). One glyph style: the line icons the strip and board draw.
- **Words in a control are for push buttons** (Accept, Send Back, Comment) and menus, in title
  case. A menu that the button opens says so with `aria-haspopup`.
- **An empty place is one quiet line** of what would be there, no glyph, frame, picture or
  motion; only the window with no tab offers its first commands with their keys
  ([docs/ui.md](docs/ui.md), Words).
- **One look per kind of control**: a new button reuses an existing class (`.fold`, `.quick`,
  `.btn.plain.ico`, `.btn.go`) rather than a variant of its own.

## Seeing a change

The person watches the window on their own screen.

- For "how does it look", ask them, or use `llx-desk window` (`screenshot` of a tab).
- Don't build capture harnesses (screencapture or Swift scripts). They cost more than the question.

## Running a build on this machine

The app is the server: `~/.local/bin/llx-desk` links into `~/Applications/LLX Desk.app`.

1. Build and bundle: `cargo build --release && scripts/bundle-macos.sh`.
2. Replace the bundle whole. `ditto` it to `LLX Desk.app.new`, then move that over the old one.
   Never copy over the link or the binary.
3. Restart: run `llx-desk quit`, then `open -g -a ~/Applications/"LLX Desk.app" --args serve`.
4. Check that `llx-desk status` names the new build. Another session may start an older app after
   the quit. If it does, open the new one again: it takes over.
