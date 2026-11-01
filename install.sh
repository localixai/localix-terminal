#!/bin/sh
# Installs the latest release of llx-desk:
#   curl -fsSL https://raw.githubusercontent.com/rsaryev/llx-desk/main/install.sh | sh
# Into ~/.local/bin, or LLX_DESK_INSTALL; LLX_DESK_VERSION=0.1.0 picks a release. The
# archive's checksum is verified against the release's SHA256SUMS before anything is written.
set -eu

REPO=rsaryev/llx-desk
dest=${LLX_DESK_INSTALL:-$HOME/.local/bin}

say() { printf 'llx-desk: %s\n' "$*" >&2; }

case "$(uname -s)/$(uname -m)" in
  Darwin/arm64) target=aarch64-apple-darwin ;;
  Darwin/x86_64) target=x86_64-apple-darwin ;;
  Linux/x86_64) target=x86_64-unknown-linux-gnu ;;
  *)
    say "there is no release build for $(uname -s) $(uname -m); build it from source:"
    say "  cargo install --locked --git https://github.com/$REPO"
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

archive=llx-desk-$target.tar.gz
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

tar -xzf "$tmp/$archive" -C "$tmp"
mkdir -p "$dest"
mv -f "$tmp/llx-desk" "$dest/llx-desk"
chmod 0755 "$dest/llx-desk"
say "installed $("$dest/llx-desk" --version 2>/dev/null || echo llx-desk) at $dest/llx-desk"

# A running daemon is the old program until it stops; the next call starts the new one.
"$dest/llx-desk" serve stop >/dev/null 2>&1 || true

case ":$PATH:" in
  *":$dest:"*) ;;
  *) say "$dest is not on PATH; add it to your shell's profile: export PATH=\"$dest:\$PATH\"" ;;
esac

if [ "$target" = x86_64-unknown-linux-gnu ] && command -v ldd >/dev/null 2>&1 &&
  ldd "$dest/llx-desk" 2>/dev/null | grep -q 'not found'; then
  say "pages need WebKitGTK 4.1 and GTK 3; on Debian and Ubuntu: sudo apt install libwebkit2gtk-4.1-0 libgtk-3-0"
fi
