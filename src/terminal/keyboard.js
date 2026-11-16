// The keys a Mac terminal has, on top of what xterm.js sends: ⌘K clears, ⌘←/⌘→ go to the line's
// start and end, ⌥←/⌥→ by words, ⌘⌫/⌥⌫ delete to the start and the word before, ⌘↑/⌘↓ scroll to
// the top and bottom, ⌘A selects all. Sent as the readline keys every shell and REPL reads; the
// window's own shortcuts (⌘T, ⌘W…) go on to its menu.
//
// Shift+Enter sends LF: a coding agent's prompt reads it as a new line rather than the end of the
// message, with nothing to set up, and a shell takes it as Enter. ESC CR would be unbound in one
// bash and turn another to vi editing. Option+Enter sends ESC CR, the Meta Return a line editor
// may bind, only from an Option that is Meta; another Option's Return is Enter.
const SENT = {
  "meta+ArrowLeft": "\x01", "meta+ArrowRight": "\x05", "alt+ArrowLeft": "\x1bb", "alt+ArrowRight": "\x1bf",
  "meta+Backspace": "\x15", "alt+Backspace": "\x17", "meta+Delete": "\x0b", "alt+Delete": "\x1bd",
  "shift+Enter": "\n",
};
// Which Option is Meta (Settings, `option` of the window's look): "left", "right", "both" or
// "off", the default, since many layouts (German, French, Nordic, Polish) type @ [ ] { } | \ ~
// with either Option. With Meta, Option and a key send ESC and the key (xterm's own encoding,
// turned on for that key); the other Option types the character the layout gives it.
let optionMeta = (window.__deskLook && window.__deskLook.option) || "off";
addEventListener("desk-look", (event) => {
  optionMeta = event.detail.option || "off";
});
// The Option keys down now, by their own place: a key pressed with Option says only that one is
// down, not which. Forgotten as the page loses the keys, since their release goes elsewhere, and
// at any key pressed without Option, so a release the page missed leaves no side held.
const optionsDown = new Set();
addEventListener("blur", () => optionsDown.clear());
function optionIsMeta() {
  if (optionMeta === "both" || optionMeta === "off") return optionMeta === "both";
  // One held since before the page had the keys was never seen: the left one, the one most pressed.
  const side = optionsDown.has("AltRight") && !optionsDown.has("AltLeft") ? "right" : "left";
  return side === optionMeta;
}
term.attachCustomKeyEventHandler((event) => {
  if (event.type === "keydown" && !event.altKey) optionsDown.clear();
  if (event.code === "AltLeft" || event.code === "AltRight") {
    if (event.type === "keydown") optionsDown.add(event.code);
    else if (event.type === "keyup") optionsDown.delete(event.code);
    return true;
  }
  if (event.type !== "keydown") return true;
  // Set before any key goes to xterm, Ctrl+Option and a key among them: it encodes them by it.
  const meta = event.altKey && optionIsMeta();
  if (event.altKey && term.options.macOptionIsMeta !== meta) term.options.macOptionIsMeta = meta;
  if (event.ctrlKey || event.isComposing) return true;
  const only = event.shiftKey && !event.metaKey && !event.altKey;
  if (event.shiftKey && !only) return true;
  const name = (event.metaKey ? "meta+" : event.altKey ? "alt+" : only ? "shift+" : "") + event.key;
  const done = (run) => {
    event.preventDefault();
    run();
    return false;
  };
  // Option's Return is sent here: xterm itself sends ESC CR for either Option's, Meta or not.
  if (name === "alt+Enter") return done(() => post(meta ? "k\x1b\r" : "k\r"));
  if (SENT[name]) return done(() => post("k" + SENT[name]));
  if (!event.metaKey || event.altKey) return true;
  switch (event.key.toLowerCase()) {
    case "k": return done(clearScreen);
    case "arrowup": return done(() => term.scrollToTop());
    case "arrowdown": return done(() => term.scrollToBottom());
    case "a": return done(() => term.selectAll());
    default: return true;
  }
});
