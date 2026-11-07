#!/bin/sh
# Packs a release build for Linux: dist/llx-desk-TARGET.tar.gz with the program and the licence,
# what install.sh downloads (macOS gets LLX Desk.app instead, scripts/bundle-macos.sh).
#   scripts/package.sh target/release/llx-desk x86_64-unknown-linux-gnu
set -eu
[ $# -eq 2 ] || { echo 'usage: scripts/package.sh BINARY TARGET' >&2; exit 2; }
binary=$1
target=$2
# Taken from where the caller is, before the move to the repository's root.
case $binary in /*) ;; *) binary=$PWD/$binary ;; esac
cd "$(dirname "$0")/.."

stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT
cp "$binary" "$stage/llx-desk"
chmod 0755 "$stage/llx-desk"
cp LICENSE "$stage/LICENSE"
mkdir -p dist
tar -czf "dist/llx-desk-$target.tar.gz" -C "$stage" llx-desk LICENSE
echo "dist/llx-desk-$target.tar.gz"
