"use strict";
// The session in xterm.js: the program's own output is written into it (W), the whole screen
// again when the page is new or behind (R); what the person types and pastes goes to the session,
// and the grid follows the pane, the session's terminal resized with it.
const base = document.title;
const style = () => getComputedStyle(document.documentElement);
const css = (name) => style().getPropertyValue(name).trim();
const DARK = ["#484f58", "#ff7b72", "#3fb950", "#d29922", "#58a6ff", "#bc8cff", "#39c5cf",
  "#b1bac4", "#6e7681", "#ffa198", "#56d364", "#e3b341", "#79c0ff", "#d2a8ff", "#56d4dd", "#f0f6fc"];
const LIGHT = ["#24292e", "#d73a49", "#28a745", "#b08800", "#0366d6", "#6f42c1", "#1b7c83",
  "#6a737d", "#959da5", "#cb2431", "#22863a", "#b08800", "#005cc5", "#5a32a3", "#1b7c83", "#24292e"];
const NAMES = ["black", "red", "green", "yellow", "blue", "magenta", "cyan", "white"];
// Whether a #rrggbb colour reads as light.
function light(hex) {
  const n = parseInt(hex.slice(1), 16);
  return 0.2126 * (n >> 16) + 0.7152 * ((n >> 8) & 255) + 0.0722 * (n & 255) >= 127.5;
}
// The system's accent colour, as the page reads it: an element coloured by the system's own
// keyword, empty where the page's engine has none.
const probe = document.createElement("i");
probe.style.cssText = "display:none;color:-apple-system-control-accent";
document.documentElement.append(probe);
const system = () => {
  const colour = getComputedStyle(probe).color;
  return colour && colour !== "rgb(0, 0, 0)" ? colour : "";
};
// `colour` (#rrggbb or rgb(...)) at the opacity `a`.
function alpha(colour, a) {
  const n = colour.startsWith("#")
    ? [1, 3, 5].map((i) => parseInt(colour.slice(i, i + 2), 16))
    : colour.match(/[\d.]+/g).slice(0, 3).map(Number);
  return "rgba(" + n.join(",") + "," + a + ")";
}
// The page's colours; the ground is the window's, so the terminal paints none. The cursor is the
// text's colour (an outline in a pane not in use); a selection is the accent at 40%.
function theme() {
  const fg = css("--fg"), bg = css("--bg");
  const colours = {
    background: "rgba(0,0,0,0)", foreground: fg, cursor: css("--cursor"), cursorAccent: bg,
    selectionBackground: system() ? alpha(system(), 0.4) : "rgba(118,167,250,.4)",
    selectionInactiveBackground: alpha(fg, 0.18),
    scrollbarSliderBackground: fg + "66", scrollbarSliderHoverBackground: fg + "e6",
    scrollbarSliderActiveBackground: fg + "e6",
  };
  (light(bg) ? LIGHT : DARK).forEach((colour, i) => {
    const name = NAMES[i % 8];
    colours[i < 8 ? name : "bright" + name[0].toUpperCase() + name.slice(1)] = colour;
  });
  return colours;
}
// xterm stops drawing while its element is out of sight and starts again when an
// IntersectionObserver says it is back. A GTK webview hidden and shown again never says so, and
// the terminal kept the one cell it had drawn. This page is the terminal alone: it always draws.
class InSight {
  constructor(told) { this.told = told; }
  observe(target) { this.told([{ target, isIntersecting: true, intersectionRatio: 1 }]); }
  unobserve() {}
  disconnect() {}
}
try {
  Object.defineProperty(window, "IntersectionObserver", { value: InSight, configurable: true, writable: true });
} catch {
  window.IntersectionObserver = InSight;
}
const term = new Terminal({
  scrollback: 5000, allowTransparency: true, allowProposedApi: false,
  cursorBlink: false, cursorStyle: "block", cursorInactiveStyle: "outline",
  // A link the program writes (OSC 8) opens as the links found in the text do: with ⌘-click, as
  // a tab of the window. xterm's own handler would ask with a dialog and open a window the
  // panel denies.
  linkHandler: {
    allowNonHttpProtocols: false, activate: (event, uri) => openTab(event, uri), hover() {}, leave() {},
  },
  fontFamily: (window.__deskLook && window.__deskLook.mono) || css("--mono"),
  fontSize: (window.__deskLook && window.__deskLook.size) || 13, lineHeight: 1, theme: theme(),
});
// The person's code font and size (the window's settings), now and as they change.
addEventListener("desk-look", (event) => {
  term.options.fontFamily = event.detail.mono;
  term.options.fontSize = event.detail.size;
  refit();
});
term.open(document.getElementById("t"));
// What this page posts (panel.rs): the keys and pastes as a terminal sends them (k), its size
// once it is ready (r) and when the pane changes (s), and its menu asked for where the person
// right-clicked, with the items of its menu.
const post = (text) => window.ipc && window.ipc.postMessage(text);
// An address is opened in a new tab (o) by ⌘-click only: a click alone selects or moves the cursor.
function openTab(event, uri) {
  if (event.metaKey) post("o " + uri);
}
// Addresses in the text of a line: http and https, without the punctuation that ends a sentence.
// Columns are taken as characters, as ASCII has them.
const ADDRESS = /https?:\/\/[^\s<>"'`]+[^\s<>"'`.,;:!?)\]}]/g;
term.registerLinkProvider({
  provideLinks(y, found) {
    const line = term.buffer.active.getLine(y - 1);
    const text = line ? line.translateToString(true) : "";
    const links = [...text.matchAll(ADDRESS)].map((match) => ({
      text: match[0],
      range: { start: { x: match.index + 1, y }, end: { x: match.index + match[0].length, y } },
      decorations: { pointerCursor: true, underline: true },
      activate: openTab,
    }));
    found(links.length ? links : undefined);
  },
});
term.onData((data) => post("k" + data));
// The keys a Mac terminal has: ⌘K clears, ⌘←/⌘→ go to the line's start and end, ⌥←/⌥→ by
// words, ⌘⌫/⌥⌫ delete to the start and the word before, ⌘↑/⌘↓ scroll to the top and bottom, ⌘A
// selects all. Sent as the readline keys every shell and REPL reads; the window's own shortcuts
// (⌘T, ⌘W…) go on to its menu.
const SENT = {
  "meta+ArrowLeft": "\x01", "meta+ArrowRight": "\x05", "alt+ArrowLeft": "\x1bb", "alt+ArrowRight": "\x1bf",
  "meta+Backspace": "\x15", "alt+Backspace": "\x17", "meta+Delete": "\x0b", "alt+Delete": "\x1bd",
};
term.attachCustomKeyEventHandler((event) => {
  if (event.type !== "keydown" || event.ctrlKey || event.shiftKey) return true;
  const name = (event.metaKey ? "meta+" : event.altKey ? "alt+" : "") + event.key;
  const done = (run) => {
    event.preventDefault();
    run();
    return false;
  };
  if (SENT[name]) return done(() => post("k" + SENT[name]));
  if (!event.metaKey || event.altKey) return true;
  switch (event.key.toLowerCase()) {
    case "k":
      // The scrollback and the screen go; a shell draws its prompt again at the top.
      return done(() => {
        term.clear();
        if (term.buffer.active.type === "normal") post("k\f");
      });
    case "arrowup": return done(() => term.scrollToTop());
    case "arrowdown": return done(() => term.scrollToBottom());
    case "a": return done(() => term.selectAll());
    default: return true;
  }
});
const sized = (kind) => post(kind + " " + term.cols + " " + term.rows);
term.onResize(() => sized("s"));
// The menu is the system's: the page names its items and does the one picked (`chosen`), the
// window copies what Copy answers and hands Paste the pasteboard's text. Two groups, a line
// between them.
const MENU = [
  [
    { label: "Copy", when: () => term.hasSelection(), run: () => term.getSelection() },
    { label: "Paste", paste: true, run: (text) => { if (text) term.paste(text); } },
  ],
  [
    { label: "Select All", run: () => term.selectAll() },
    { label: "Clear", run: () => post("k\f") },
  ],
];
let offered = [];
addEventListener("contextmenu", (event) => {
  event.preventDefault();
  const groups = MENU.map((group) => group.filter((item) => !item.when || item.when()))
    .filter((group) => group.length);
  // What is picked is a place among the items alone: the lines are not counted.
  offered = groups.flat();
  const items = groups.flatMap((group, i) => [
    ...(i ? [{ line: true }] : []),
    ...group.map((item) => ({ label: item.label, paste: !!item.paste })),
  ]);
  post("m " + Math.round(event.clientX) + " " + Math.round(event.clientY) + " " + JSON.stringify(items));
});
window.chosen = (place, text) => {
  const item = offered[place];
  const answer = item ? item.run(text) : undefined;
  return typeof answer === "string" ? answer : null;
};
// The grid follows the pane; the session is told (term.onResize) and its pty resized. The
// fit is the page's own, and at once: the first size is posted before the page says it is
// ready, and a webview out of sight may run no frames. The scrollbar takes no width.
function refit() {
  if (!innerWidth || !innerHeight || !term.element) return;
  const cell = term._core._renderService.dimensions.css.cell;
  if (!cell.width || !cell.height) return;
  const cs = getComputedStyle(term.element);
  const w = term.element.clientWidth - parseFloat(cs.paddingLeft) - parseFloat(cs.paddingRight);
  const h = term.element.clientHeight - parseFloat(cs.paddingTop) - parseFloat(cs.paddingBottom);
  const cols = Math.max(2, Math.floor(w / cell.width)), rows = Math.max(1, Math.floor(h / cell.height));
  if (cols !== term.cols || rows !== term.rows) term.resize(cols, rows);
}
// A full-screen program has no gutter (the page's style), so the grid is fitted again.
term.buffer.onBufferChange((buffer) => {
  document.documentElement.toggleAttribute("data-alt", buffer.type === "alternate");
  refit();
});
refit();
addEventListener("resize", refit);
document.fonts.ready.then(refit);
matchMedia("(prefers-color-scheme: dark)").addEventListener("change", () => {
  term.options.theme = theme();
});
term.focus();
addEventListener("focus", () => term.focus());
// A click anywhere in the page, the scrollbar's track too, is a click in the terminal.
document.addEventListener("mousedown", () => term.focus());
// The session's output, base64: raw bytes as the program wrote them.
const bytes = (b64) => Uint8Array.from(atob(b64), (c) => c.charCodeAt(0));
function W(b64) {
  term.write(bytes(b64));
}
// The whole screen again, for a page that just loaded or one that fell behind.
function R(b64) {
  term.reset();
  term.write(bytes(b64));
}
// The session ended: the title says so.
function E(end) {
  document.title = base + end;
}
sized("r");
