#!/bin/bash
# The app end to end, as a person and an agent use it: this tree is built and its app started
# beside yours, on a demo project, driven through the `window` tool and the keyboard, and each
# step is checked by what the tool reports, what is written to the project and what the window
# offers to assistive tools. Each scene is also pictured, in dark and in light: the window with
# the desktop around it, as it is on the screen. Run it at a Mac before a release and after a
# change a person sees; it ends with the number of checks that failed, and fails if any did.
#   scripts/e2e.sh [--update] [dark|light]
# The pictures are written to target/e2e; `--update` also copies the README's (the session, the
# editor, the board and the Store) to assets/, to be looked at and committed.
# The app it starts is apart from yours, which keeps running: it has its own socket, data and
# settings in a scratch folder (LLXTERM_RUNTIME, LLXTERM_DATA, XDG_CONFIG_HOME), and only a
# copy of the Store's index is taken from yours, so the Store is full at once. While it runs
# the other apps are hidden, yours among them, so that only the desktop is behind the window
# and around it, and are shown again after: leave the Mac alone for the two minutes it takes.
# macOS asks once whether the terminal may control System Events and record the screen. The
# session's agent is `claude`, when the login shell finds it; without it the session is a
# shell. What the window offers to assistive tools is read with `agent-desktop`, and skipped
# without it. `E2E_APP` names an app already built, to run that instead of building.
set -euo pipefail
cd "$(dirname "$0")/.."
app=${E2E_APP:-$PWD/target/release/bundle/macos/Localix Terminal.app}
cli="$app/Contents/MacOS/llxterm"
out=$PWD/target/e2e
update=0
if [ "${1:-}" = --update ]; then update=1; shift; fi
# The window's place and size on the screen, in points, and how much of the desktop around it a
# picture takes.
x=166 y=112 width=1180 height=740 margin=56
failed=0
# What the person's command links to before the run: it must be the same after.
linked=$(readlink "$HOME/.local/bin/llxterm" 2>/dev/null || true)
if [ -z "${E2E_APP:-}" ]; then
  cargo build --release --quiet
  scripts/bundle-macos.sh >/dev/null
fi
[ -x "$cli" ] || { echo "there is no app at $app" >&2; exit 1; }
mkdir -p "$out"

# One place each run, so an agent asked once whether it trusts the demo project is not asked again.
work=/private/tmp/llxterm-e2e
rm -rf "$work"
mkdir -p "$work"
demo="$work/acme-shop"
touch "$work/others"
# Where the app under test keeps its socket, its data and its settings: it and each command
# sent to it are given these, and nothing else of this shell's.
apart=(env -i HOME="$HOME" USER="$USER" PATH=/usr/bin:/bin "LLXTERM_RUNTIME=$work/run"
  "LLXTERM_DATA=$work/data" "XDG_CONFIG_HOME=$work/config")
pid=""

quit_app() {
  "${apart[@]}" "$cli" quit >/dev/null 2>&1 || true
  [ -z "$pid" ] || for _ in $(seq 20); do kill -0 "$pid" 2>/dev/null || break; sleep 0.5; done
  pid=""
}
cleanup() {
  quit_app
  while IFS= read -r name; do
    osascript -e "tell application \"System Events\" to set visible of process \"$name\" to true" >/dev/null 2>&1 || true
  done < <(sort -u "$work/others")
  rm -rf "$work"
}
trap cleanup EXIT

# The demo project: a small shop front with one commit, so the tree, the editor and the page
# have something of their own to show.
project() {
  mkdir -p "$demo/src"
  cp -R scripts/e2e/project/. "$demo/"
  git -C "$demo" init -q
  git -C "$demo" add -A
  git -C "$demo" -c user.name=Ada -c user.email=ada@example.com commit -q -m "A shop front with a cart"
}

# The app started apart from the person's, in `theme`, its window where the pictures want it
# and the side showing the project's files.
# The app under test started in the background, as a client or a login item starts it: a
# second app beside the person's, the bundle's own program run here with its places
# (LaunchServices would hand the person's running app the request instead). It has no window
# until it is asked for one.
serve() {
  "${apart[@]}" "$cli" serve > "$work/app.log" 2>&1 &
  for _ in $(seq 20); do "${apart[@]}" "$cli" status >/dev/null 2>&1 && break; sleep 0.5; done
  pid=$("${apart[@]}" "$cli" status | sed -n 's/.*(pid \([0-9]*\)).*/\1/p' | head -1)
}

# The app under test asked to open again, as its Dock icon and `open -a` ask it: the system's
# reopen event, sent to its process.
reopen() {
  osascript -l JavaScript - "$pid" >/dev/null <<'JXA'
ObjC.import("Foundation");
function run(argv) {
  const to = $.NSAppleEventDescriptor.descriptorWithProcessIdentifier(parseInt(argv[0], 10));
  // 'aevt' 'rapp': reopen application.
  const event = $.NSAppleEventDescriptor.appleEventWithEventClassEventIDTargetDescriptorReturnIDTransactionID(0x61657674, 0x72617070, to, -1, 0);
  event.sendEventWithOptionsTimeoutError(1, 5, null);
}
JXA
  sleep 3
}

start() {
  quit_app
  rm -rf "$work/run" "$work/data" "$work/config" "$demo/.localix"
  mkdir -p "$work/run" "$work/data" "$work/config/llxterm"
  chmod 700 "$work/run"
  [ ! -f "$HOME/.local/share/llxterm/catalog.sqlite" ] || cp "$HOME/.local/share/llxterm/catalog.sqlite" "$work/data/"
  [ ! -d "$HOME/.local/share/llxterm/catalog" ] || cp -R "$HOME/.local/share/llxterm/catalog" "$work/data/"
  printf '{"theme":"%s"}\n' "$1" > "$work/config/llxterm/settings.json"
  printf '{"deck":{"x":%s,"y":%s,"width":%s,"height":%s},"side":true,"side_width":248,"files":true}\n' "$x" "$y" "$width" "$height" > "$work/data/window.json"
  serve
  "${apart[@]}" "$cli" open "$demo" >/dev/null
  sleep 2
}

# AppleScript lines told to the app under test, as System Events knows it: by its process, since
# the person's app has the same name.
tell_app() {
  osascript -e "tell application \"System Events\" to tell (first process whose unix id is $pid)" "$@" -e "end tell"
}

# One call of the window tool on the demo project, its report printed; `as NAME` first makes it
# that agent's. It runs in the project with a bare environment, as a terminal the person opens
# has.
call() {
  local who=()
  if [ "$1" = as ]; then who=(--as "$2"); shift 2; fi
  (cd "$demo" && "${apart[@]}" "$cli" window "$1" --project "$demo" ${who[@]+"${who[@]}"}) 2>&1 || true
}

# A check: `has WHAT TEXT WORDS` passes when TEXT holds WORDS, and says what it read when not.
has() {
  if grep -qF -- "$3" <<<"$2"; then
    echo "  ok      $theme: $1"
  else
    failed=$((failed + 1))
    echo "  FAILED  $theme: $1: no \"$3\" in:"
    head -6 <<<"$2" | sed 's/^/            /'
  fi
}

# What the window offers to assistive tools, names and roles, one control a line.
offered() {
  agent-desktop snapshot --app "Localix Terminal" --compact 2>/dev/null | tee "$out/offered.json" || true
}

# A key pressed in the window, as the person presses it, by its place on the keyboard (43 is the
# comma, 13 the W), so the layout in use does not change which key it is: `press 43 command`.
press() {
  tell_app -e "set frontmost to true" >/dev/null
  local held=""
  [ -z "${2:-}" ] || held=" using {$2 down}"
  osascript -e "tell application \"System Events\" to key code $1$held" >/dev/null
  sleep 1
}

# Only the desktop behind the window and around it: every other app that shows is hidden, and
# its name kept so that it is shown again at the end. Before each picture, since an app used
# meanwhile comes back.
alone() {
  osascript >> "$work/others" <<APPLESCRIPT
tell application "System Events"
  set others to {}
  repeat with other in (get name of every process whose visible is true and unix id is not $pid)
    set visible of (every process whose name is (other as text) and unix id is not $pid) to false
    set end of others to other as text
  end repeat
  set AppleScript's text item delimiters to linefeed
  return others as text
end tell
APPLESCRIPT
}

# The window as it is on the screen, glass, shadow and a margin of the desktop around it, written
# to target/e2e/NAME-THEME.png. The pointer is put in a corner of the
# screen first, so nothing is pointed at.
shot() {
  local file="$out/$1-$theme.png" size
  alone
  # The window is put back where the picture is taken, were it moved since it opened, and the
  # pointer in a corner of the screen, so nothing is pointed at.
  tell_app -e "set frontmost to true" -e "set position of window 1 to {$x, $y}" \
    -e "set size of window 1 to {$width, $height}" >/dev/null
  osascript -l JavaScript -e 'ObjC.import("CoreGraphics"); $.CGEventPost($.kCGHIDEventTap, $.CGEventCreateMouseEvent(null, $.kCGEventMouseMoved, {x: 2, y: 400}, 0))' >/dev/null
  sleep 1.5
  screencapture -x -R "$((x - margin)),$((y - margin)),$((width + 2 * margin)),$((height + 2 * margin))" "$file"
  # The picture as taken is kept beside it, for what shows it larger (the site's link preview).
  cp "$file" "$out/$1-$theme-full.png"
  # Twice the width the README shows it at: sharp on a dense screen, and a third of the file.
  sips --resampleWidth 1720 "$file" >/dev/null
  size=$(stat -f %z "$file")
  has "the picture of $1 is of something ($file)" "$([ "$size" -gt 200000 ] && echo drawn)" drawn
}

scenes() {
  theme=$1
  local said agent shown path
  start "$theme"
  has "the app runs apart from the person's" "$("${apart[@]}" "$cli" status)" "pid $pid"
  has "a project opened anew has no tabs" "$(call '{"action":"list"}')" "No tabs"
  shot empty

  # The board: cards added, one taken by an agent, one handed in for the person's review.
  said=$(call '{"action":"open","kind":"board"}')
  has "the board opens as a tab" "$said" "b1"
  # shellcheck disable=SC2016
  said=$(call '{"action":"interact","tab":"b1","op":"add","cards":[
    {"title":"Cart button on every product","note":"Adds one; the header counts them."},
    {"title":"Free delivery from $30","note":"total() adds $4 under it; a test says so."},
    {"title":"Subscription: pause and cancel","note":"Both from the account page."},
    {"title":"Dark theme for the shop","note":"Follows the system."},
    {"title":"Gift cards","note":"An idea: not work yet."}]}')
  has "five cards are added" "$said" "c5"
  call as delivery '{"action":"interact","tab":"b1","op":"take","card":"c2"}' >/dev/null
  said=$(call as delivery '{"action":"interact","tab":"b1","op":"review","card":"c2","note":"total() and its test are in src/cart.ts."}')
  has "an agent hands a card in" "$said" "Review"
  said=$(call as cart-button '{"action":"interact","tab":"b1","op":"take","card":"c1"}')
  has "another agent takes a card" "$said" "c1"
  said=$(call as cart-button '{"action":"interact","tab":"b1","op":"take","card":"c3"}')
  has "an agent with a card in Doing is not given a second" "$said" "c1"
  has "the board is written to the project" "$(cat "$demo/.localix/board.json" 2>&1)" "Free delivery from"

  # The session: an agent in a terminal, a file and a page as its documents.
  if agent=$(/bin/zsh -lic 'command -v claude' 2>/dev/null | tail -1) && [ -x "$agent" ]; then
    call "{\"action\":\"open\",\"kind\":\"terminal\",\"command\":\"$agent\",\"yield_ms\":8000}" >/dev/null
    # Its question about the folder, the first time, is answered yes; then what a person would
    # ask it is typed and left unsent, so the run costs nothing and is the same each time.
    call '{"action":"interact","tab":"t1","keys":["Down","Enter"],"yield_ms":6000}' >/dev/null
    said=$(call '{"action":"interact","tab":"t1","input":"add a cart button to every product, then show me the page","yield_ms":1000}')
    has "the terminal shows what was typed to the agent" "$said" "add a cart button"
  else
    call '{"action":"open","kind":"terminal"}' >/dev/null
    # shellcheck disable=SC2016
    said=$(call '{"action":"interact","tab":"t1","input":"echo desk-$((6 * 7))\n","yield_ms":1000}')
    has "the terminal runs what was typed" "$said" "desk-42"
  fi
  said=$(call '{"action":"open","kind":"file","path":"src/cart.ts"}')
  has "a file of the project opens in an editor" "$said" "f1"
  has "the editor holds the file's text" "$(call '{"action":"interact","tab":"f1"}')" "export function total"
  said=$(call "{\"action\":\"open\",\"kind\":\"browser\",\"url\":\"$demo/index.html\"}")
  has "a page of the project opens" "$said" "Acme Shop"
  said=$(call '{"action":"interact","tab":"w1","script":"document.querySelectorAll(\"article\").length + \" products, dark: \" + matchMedia(\"(prefers-color-scheme: dark)\").matches"}')
  has "the page is drawn" "$said" "3 products"
  has "the page follows the window's theme" "$said" "dark: $([ "$theme" = dark ] && echo true || echo false)"
  said=$(call '{"action":"list"}')
  has "the project's tabs are listed" "$said" "t1 (terminal)"
  sleep 1
  shot window
  if command -v agent-desktop >/dev/null; then
    said=$(offered)
    has "the side names the project's files" "$said" "index.html"
    has "the side names the board" "$said" "Board of cards"
  fi
  # What is made in the project's folder shows in the tree while it is in sight.
  echo "Notes for the shop." > "$demo/NOTES.md"
  sleep 2
  if command -v agent-desktop >/dev/null; then
    has "a file made on disk shows in the tree" "$(offered)" "NOTES.md"
  fi
  rm "$demo/NOTES.md"

  call '{"action":"interact","tab":"b1","op":"show"}' >/dev/null
  shot board

  # The Store: its tab, a search an agent makes, and a listing read.
  said=$(call '{"action":"open","kind":"store"}')
  has "the Store opens with its listings" "$said" "plugins"
  said=$(call '{"action":"interact","tab":"p1","store":{"search":"github","limit":5}}')
  has "the Store finds what is searched for" "$said" "github"
  sleep 2
  shot store

  # Settings, as the person opens them with the keyboard: a tab, shut as any tab is.
  press 43 command
  has "Settings open on the key" "$(call '{"action":"list"}' ; offered)" "Appearance and shortcuts"
  shot settings
  press 13 command
  sleep 1
  if command -v agent-desktop >/dev/null; then
    said=$(offered)
    has "the key that shuts a tab shuts Settings" "$(grep -c 'Appearance and shortcuts' <<<"$said" || true)" "0"
  fi

  # A tab closed is gone from the list, and the others stay.
  call '{"action":"close","tab":"w1"}' >/dev/null
  said=$(call '{"action":"list"}')
  has "the other tabs stay when one is closed" "$said" "f1 (file)"

  # The editor: the session closed, and its documents with it. Files opened in a project with
  # no terminal are one tab of their own, a row of them over the editor, the first in sight.
  call '{"action":"close","tab":"t1"}' >/dev/null
  has "a session closing closes its documents" "$(call '{"action":"list"}' | grep -c '(file)' || true)" "0"
  said=$(call '{"action":"open","kind":"file","path":"src/cart.ts"}')
  shown=$(grep -o 'f[0-9][0-9]*' <<<"$said" | head -1)
  for path in src/cart.test.ts src/products.ts src/checkout.ts; do
    call "{\"action\":\"open\",\"kind\":\"file\",\"path\":\"$path\"}" >/dev/null
  done
  call "{\"action\":\"show\",\"tab\":\"$shown\"}" >/dev/null
  said=$(call '{"action":"list"}')
  has "files opened with no session stay open together" "$said" "src/checkout.ts"
  has "the file asked for is the one in sight" "$said" "$shown (file) [shown]: src/cart.ts"
  sleep 1
  shot editor
  # The app under test is not the one the person uses: their command still runs theirs.
  has "the person's command is left as it was" "$(readlink "$HOME/.local/bin/llxterm" 2>&1)" "$linked"
}

project
for theme in ${1:-dark light}; do scenes "$theme"; done
# The window comes back when the app is asked again, whatever was open last: a project whose
# folder was deleted since must not leave the person with no window.
reopening() {
  theme=reopen
  start dark
  mkdir -p "$work/gone"
  "${apart[@]}" "$cli" open "$work/gone" >/dev/null
  sleep 1
  quit_app
  rm -rf "$work/gone"
  serve
  has "started in the background, the app has no window yet" "$("${apart[@]}" "$cli" status)" "window: not open yet"
  reopen
  has "asked again, it opens its window, though the project used last is gone" "$("${apart[@]}" "$cli" status)" "window: open"
}
reopening

if [ "$update" = 1 ]; then
  for name in window editor board store; do cp "$out/$name"-*.png assets/; done
fi
if [ "$failed" = 0 ]; then
  echo "e2e: every check passed; the pictures are in $out"
else
  echo "e2e: $failed failed" >&2
  exit 1
fi
