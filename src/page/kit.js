"use strict";
// What the app's own pages share (page.rs puts it before each page's script, so the two run as
// one): what a page posts to the app, how it builds an element and a line glyph, and the
// system's menu. A page defines none of these again.
const post = (text) => window.ipc && window.ipc.postMessage(text);
// What the person did, as one JSON action ("k" and it).
const act = (action) => post("k" + JSON.stringify(action));

// An element: its tag with classes (`div.card.sel`), its fields, and its children.
function el(spec, fields, ...kids) {
  const [tag, ...classes] = spec.split(".");
  const node = document.createElement(tag);
  if (classes.length) node.className = classes.join(" ");
  for (const [name, value] of Object.entries(fields || {})) {
    if (value === undefined || value === null || value === false) continue;
    if (name === "text") node.textContent = value;
    else if (name.startsWith("on")) node.addEventListener(name.slice(2), value);
    else if (name === "style") node.style.cssText = value;
    else node.setAttribute(name, value === true ? "" : value);
  }
  for (const kid of kids.flat()) if (kid !== null && kid !== undefined && kid !== false) node.append(kid);
  return node;
}

// A line glyph: the path `d` on a grid of `grid` points, `size` points on the page when the
// sheet does not size it.
function svgPath(d, grid = 16, size = 0) {
  const ns = "http://www.w3.org/2000/svg";
  const svg = document.createElementNS(ns, "svg");
  svg.setAttribute("viewBox", `0 0 ${grid} ${grid}`);
  svg.setAttribute("aria-hidden", "true");
  if (size) { svg.setAttribute("width", size); svg.setAttribute("height", size); }
  const path = document.createElementNS(ns, "path");
  path.setAttribute("d", d);
  svg.append(path);
  return svg;
}

// A menu is the system's own (the window shows it, panel.rs): its entries, each with what it
// does, shown at a place of the page; the window hands the place of the one picked back to
// `chosen`. An entry is `{label, run}`, with `marked` checked, `off` shown and not picked and
// `paste` handed the pasteboard's text; a submenu is `{label, items}`, a line `{line}`.
let offered = [];
function systemMenu(x, y, entries) {
  offered = [];
  const items = (list) => list.filter(Boolean).map((entry) => {
    if (entry.line) return { line: true };
    if (entry.items) return { label: entry.label, items: items(entry.items) };
    offered.push(entry.run || (() => {}));
    return { label: entry.label, marked: !!entry.marked, off: !!entry.off, paste: !!entry.paste };
  });
  const said = items(entries);
  post("m " + Math.round(x) + " " + Math.round(y) + " " + JSON.stringify(said));
}
// The same menu under `anchor`, its left edge on the anchor's.
function menuUnder(anchor, entries) {
  const box = anchor.getBoundingClientRect();
  systemMenu(box.left, box.bottom + 4, entries);
}
// What an entry's `run` answers as text goes back to the window (a terminal's Copy).
window.chosen = (place, text) => {
  const run = offered[place];
  offered = [];
  const answer = run ? run(text) : undefined;
  return typeof answer === "string" ? answer : null;
};
