#!/bin/sh
# Installs the Zig that builds the terminal core (libghostty-vt) where its release has no
# prebuilt library: Linux. The version is the one the core's bindings (ghostty-vt-sys) take,
# fetched from ziglang.org and held to its sha256 below before it is unpacked.
#
#   scripts/install-zig.sh [DIR]
#
# Unpacks into DIR (by default ~/.cache/llxterm/zig), once per version, and prints the folder
# that holds `zig`: CI puts it on the PATH (`scripts/install-zig.sh >> "$GITHUB_PATH"`). A
# version already there is checked by `zig version` and used as it is.
set -eu

version=0.17.1
case "$(uname -s)/$(uname -m)" in
  Linux/x86_64)
    arch=x86_64
    sum=3581863e3cbcfd0fce0efc682ce7e9e93da96668a977791d356a26d2a7e9b17a
    ;;
  Linux/aarch64 | Linux/arm64)
    arch=aarch64
    sum=a1546d5646eb184437918d6635ca6b762a0e889eac09cef720ee7a1ed3fe7795
    ;;
  *)
    echo "install-zig.sh: Linux only: macOS links the core's prebuilt library" >&2
    exit 1
    ;;
esac

root=${1:-${HOME}/.cache/llxterm/zig}
name=zig-$arch-linux-$version
dir=$root/$name
if [ -x "$dir/zig" ] && [ "$("$dir/zig" version)" = "$version" ]; then
  echo "$dir"
  exit 0
fi

mkdir -p "$root"
tmp=$(mktemp -d "$root/.fetch.XXXXXX")
trap 'rm -rf "$tmp"' EXIT
curl --proto '=https' --tlsv1.2 -fsSL --retry 3 \
  -o "$tmp/$name.tar.xz" "https://ziglang.org/download/$version/$name.tar.xz"
echo "$sum  $tmp/$name.tar.xz" | sha256sum -c - >&2
tar -xJf "$tmp/$name.tar.xz" -C "$tmp"
rm -rf "$dir"
mv "$tmp/$name" "$dir"
[ "$("$dir/zig" version)" = "$version" ]
echo "$dir"
