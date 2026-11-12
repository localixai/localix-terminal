#!/bin/sh
# Builds Localix Terminal.app around a release build: the program as the app's executable, its icon and
# Info.plist (packaging/macos), the version Cargo.toml says, and the Claude Code plugin as a
# marketplace of its own (Contents/Resources/claude-code), which Settings → Integrations installs
# from (src/setup/claude.rs). The app is all there is to install: opened, it is the server and
# links `llxterm` into ~/.local/bin (src/cli/app.rs); the CLI and the app are the one program.
#   scripts/bundle-macos.sh [target/release/llxterm] [out-dir]
# Signed ad hoc, so the system runs it here, unless LLXTERM_SIGN names an identity
# ("Developer ID Application: …"): then it is signed with that, with a secure timestamp, and a
# signature that fails is an error, never a quiet fall back to ad hoc. Notarization is not done
# here: it is scripts/dmg.sh.
set -eu
cd "$(dirname "$0")/.."
binary=${1:-target/release/llxterm}
out=${2:-target/release/bundle/macos}
version=$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -n 1)
[ -x "$binary" ] || { echo "no program at $binary: cargo build --release first" >&2; exit 1; }
# The plugin is the app's: one version for both, or the app would call its plugin old.
scripts/check-versions.sh >/dev/null

app="$out/Localix Terminal.app"
stage="$out/.Localix Terminal.app.new"
mkdir -p "$out"
[ ! -e "$stage" ] || mv "$stage" "$out/.Localix Terminal.app.old.$$"
mkdir -p "$stage/Contents/MacOS" "$stage/Contents/Resources"
sed "s/@VERSION@/$version/g" packaging/macos/Info.plist > "$stage/Contents/Info.plist"
plutil -lint "$stage/Contents/Info.plist" >/dev/null
cp "$binary" "$stage/Contents/MacOS/llxterm"
chmod 0755 "$stage/Contents/MacOS/llxterm"
cp packaging/macos/llxterm.icns "$stage/Contents/Resources/llxterm.icns"
market="$stage/Contents/Resources/claude-code"
mkdir -p "$market/.claude-plugin"
rsync -a --exclude .DS_Store --exclude node_modules plugins/claude-code/ "$market/plugin/"
sed 's#"\./plugins/claude-code"#"./plugin"#' .claude-plugin/marketplace.json > "$market/.claude-plugin/marketplace.json"
grep -q '"./plugin"' "$market/.claude-plugin/marketplace.json" || { echo "the marketplace does not point at its plugin" >&2; exit 1; }
printf 'APPL????' > "$stage/Contents/PkgInfo"
# The one signature of the bundle covers its program and its resources, but a Mach-O file among the
# resources would not be signed as code: it has to be signed before the bundle, so it stops here.
if find "$stage/Contents/Resources" -type f -exec file {} + | grep -q 'Mach-O'; then
  echo "a Mach-O file in Contents/Resources: sign it before the bundle" >&2
  exit 1
fi
sign=${LLXTERM_SIGN:--}
sign_once() {
  if [ "$sign" = - ]; then
    codesign --force --sign - --options runtime --entitlements packaging/macos/llxterm.entitlements "$stage"
  else
    codesign --force --sign "$sign" --options runtime --entitlements packaging/macos/llxterm.entitlements --timestamp "$stage"
  fi
}
# Apple's timestamp server fails now and then: four tries, the wait doubling between them.
tries=0
delay=5
until sign_once; do
  tries=$((tries + 1))
  [ "$tries" -lt 4 ] || { echo "codesign failed" >&2; exit 1; }
  sleep "$delay"
  delay=$((delay * 2))
done
if [ "$sign" != - ]; then
  codesign --verify --deep --strict "$stage"
fi
# The new app replaces the old whole: what was there is moved aside, then let go.
if [ -e "$app" ]; then
  mv "$app" "$out/.Localix Terminal.app.old.$$"
fi
mv "$stage" "$app"
find "$out" -maxdepth 1 -name '.Localix Terminal.app.old.*' -exec rm -rf {} +
echo "$app"
