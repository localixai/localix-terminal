#!/bin/sh
# Packs a release build: dist/llx-desk-TARGET.tar.gz with the program and the licence, the names
# the plugin's launcher and install.sh download.
#   scripts/package.sh target/release/llx-desk x86_64-unknown-linux-gnu
set -eu
binary=$1
target=$2
cd "$(dirname "$0")/.."

stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT
cp "$binary" "$stage/llx-desk"
chmod 0755 "$stage/llx-desk"
cp LICENSE "$stage/LICENSE"
mkdir -p dist
tar -czf "dist/llx-desk-$target.tar.gz" -C "$stage" llx-desk LICENSE
echo "dist/llx-desk-$target.tar.gz"
