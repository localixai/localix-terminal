#!/bin/bash
# The window the disk image opens as: 640 by 400 points, the app on the left, Applications on the
# right and an arrow between them, on a light ground. This writes the two files scripts/dmg.sh
# puts on the image: packaging/macos/dmg/background.tiff (drawn from background.svg, at one and at
# two pixels a point) and packaging/macos/dmg/DS_Store (the window's size, the icons' size and
# places and the picture, as Finder itself records them). Run it at a Mac when the layout or the
# picture changes and commit what it writes; a release never runs it.
#   scripts/dmg-layout.sh
# Finder lays the window out, on a scratch image mounted as "Localix Terminal": macOS asks once whether
# the terminal may control Finder, and no other volume of that name may be mounted, since Finder
# finds the picture by the volume's name. The ground is light whatever the Mac's appearance: over
# a picture Finder writes the names in black.
set -euo pipefail
cd "$(dirname "$0")/.."
volume="Localix Terminal"
dir=packaging/macos/dmg
[ ! -e "/Volumes/$volume" ] || { echo "eject the volume \"$volume\" first" >&2; exit 1; }

work=$(mktemp -d)
mounted=0
cleanup() {
  [ "$mounted" = 0 ] || hdiutil detach "/Volumes/$volume" -quiet || hdiutil detach "/Volumes/$volume" -force -quiet || true
  rm -rf "$work"
}
trap cleanup EXIT

# The picture: one drawing of the SVG for each of the two densities, joined into one file.
sips -s format png -z 400 640 "$dir/background.svg" --out "$work/background.png" >/dev/null
sips -s format png -z 800 1280 "$dir/background.svg" --out "$work/background@2x.png" >/dev/null
tiffutil -cathidpicheck "$work/background.png" "$work/background@2x.png" -out "$dir/background.tiff" >/dev/null 2>&1

# What the image holds, by name: Finder records a place for each name, so an empty folder stands
# for the app.
stage="$work/stage"
mkdir -p "$stage/.background" "$stage/Localix Terminal.app"
ln -s /Applications "$stage/Applications"
cp "$dir/background.tiff" "$stage/.background/background.tiff"
hdiutil create -quiet -volname "$volume" -srcfolder "$stage" -fs APFS -format UDRW -ov "$work/layout.dmg"
hdiutil attach -nobrowse -noautoopen -quiet "$work/layout.dmg"
mounted=1

# The window's bounds are its content, 640 by 400, and the 28 points of its title bar. An icon's
# place is its centre: both sit on the arrow's line, 288 points apart.
osascript - "$volume" <<'APPLESCRIPT'
on run argv
  tell application "Finder"
    tell disk (item 1 of argv)
      open
      set current view of container window to icon view
      set toolbar visible of container window to false
      set statusbar visible of container window to false
      set pathbar visible of container window to false
      set the bounds of container window to {240, 160, 880, 588}
      set opts to the icon view options of container window
      set arrangement of opts to not arranged
      set icon size of opts to 128
      set text size of opts to 13
      set label position of opts to bottom
      set background picture of opts to file ".background:background.tiff"
      set position of item "Localix Terminal.app" of container window to {176, 180}
      set position of item "Applications" of container window to {464, 180}
      update without registering applications
      delay 1
      close
    end tell
  end tell
end run
APPLESCRIPT

# Finder writes its record as the window shuts: waited for until it is there and no longer grows.
store="/Volumes/$volume/.DS_Store"
last=-1
for _ in $(seq 1 40); do
  sync
  size=$(stat -f %z "$store" 2>/dev/null || echo 0)
  [ "$size" -gt 0 ] && [ "$size" = "$last" ] && break
  last=$size
  sleep 0.5
done
[ "$size" -gt 0 ] && [ "$size" = "$last" ] || { echo "Finder wrote no layout" >&2; exit 1; }
cp "$store" "$dir/DS_Store"
ls -l "$dir/background.tiff" "$dir/DS_Store"
