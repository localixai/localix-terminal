# Contributing

Issues and pull requests are welcome. Keep a change small and focused.

```sh
cargo build
cargo clippy --all-targets -- -D warnings
cargo test                                   # unit tests, and the CLI against a server with no window
bun test plugins/claude-code/hooks/register.spec.ts  # plugin hooks with a mocked host; Bun 1.3+
bun test src/browser src/editor                    # UI gestures, frame cadence, motion, the editor's page
bunx tsc -p plugins/claude-code                    # the plugin's types (see below)
shellcheck install.sh scripts/*.sh plugins/claude-code/bin/llxterm
cargo deny --locked check                          # advisories, licences and sources (deny.toml)
LLXTERM_DISPLAY_TESTS=1 cargo test --test browser   # real windows: needs a display, and shows some
LLXTERM_DISPLAY_TESTS=1 cargo test --test browser -- panel carried   # only the scenes whose names hold the word
```

- The plugin's type check needs the `claude-code` module's declarations, which Claude Code writes
  to `plugins/claude-code/.claude-plugin/types/` (ignored by git) when it loads the plugin from the
  folder: run `claude --plugin-dir plugins/claude-code` once. CI cannot get them, so it does not run
  this check; run it before you change the hooks.

- `cargo test` builds a test build (the `test-server` feature, turned on only by the crate's own
  dev-dependency): its server never opens a window and keeps its lock and socket in `LLXTERM_TEST_DIR`,
  so tests never reach your own server. Do not run a test build's binary by hand without that variable.
- `LLXTERM_RECORD=DIR` (set where the app or `serve` starts) records each terminal's raw output and
  resizes into `DIR` as asciicast v2, one file per terminal, readable by you alone. It is for
  development: a recording holds whatever the programs wrote, secrets too. The recordings in
  `tests/fixtures/terminal/` were made so, in a scratch home with nothing of anyone's, and read by
  hand before they were kept; `tests/core_screens.rs` feeds each to the terminal core and holds
  its last screen to the one kept beside it, `*.screen` (`LLXTERM_WRITE_SCREENS=1` writes them
  anew, to be read before they are committed).
- The terminal core is libghostty-vt, linked statically through `ghostty-vt-sys`, which needs
  Rust 1.96 (`rust-version`). On macOS its build script fetches the release's prebuilt library
  and holds it to the sha256 the crate pins; on Linux it builds the library from the release's
  source with Zig 0.17 (`scripts/install-zig.sh` installs the pinned one, checked by its
  sha256, as CI does). To build offline, point `GHOSTTY_RELEASE_DIR` at a folder that holds the
  release's files (each checked against its pinned sha256 as if fetched; Linux still builds them
  with Zig), or `GHOSTTY_VT_LIB_DIR`
  at a folder with a `libghostty-vt.a` built from the same release, which needs no Zig at all
  (`GHOSTTY_VT_LIB_SHA256` checks it).
- Read [ARCHITECTURE.md](ARCHITECTURE.md) first; its rules are checked by the lints and
  `tests/architecture.rs` in the normal `cargo test` run.
- Display tests also check split proportions, sidebar row identity and IME input in the real
  WebView. Resize tests use a deterministic frame clock and do not need a display.
- Write the test first: a bug gets a test that fails for the right reason, a feature one written from
  what a person or an agent sees.
- Every Rust source file under `src/`, including unit tests, opens with a `//!` line and stays
  under 300 nonblank, non-comment lines. Split by responsibility, not by line count; keep a
  module's data, its pure rules and its adapters separate. `cargo test --test architecture`
  checks the limit, headers and domain-to-adapter boundaries.
- SQL migrations in `src/store/schema.rs` are append-only. A regression test fingerprints the
  existing steps: append a new step instead of changing an existing step or its fingerprint.
- Update `README.md` and `CHANGELOG.md` (under `## Unreleased`) with any change of behaviour.
- A change a person sees follows [docs/ui.md](docs/ui.md) in every state of its matrix.
- English only, in code, comments and docs.

## Releasing

1. Set the new version in `Cargo.toml` and `plugins/claude-code/.claude-plugin/plugin.json`;
   `scripts/check-versions.sh` says when they differ.
2. Rename `## Unreleased` in `CHANGELOG.md` to `## 0.2.0` (the version) and open a new `## Unreleased`
   above it; that section is the release's notes.
3. Commit, then `git tag v0.2.0 && git push origin main v0.2.0`. The Release workflow first checks that
   the tag is on main and that CI passed for it. Then it builds Linux on the project's server and the
   Mac app on GitHub's Mac runner, signed and notarized with the project's Developer ID: one disk image
   (`Localix-Terminal-macos.dmg`) and one zip (`Localix-Terminal-macos.zip`, which `install.sh` fetches), for Apple
   Silicon only. It publishes them with the Linux archive and `SHA256SUMS`, each with a build
   provenance attestation (`gh attestation verify FILE -R localixai/localix-terminal`).

Releases build with the exact Rust that `RUSTUP_TOOLCHAIN` names in `release.yml`; move it to a
newer stable once CI passed on it. `TEAM_ID` in `install.sh` is the Apple team that signs the app:
the Release workflow fails when it names another team, and warns while it is empty.

`scripts/dmg.sh [--universal] [--notarize]` makes the same Mac files here (ad hoc signed unless you
have a Developer ID identity; `--notarize` needs `LLX_NOTARY_PROFILE`, see the script's header).
`--universal` is optional: it joins an Apple Silicon and an Intel program, but releases are Apple
Silicon only and do not use it.
Running Release by hand from the Actions tab is a dry run: it builds, signs and notarizes the same
files and publishes nothing; they stay on the run as artifacts. The Apple secrets live in the
`release` environment of the repository, and only the macos job uses them.

`scripts/e2e.sh [--update] [dark|light]` is the end-to-end run, at a Mac: it builds this tree,
starts its app beside yours (its own socket, data and settings in a scratch folder, so your app
keeps running), and uses it on the demo project in
`scripts/e2e/project` as a person and an agent do: a board worked by two agents, an agent in a
terminal, a file, a page, the Store and Settings, in dark and in light. Each step is checked by
what the `window` tool reports, what is written to the project and what the window offers to
assistive tools, and each scene is pictured into `target/e2e`. Run it before a release and after
a change a person sees, and look at the pictures; `--update` also copies the README's
(`assets/*-dark.png`: what is shown of the app, in the README and demos, is dark only), to be
committed. It hides the other apps while it runs, so leave the Mac alone for the two minutes it
takes.

The workflows run on pushes, tags and by hand only, never on a pull request: the Linux jobs run on the
project's self-hosted runners, which a fork's code must never reach.
