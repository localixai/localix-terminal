#!/bin/sh
# Builds LLX Desk.app around a release build: the program as the app's executable, its icon and
# Info.plist (packaging/macos), the version Cargo.toml says, and the Claude Code plugin as a
# marketplace of its own (Contents/Resources/claude-code), which Settings → Integrations installs
# from (src/setup/claude.rs). The app is all there is to install: opened, it is the server and
# links `llx-desk` into ~/.local/bin (src/cli/app.rs); the CLI and the app are the one program.
#   scripts/bundle-macos.sh [target/release/llx-desk] [out-dir]
# Signed ad hoc, so the system runs it here; a release signs it with its own identity
# (LLX_DESK_SIGN="Developer ID Application: …") and notarizes it.
set -eu
cd "$(dirname "$0")/.."
binary=${1:-target/release/llx-desk}
out=${2:-target/release/bundle/macos}
version=$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -n 1)
[ -x "$binary" ] || { echo "no program at $binary: cargo build --release first" >&2; exit 1; }
# The plugin is the app's: one version for both, or the app would call its plugin old.
scripts/check-versions.sh >/dev/null

app="$out/LLX Desk.app"
stage="$out/.LLX Desk.app.new"
mkdir -p "$out"
[ ! -e "$stage" ] || mv "$stage" "$out/.LLX Desk.app.old.$$"
mkdir -p "$stage/Contents/MacOS" "$stage/Contents/Resources"
sed "s/@VERSION@/$version/g" packaging/macos/Info.plist > "$stage/Contents/Info.plist"
plutil -lint "$stage/Contents/Info.plist" >/dev/null
cp "$binary" "$stage/Contents/MacOS/llx-desk"
chmod 0755 "$stage/Contents/MacOS/llx-desk"
cp packaging/macos/llx-desk.icns "$stage/Contents/Resources/llx-desk.icns"
market="$stage/Contents/Resources/claude-code"
mkdir -p "$market/.claude-plugin"
rsync -a --exclude '.claude-plugin/types' --exclude node_modules plugins/claude-code/ "$market/plugin/"
sed 's#"\./plugins/claude-code"#"./plugin"#' .claude-plugin/marketplace.json > "$market/.claude-plugin/marketplace.json"
grep -q '"./plugin"' "$market/.claude-plugin/marketplace.json" || { echo "the marketplace does not point at its plugin" >&2; exit 1; }
printf 'APPL????' > "$stage/Contents/PkgInfo"
codesign --force --sign "${LLX_DESK_SIGN:--}" --options runtime --timestamp=none "$stage" >/dev/null 2>&1 ||
  codesign --force --sign - "$stage"
# The new app replaces the old whole: what was there is moved aside, then let go.
if [ -e "$app" ]; then
  mv "$app" "$out/.LLX Desk.app.old.$$"
fi
mv "$stage" "$app"
find "$out" -maxdepth 1 -name '.LLX Desk.app.old.*' -exec rm -rf {} +
echo "$app"
