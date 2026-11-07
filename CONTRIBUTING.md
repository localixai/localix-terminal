# Contributing

Issues and pull requests are welcome. Keep a change small and focused.

```sh
cargo build
cargo clippy --all-targets -- -D warnings
cargo test                                   # unit tests, and the CLI against a server with no window
LLX_DESK_DISPLAY_TESTS=1 cargo test --test browser   # real windows: needs a display, and shows some
```

- `cargo test` builds a test build (the `test-server` feature, turned on only by the crate's own
  dev-dependency): its server never opens a window and keeps its lock and socket in `LLX_DESK_TEST_DIR`,
  so tests never reach your own server. Do not run a test build's binary by hand without that variable.
- Read [ARCHITECTURE.md](ARCHITECTURE.md) first; its rules are checked by the lints.
- Write the test first: a bug gets a test that fails for the right reason, a feature one written from
  what a person or an agent sees.
- Every file opens with a `//!` line and stays under 300 lines of working code.
- Update `README.md` and `CHANGELOG.md` (under `## Unreleased`) with any change of behaviour.
- English only, in code, comments and docs.

## Releasing

1. Set the new version in `Cargo.toml`, `plugins/claude-code/.claude-plugin/plugin.json`
   and `.claude-plugin/marketplace.json`; `scripts/check-versions.sh` says when they differ.
2. Rename `## Unreleased` in `CHANGELOG.md` to `## 0.2.0` (the version) and open a new `## Unreleased`
   above it; that section is the release's notes.
3. Commit, then `git tag v0.2.0 && git push origin main v0.2.0`. The Release workflow first checks that
   the tag is on main and that CI passed for it. Then it builds Linux on the project's server and the
   Mac app on GitHub's Mac runner, signed and notarized with the project's Developer ID: one disk image
   (`LLX-Desk-macos.dmg`) and one zip (`LLX-Desk-macos.zip`, which `install.sh` fetches), each for both
   Macs. It publishes them with the Linux archive and `SHA256SUMS`.

`scripts/dmg.sh [--universal] [--notarize]` makes the same Mac files here (ad hoc signed unless you
have a Developer ID identity; `--notarize` needs `LLX_NOTARY_PROFILE`, see the script's header).
Running Release by hand from the Actions tab is a dry run: it builds, signs and notarizes the same
files and publishes nothing; they stay on the run as artifacts. The Apple secrets live in the
`release` environment of the repository, and only the macos job uses them.

The workflows run on pushes, tags and by hand only, never on a pull request: the Linux jobs run on the
project's self-hosted runners, which a fork's code must never reach.
