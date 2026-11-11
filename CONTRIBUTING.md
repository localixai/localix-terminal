# Contributing

Issues and pull requests are welcome. Keep a change small and focused.

```sh
cargo build
cargo clippy --all-targets -- -D warnings
cargo test                                   # unit tests, and the CLI against a server with no window
bun test plugins/claude-code/hooks/register.spec.ts  # plugin hooks with a mocked host; Bun 1.3+
bun test src/browser                               # UI gestures, frame cadence and motion lifecycle
bunx tsc -p plugins/claude-code                    # the plugin's types (see below)
shellcheck install.sh scripts/*.sh plugins/claude-code/bin/llx-desk
cargo deny --locked check                          # advisories, licences and sources (deny.toml)
LLX_DESK_DISPLAY_TESTS=1 cargo test --test browser   # real windows: needs a display, and shows some
```

- The plugin's type check needs the `claude-code` module's declarations, which Claude Code writes
  to `plugins/claude-code/.claude-plugin/types/` (ignored by git) when it loads the plugin from the
  folder: run `claude --plugin-dir plugins/claude-code` once. CI cannot get them, so it does not run
  this check; run it before you change the hooks.

- `cargo test` builds a test build (the `test-server` feature, turned on only by the crate's own
  dev-dependency): its server never opens a window and keeps its lock and socket in `LLX_DESK_TEST_DIR`,
  so tests never reach your own server. Do not run a test build's binary by hand without that variable.
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

1. Set the new version in `Cargo.toml`, `plugins/claude-code/.claude-plugin/plugin.json`
   and `.claude-plugin/marketplace.json`; `scripts/check-versions.sh` says when they differ.
2. Rename `## Unreleased` in `CHANGELOG.md` to `## 0.2.0` (the version) and open a new `## Unreleased`
   above it; that section is the release's notes.
3. Commit, then `git tag v0.2.0 && git push origin main v0.2.0`. The Release workflow first checks that
   the tag is on main and that CI passed for it. Then it builds Linux on the project's server and the
   Mac app on GitHub's Mac runner, signed and notarized with the project's Developer ID: one disk image
   (`LLX-Desk-macos.dmg`) and one zip (`LLX-Desk-macos.zip`, which `install.sh` fetches), for Apple
   Silicon only. It publishes them with the Linux archive and `SHA256SUMS`, each with a build
   provenance attestation (`gh attestation verify FILE -R rsaryev/llx-desk`).

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

The workflows run on pushes, tags and by hand only, never on a pull request: the Linux jobs run on the
project's self-hosted runners, which a fork's code must never reach.
