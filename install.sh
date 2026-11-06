#!/bin/sh
# Installs LLX Desk, the app, which is all there is to install:
#   curl -fsSL https://raw.githubusercontent.com/rsaryev/llx-desk/main/install.sh | sh
# macOS: LLX Desk.app into ~/Applications. Linux: the program into ~/.local/share/llx-desk, the
# `llx-desk` command and the app's desktop entry put in place by the program itself
# (`llx-desk setup command`). LLX_DESK_VERSION=0.1.0 picks a release. The download's checksum is
# verified against the release's SHA256SUMS before anything is written.
set -eu

REPO=rsaryev/llx-desk

say() { printf 'llx-desk: %s\n' "$*" >&2; }

case "$(uname -s)/$(uname -m)" in
  Darwin/*) archive=LLX-Desk-macos.zip ;;
  Linux/x86_64) archive=llx-desk-x86_64-unknown-linux-gnu.tar.gz ;;
  *)
    say "there is no release build for $(uname -s) $(uname -m); build it from source:"
    say "  cargo install --locked --git https://github.com/$REPO && llx-desk setup command"
    exit 1
    ;;
esac

if [ -n "${LLX_DESK_VERSION:-}" ]; then
  url=https://github.com/$REPO/releases/download/v${LLX_DESK_VERSION#v}
else
  url=https://github.com/$REPO/releases/latest/download
fi

fetch() {
  if command -v curl >/dev/null 2>&1; then
    curl -fsSL --retry 2 -o "$2" "$1"
  else
    wget -q -O "$2" "$1"
  fi
}

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT INT TERM
say "downloading $archive"
fetch "$url/$archive" "$tmp/$archive" || { say "could not download $url/$archive"; exit 1; }
fetch "$url/SHA256SUMS" "$tmp/SHA256SUMS" || { say "could not download $url/SHA256SUMS"; exit 1; }

want=$(awk -v f="$archive" '$2 == f { print $1 }' "$tmp/SHA256SUMS")
if command -v sha256sum >/dev/null 2>&1; then
  got=$(sha256sum "$tmp/$archive" | awk '{ print $1 }')
else
  got=$(shasum -a 256 "$tmp/$archive" | awk '{ print $1 }')
fi
if [ -z "$want" ] || [ "$want" != "$got" ]; then
  say "the download does not match its checksum; nothing was installed"
  exit 1
fi

case "$archive" in
  *.zip)
    # A running app is the old one until it quits; the new one replaces it whole.
    ditto -x -k "$tmp/$archive" "$tmp/app"
    mkdir -p "$HOME/Applications"
    rm -rf "$HOME/Applications/LLX Desk.app.old"
    [ ! -e "$HOME/Applications/LLX Desk.app" ] || mv "$HOME/Applications/LLX Desk.app" "$HOME/Applications/LLX Desk.app.old"
    mv "$tmp/app/LLX Desk.app" "$HOME/Applications/LLX Desk.app"
    rm -rf "$HOME/Applications/LLX Desk.app.old"
    program="$HOME/Applications/LLX Desk.app/Contents/MacOS/llx-desk"
    ;;
  *)
    tar -xzf "$tmp/$archive" -C "$tmp"
    dir=${XDG_DATA_HOME:-$HOME/.local/share}/llx-desk
    mkdir -p "$dir"
    mv -f "$tmp/llx-desk" "$dir/llx-desk"
    chmod 0755 "$dir/llx-desk"
    program="$dir/llx-desk"
    if command -v ldd >/dev/null 2>&1 && ldd "$program" 2>/dev/null | grep -q 'not found'; then
      say "pages need WebKitGTK 4.1 and GTK 3; on Debian and Ubuntu: sudo apt install libwebkit2gtk-4.1-0 libgtk-3-0"
    fi
    ;;
esac

"$program" setup command
say "installed $("$program" --version). Open LLX Desk (or run \`llx-desk open\`); Settings → Integrations connects Claude Code, Cursor and Codex."
