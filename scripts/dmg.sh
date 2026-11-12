#!/bin/bash
# The release files of the Mac app: LLX-Desk-macos.dmg (the disk image people open, the app and a
# link to /Applications in it, in the window packaging/macos/dmg lays out) and LLX-Desk-macos.zip (the app, which install.sh fetches), in dist.
# CI and a person at this Mac run the same script.
#   scripts/dmg.sh [--universal] [--notarize] [--out dist]
# --universal builds both Macs' programs and joins them with lipo (releases are Apple silicon only
# and do not use it); without it the program is the one of this Mac. The signature is LLX_DESK_SIGN, or else the first Developer ID
# Application identity of the keychain, or else ad hoc: an ad hoc build is bundled, verified and
# zipped, and Gatekeeper will refuse it on another Mac. --notarize needs a Developer ID identity
# and the notary's credentials, either a keychain profile in LLX_NOTARY_PROFILE (made once with
# `xcrun notarytool store-credentials`) or APPLE_API_KEY_PATH, APPLE_API_KEY and APPLE_API_ISSUER.
# The app is notarized and stapled first, so the zip and the disk image both hold its ticket; then
# the disk image is signed, notarized and stapled as well. The keychain and its search list are
# never touched here, and no secret is printed.
set -euo pipefail
cd "$(dirname "$0")/.."
# The oldest macOS the app runs on is Info.plist's: the program is built for it and checked against it.
min=$(plutil -extract LSMinimumSystemVersion raw packaging/macos/Info.plist)
export MACOSX_DEPLOYMENT_TARGET=$min

universal=0
notarize=0
out=dist
while [ $# -gt 0 ]; do
  case "$1" in
    --universal) universal=1 ;;
    --notarize) notarize=1 ;;
    --out)
      [ $# -ge 2 ] || { echo "--out needs a directory" >&2; exit 2; }
      out=$2
      shift
      ;;
    *) echo "usage: scripts/dmg.sh [--universal] [--notarize] [--out dist]" >&2; exit 2 ;;
  esac
  shift
done
mkdir -p "$out"
out=$(cd "$out" && pwd)

version=$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -n 1)
scripts/check-versions.sh

# The signature: what the environment names, else the keychain's Developer ID, else ad hoc.
sign=${LLX_DESK_SIGN:-}
if [ -z "$sign" ]; then
  sign=$(security find-identity -v -p codesigning |
    sed -n 's/.*"\(Developer ID Application:.*\)".*/\1/p' | head -n 1)
fi
sign=${sign:--}
team=
case "$sign" in
  *\(*\)) team=${sign##*(}; team=${team%)} ;;
esac
if [ "$sign" = - ]; then
  echo "signing ad hoc: Gatekeeper will refuse this build on another Mac"
  [ "$notarize" = 0 ] || { echo "--notarize needs a Developer ID identity (LLX_DESK_SIGN)" >&2; exit 1; }
else
  echo "signing with a Developer ID identity${team:+, team $team}"
fi

# The notary's credentials, checked before anything is built.
AUTH=()
if [ "$notarize" = 1 ]; then
  if [ -n "${LLX_NOTARY_PROFILE:-}" ]; then
    AUTH=(--keychain-profile "$LLX_NOTARY_PROFILE")
  elif [ -n "${APPLE_API_KEY_PATH:-}" ] && [ -n "${APPLE_API_KEY:-}" ] && [ -n "${APPLE_API_ISSUER:-}" ]; then
    [ -r "$APPLE_API_KEY_PATH" ] || { echo "no key file at APPLE_API_KEY_PATH" >&2; exit 1; }
    AUTH=(--key "$APPLE_API_KEY_PATH" --key-id "$APPLE_API_KEY" --issuer "$APPLE_API_ISSUER")
  else
    echo "--notarize needs LLX_NOTARY_PROFILE, or APPLE_API_KEY_PATH, APPLE_API_KEY and APPLE_API_ISSUER" >&2
    exit 1
  fi
fi

work=$(mktemp -d)
mount=
cleanup() {
  if [ -n "$mount" ]; then
    hdiutil detach "$mount" -quiet || hdiutil detach "$mount" -force -quiet || true
  fi
  rm -rf "$work"
}
trap cleanup EXIT

# Apple's servers fail now and then: a step that talks to them is tried four times, 30 seconds apart
# (the notary's answers are asked again and again by notarize_file itself).
retry() {
  local attempt
  for attempt in 1 2 3 4; do
    "$@" && return 0
    [ "$attempt" = 4 ] || sleep 30
  done
  return 1
}

# Signs the disk image (the app is signed by scripts/bundle-macos.sh): the timestamp is Apple's.
codesign_image() {
  local tries=0 delay=5
  until codesign --force --timestamp --sign "$sign" "$1"; do
    tries=$((tries + 1))
    [ "$tries" -lt 4 ] || { echo "codesign failed" >&2; return 1; }
    sleep "$delay"
    delay=$((delay * 2))
  done
}

# Sends a file to the notary once and waits for the verdict by its id, so a poll that fails
# resumes where it was and does not upload again.
notarize_file() {
  local id status end fails=0
  id=$(retry xcrun notarytool submit "$1" "${AUTH[@]}" --no-wait --output-format json |
    plutil -extract id raw -o - -) || return 1
  echo "submitted $(basename "$1") to the notary as $id"
  end=$((SECONDS + 1800))
  while [ "$SECONDS" -lt "$end" ]; do
    if status=$(xcrun notarytool info "$id" "${AUTH[@]}" --output-format json |
      plutil -extract status raw -o - -); then
      fails=0
      case "$status" in
        Accepted) echo "the notary accepted $(basename "$1")"; return 0 ;;
        Invalid | Rejected)
          xcrun notarytool log "$id" "${AUTH[@]}" >&2 || true
          echo "the notary says $status for $(basename "$1")" >&2
          return 1
          ;;
      esac
    else
      fails=$((fails + 1))
      [ "$fails" -lt 5 ] || { echo "the notary did not answer five times" >&2; return 1; }
    fi
    sleep 15
  done
  echo "the notary took more than 30 minutes" >&2
  return 1
}

# The ticket is stapled and then checked: a check alone could not add a ticket that is missing.
staple_and_validate() {
  xcrun stapler staple "$1" && xcrun stapler validate "$1"
}

# The program: this Mac's, or one for both Macs.
if [ "$universal" = 1 ]; then
  rustup target add aarch64-apple-darwin x86_64-apple-darwin
  for target in aarch64-apple-darwin x86_64-apple-darwin; do
    cargo build --release --locked --target "$target"
  done
  mkdir -p target/universal
  lipo -create -output target/universal/llx-desk \
    target/aarch64-apple-darwin/release/llx-desk target/x86_64-apple-darwin/release/llx-desk
  binary=target/universal/llx-desk
  archs=$(lipo -archs "$binary")
  # Each on its own: lipo names them in either order, one space apart.
  for arch in arm64 x86_64; do
    case " $archs " in
      *" $arch "*) ;;
      *) echo "the program has $archs, not arm64 and x86_64" >&2; exit 1 ;;
    esac
  done
else
  cargo build --release --locked
  binary=target/release/llx-desk
fi
# No program may ask for a newer macOS than Info.plist's LSMinimumSystemVersion.
for minos in $(otool -l "$binary" | awk '/^ +minos /{ print $2 }'); do
  newest=$(printf '%s\n' "$minos" "$min" | sort -V | tail -n 1)
  [ "$newest" = "$min" ] || { echo "the program needs macOS $minos, Info.plist says $min" >&2; exit 1; }
done

# The app, signed by the script that makes it.
app=$(LLX_DESK_SIGN="$sign" scripts/bundle-macos.sh "$binary" "$out/bundle")
codesign --verify --deep --strict --verbose=2 "$app"
if [ "$sign" != - ]; then
  seen=$(codesign -dv --verbose=2 "$app" 2>&1 | sed -n 's/^TeamIdentifier=//p')
  if [ -n "$team" ]; then
    [ "$seen" = "$team" ] || { echo "the app is signed by team '$seen', not $team" >&2; exit 1; }
  else
    [ -n "$seen" ] && [ "$seen" != "not set" ] || { echo "the app has no team identifier" >&2; exit 1; }
  fi
fi

if [ "$notarize" = 1 ]; then
  ditto -c -k --keepParent "$app" "$work/app-notarize.zip"
  notarize_file "$work/app-notarize.zip"
  retry staple_and_validate "$app"
fi

# The update zip is made from the stapled app, so it carries the ticket.
rm -f "$out/LLX-Desk-macos.zip" "$out/LLX-Desk-macos.dmg"
ditto -c -k --keepParent "$app" "$out/LLX-Desk-macos.zip"

# The disk image: the app and a link to /Applications to drag it to, in the window that
# scripts/dmg-layout.sh laid out: its picture and Finder's record of the window, both hidden. The
# record finds the picture by the volume's name, so the name here is the one that script uses.
# APFS, since macOS 26.4 beta cannot mount an HFS+ image.
stage="$work/stage"
mkdir -p "$stage/.background"
ditto "$app" "$stage/LLX Desk.app"
ln -s /Applications "$stage/Applications"
cp packaging/macos/dmg/background.tiff "$stage/.background/background.tiff"
cp packaging/macos/dmg/DS_Store "$stage/.DS_Store"
hdiutil create -quiet -volname "LLX Desk" -srcfolder "$stage" -fs APFS -format UDZO -ov \
  "$out/LLX-Desk-macos.dmg"
if [ "$sign" != - ]; then
  codesign_image "$out/LLX-Desk-macos.dmg"
  codesign --verify --strict --verbose=2 "$out/LLX-Desk-macos.dmg"
fi
if [ "$notarize" = 1 ]; then
  notarize_file "$out/LLX-Desk-macos.dmg"
  retry staple_and_validate "$out/LLX-Desk-macos.dmg"
fi

# What a person gets by opening the image: the app signed as it was, and the link beside it.
mount="$work/mount"
mkdir -p "$mount"
hdiutil attach -nobrowse -readonly -noautoopen -quiet -mountpoint "$mount" "$out/LLX-Desk-macos.dmg"
[ -d "$mount/LLX Desk.app" ] || { echo "the disk image has no LLX Desk.app" >&2; exit 1; }
[ -L "$mount/Applications" ] || { echo "the disk image has no link to /Applications" >&2; exit 1; }
[ -s "$mount/.DS_Store" ] && [ -s "$mount/.background/background.tiff" ] ||
  { echo "the disk image has no window layout" >&2; exit 1; }
codesign --verify --deep --strict --verbose=2 "$mount/LLX Desk.app"
if [ "$notarize" = 1 ]; then
  spctl --assess --type execute -vv "$mount/LLX Desk.app"
  spctl --assess --type open --context context:primary-signature -vv "$out/LLX-Desk-macos.dmg"
elif [ "$sign" = - ]; then
  echo "not asking Gatekeeper: it refuses an ad hoc build on another Mac"
else
  echo "not asking Gatekeeper: this build is signed but not notarized"
fi
hdiutil detach "$mount" -quiet
mount=

(cd "$out" && shasum -a 256 LLX-Desk-macos.dmg LLX-Desk-macos.zip > SHA256SUMS.local)
echo "LLX Desk $version:"
ls -l "$out/LLX-Desk-macos.dmg" "$out/LLX-Desk-macos.zip"
cat "$out/SHA256SUMS.local"
