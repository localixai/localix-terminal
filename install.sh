#!/bin/sh
# Installs Localix Terminal, the app, which is all there is to install:
#   curl -fsSL https://raw.githubusercontent.com/localixai/localix-terminal/main/install.sh | sh
# macOS: Localix Terminal.app into ~/Applications. Linux: the program into ~/.local/share/llxterm, the
# `llxterm` command and the app's desktop entry put in place by the program itself
# (`llxterm setup command`). LLXTERM_VERSION=0.1.0 picks a release. The download's checksum is
# verified against the release's SHA256SUMS before anything is written.
set -eu

REPO=localixai/localix-terminal
# The Apple Developer team that signs the Mac app (ten letters and digits). When set, the app must
# carry a Developer ID certificate of this team, not only a valid signature; empty, any signed app
# Apple notarized passes. The release workflow checks that it names the team that signs releases.
TEAM_ID=64V3JT9A64

say() { printf 'llxterm: %s\n' "$*" >&2; }

fetch() {
  if command -v curl >/dev/null 2>&1; then
    curl -fsSL --retry 2 -o "$2" "$1"
  else
    wget -q -O "$2" "$1"
  fi
}

# Whether the app's signature holds, and with TEAM_ID set, whether it is this team's Developer ID.
signed() {
  if [ -n "$TEAM_ID" ]; then
    codesign --verify --deep --strict \
      -R="anchor apple generic and certificate leaf[subject.OU] = \"$TEAM_ID\"" "$1"
  else
    codesign --verify --deep --strict "$1"
  fi
}

# All of it is in one function, called on the last line: a download cut short runs nothing.
main() {
  # A terminal under Rosetta says x86_64 on an Apple silicon Mac: the Mac's own chip decides.
  arch=$(uname -m)
  if [ "$(uname -s)" = Darwin ] && [ "$arch" = x86_64 ] && [ "$(sysctl -n hw.optional.arm64 2>/dev/null || true)" = 1 ]; then
    arch=arm64
  fi

  case "$(uname -s)/$arch" in
    Darwin/arm64) archive=Localix-Terminal-macos.zip ;;
    Linux/x86_64) archive=llxterm-x86_64-unknown-linux-gnu.tar.gz ;;
    *)
      say "there is no release build for $(uname -s) $arch; build it from source:"
      say "  cargo install --locked --git https://github.com/$REPO && llxterm setup command"
      exit 1
      ;;
  esac

  if [ -n "${LLXTERM_VERSION:-}" ]; then
    url=https://github.com/$REPO/releases/download/v${LLXTERM_VERSION#v}
  else
    url=https://github.com/$REPO/releases/latest/download
  fi

  tmp=$(mktemp -d)
  trap 'rm -rf "$tmp"' EXIT
  trap 'exit 1' INT TERM
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
      # The checksum comes from the same place as the zip: the signature and Apple's notarization
      # are what say it is Localix Terminal. Nothing is moved until both hold.
      if ! signed "$tmp/app/Localix Terminal.app" ||
        ! spctl --assess --type execute "$tmp/app/Localix Terminal.app"; then
        say "the app is not signed and notarized as it should be; nothing was installed"
        exit 1
      fi
      mkdir -p "$HOME/Applications"
      rm -rf "$HOME/Applications/Localix Terminal.app.old"
      [ ! -e "$HOME/Applications/Localix Terminal.app" ] || mv "$HOME/Applications/Localix Terminal.app" "$HOME/Applications/Localix Terminal.app.old"
      # If the new one cannot be put in place, the old one comes back.
      mv "$tmp/app/Localix Terminal.app" "$HOME/Applications/Localix Terminal.app" || {
        [ ! -e "$HOME/Applications/Localix Terminal.app.old" ] || mv "$HOME/Applications/Localix Terminal.app.old" "$HOME/Applications/Localix Terminal.app"
        say "could not install the app"
        exit 1
      }
      rm -rf "$HOME/Applications/Localix Terminal.app.old"
      program="$HOME/Applications/Localix Terminal.app/Contents/MacOS/llxterm"
      ;;
    *)
      tar -xzf "$tmp/$archive" -C "$tmp"
      dir=${XDG_DATA_HOME:-$HOME/.local/share}/llxterm
      mkdir -p "$dir"
      mv -f "$tmp/llxterm" "$dir/llxterm"
      chmod 0755 "$dir/llxterm"
      program="$dir/llxterm"
      if command -v ldd >/dev/null 2>&1 && ldd "$program" 2>/dev/null | grep -q 'not found'; then
        say "pages need WebKitGTK 4.1 and GTK 3; on Debian and Ubuntu: sudo apt install libwebkit2gtk-4.1-0 libgtk-3-0"
      fi
      ;;
  esac

  "$program" setup command
  say "installed $("$program" --version). Open Localix Terminal (or run \`llxterm open\`); Settings → Integrations connects your agents (Claude Code, Codex, Gemini CLI, Copilot, OpenCode, Hermes), the Claude Code plugin on macOS."
}

main "$@"
