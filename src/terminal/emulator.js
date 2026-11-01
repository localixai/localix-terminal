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
// The page's colours; the ground is the window's, so the terminal paints none.
function theme() {
  const fg = css("--fg"), bg = css("--bg");
  const colours = {
    background: "rgba(0,0,0,0)", foreground: fg, cursor: css("--cursor"), cursorAccent: bg,
    selectionBackground: fg + "47",
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
  cursorBlink: true, cursorStyle: "block",
  fontFamily: css("--mono"), fontSize: 13, lineHeight: 1.1, theme: theme(),
});
const fit = new FitAddon.FitAddon();
term.loadAddon(fit);
term.open(document.getElementById("t"));
// What this page posts (panel.rs): the keys and pastes as a terminal sends them (k), its size
// once it is ready (r) and when the pane changes (s), and its menu asked for where the person
// right-clicked, with whether something is selected (Copy).
const post = (text) => window.ipc && window.ipc.postMessage(text);
term.onData((data) => post("k" + data));
const sized = (kind) => post(kind + " " + term.cols + " " + term.rows);
term.onResize(() => sized("s"));
addEventListener("contextmenu", (event) => {
  event.preventDefault();
  const selected = term.hasSelection() ? 1 : 0;
  post("m " + Math.round(event.clientX) + " " + Math.round(event.clientY) + " " + selected);
});
// The grid follows the pane; the session is told (term.onResize) and its pty resized.
function refit() {
  if (innerWidth && innerHeight) fit.fit();
}
refit();
addEventListener("resize", refit);
document.fonts.ready.then(refit);
matchMedia("(prefers-color-scheme: dark)").addEventListener("change", () => {
  term.options.theme = theme();
});
term.focus();
addEventListener("focus", () => term.focus());
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
