"use strict";
// The board in the window. The desk paints the whole board (B) whenever it changes, and says when
// it refused what the person did (E); what the person does is posted back as one JSON action
// ("k" and it), which the desk applies, keeps and paints again. Everything the board holds came
// from an agent or the person: it is set as text, never parsed as markup.
const post = (text) => window.ipc && window.ipc.postMessage(text);
const act = (action) => post("k" + JSON.stringify(action));
const COLUMNS = [["backlog", "Backlog"], ["todo", "Todo"], ["doing", "Doing"], ["review", "Review"], ["done", "Done"], ["discarded", "Discarded"]];
const ORDER = COLUMNS.map(([column]) => column);
const TITLES = Object.fromEntries(COLUMNS);
const EMPTY = {
  backlog: "New cards land here. Nobody works on one until you move it to Todo.",
  todo: "Nothing to do yet. Move a card here from Backlog, or let the agent plan.",
  doing: "Agents take cards from Todo and work on them here.",
  review: "Finished work waits here to be accepted or sent back.",
  done: "Accepted cards collect here.",
  discarded: "Nothing given up. Cards dropped from any column land here.",
};
const motion = matchMedia("(prefers-reduced-motion: reduce)");
let board = { key: "", project: "", limit: 1, now: 0, cards: [] };
let paintedAt = Date.now();
let opened = null;
let returning = false;
let removing = false;
let dragged = null;
let seen = null;
let boardSig = null, paintFrame = 0, paintTimer = 0, repaint = false, composing = false;
// Columns the person folded or opened; an empty Review or Done folds by itself until then.
const folds = new Map();

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
// A small picture, drawn as the window's strip draws its own.
function icon(d, size = 12) {
  const ns = "http://www.w3.org/2000/svg";
  const svg = document.createElementNS(ns, "svg");
  for (const [name, value] of [["width", size], ["height", size], ["viewBox", "0 0 12 12"], ["aria-hidden", "true"]]) svg.setAttribute(name, value);
  const path = document.createElementNS(ns, "path");
  path.setAttribute("d", d);
  svg.append(path);
  return svg;
}
const GLYPH = {
  close: "m3 3 6 6M9 3 3 9", check: "m2.5 6.3 2.3 2.2L9.5 3.5", back: "M4.5 3 2 5.5 4.5 8M2 5.5h5.5a2.5 2.5 0 0 1 0 5H6",
  fold: "M7.5 3 4.5 6l3 3", open: "M4.5 3l3 3-3 3",
  // For the empty columns, drawn larger: an idea, a list, someone at work, an eye, a seal.
  idea: "M4.4 8.6C3.4 7.9 2.8 6.9 2.8 5.7a3.2 3.2 0 0 1 6.4 0c0 1.2-.6 2.2-1.6 2.9v1H4.4zM4.6 11h2.8",
  list: "M4.5 3.5h5M4.5 6h5M4.5 8.5h5M2.5 3.5h0M2.5 6h0M2.5 8.5h0",
  work: "M6 1.8v1.4M6 8.8v1.4M1.8 6h1.4M8.8 6h1.4M3 3l1 1M8 8l1 1M9 3 8 4M4 8 3 9M6 4.2a1.8 1.8 0 1 0 0 3.6 1.8 1.8 0 0 0 0-3.6z",
  eye: "M1.2 6S3 2.8 6 2.8 10.8 6 10.8 6 9 9.2 6 9.2 1.2 6 1.2 6zM6 4.8a1.2 1.2 0 1 0 0 2.4 1.2 1.2 0 0 0 0-2.4z",
  seal: "M6 1.3l1.3 1 1.6-.1.5 1.5 1.3.9-.5 1.5.5 1.5-1.3.9-.5 1.5-1.6-.1L6 10.7l-1.3-1-1.6.1-.5-1.5-1.3-.9.5-1.5-.5-1.5 1.3-.9.5-1.5 1.6.1zM4.3 6.1l1.2 1.1 2.2-2.3",
  trash: "M2.3 3.4h7.4M4.7 3.4V2.3h2.6v1.1M3.3 3.4l.5 6.3h4.4l.5-6.3M5 5.2v2.8M7 5.2v2.8",
  shelf: "M2 4h8v5.7H2zM1.6 2.3h8.8V4H1.6zM4.6 6h2.8",
  chevron: "M3.5 4.8 6 7.2l2.5-2.4",
};
const EMPTY_GLYPH = { backlog: "idea", todo: "list", doing: "work", review: "eye", done: "seal", discarded: "shelf" };

const now = () => board.now + Math.floor((Date.now() - paintedAt) / 1000);
function age(then) {
  const secs = Math.max(0, now() - then);
  if (secs < 60) return "now";
  if (secs < 3600) return Math.floor(secs / 60) + "m";
  if (secs < 86400) return Math.floor(secs / 3600) + "h";
  return Math.floor(secs / 86400) + "d";
}
const ago = (then) => (age(then) === "now" ? "just now" : age(then) + " ago");
// The latest step of `card` that moved it, found from the end without copying its history.
function lastMove(card) {
  for (let at = card.history.length - 1; at >= 0; at--) if (card.history[at].to) return card.history[at];
  return null;
}
const since = (card) => {
  const moved = lastMove(card);
  return moved ? moved.at : 0;
};
const who = (card) => card.who || "agent";
// The same colour for a worker every time it shows, one of the system's; the person and the main
// agent in grey.
const PALETTE = ["#0a84ff", "#bf5af2", "#ff9f0a", "#30b0c7", "#ff375f", "#5e5ce6", "#34c759", "#a2845e"];
function hue(name) {
  if (name === "agent" || name === "person") return "var(--gray)";
  let hash = 7;
  for (const c of name) hash = (hash * 33 + c.charCodeAt(0)) >>> 0;
  return PALETTE[hash % PALETTE.length];
}
const avatar = (name) => el("span.av", { text: name === "person" ? "y" : name[0], style: `--h:${hue(name)}` });
// Who works on a card; in Todo, only whose it was, when it was someone's.
function owner(card) {
  const waiting = card.column === "todo" || card.column === "backlog";
  if (waiting && !card.who) return null;
  const name = who(card);
  return el("span.who", { title: waiting ? `Was ${name}'s` : `${name} has it` },
    avatar(name), waiting ? `was ${name}` : name);
}
// What the person said when they last sent the card back, while it waits in Todo.
function returned(card) {
  const last = lastMove(card);
  return card.column === "todo" && last && last.by === "person" && last.to === "todo" ? last.text : null;
}
const note = (kind, label, text) => el(`div.note.${kind}`, {}, el("b", { text: label + " " }), text);
const folded = (column) => folds.has(column) ? folds.get(column)
  : ["backlog", "review", "done", "discarded"].includes(column) && !board.cards.some((card) => card.column === column);

// What the person types is kept across paints: the field, its words and the caret.
function kept() {
  const field = document.activeElement;
  if (!field || !field.dataset) return null;
  if (field.dataset.keep) return { keep: field.dataset.keep, value: field.value, start: field.selectionStart, end: field.selectionEnd };
  if (field.dataset.id) return { card: field.dataset.id };
  return null;
}
function restore(held) {
  if (!held) return;
  if (held.card) {
    const card = document.querySelector(`.card[data-id="${held.card}"]`);
    if (card) card.focus({ preventScroll: true });
    return;
  }
  const field = document.querySelector(`[data-keep="${held.keep}"]`);
  if (!field) return;
  field.value = held.value;
  field.focus({ preventScroll: true });
  try { field.setSelectionRange(held.start, held.end); } catch {}
}

// The one move a card asks of the person, shown as it is pointed at.
function quick(card) {
  if (card.column === "review") return { label: "Accept", glyph: "check", action: { op: "accept", card: card.id } };
  if (card.column === "done") return { label: "Reopen", glyph: "back", plain: true, action: { op: "move", card: card.id, to: "todo" } };
  if (card.column === "backlog") return { label: "Move to Todo", glyph: "open", plain: true, action: { op: "move", card: card.id, to: "todo" } };
  if (card.blocked) return { label: "Unblock", glyph: "check", plain: true, action: { op: "unblock", card: card.id } };
  return null;
}

function cardView(card) {
  const back = returned(card);
  const offer = quick(card);
  const hasQuick = !!offer && card.column !== "review";
  const moved = since(card);
  const node = el(`div.card${opened === card.id ? ".sel" : ""}${seen && !seen.has(card.id) ? ".fresh" : ""}${hasQuick ? ".quick-on" : ""}`, {
    draggable: "true", tabindex: "0", role: "button", "data-id": card.id, "data-col": card.column, "aria-label": `${card.id}: ${card.title}`,
    "aria-keyshortcuts": "Enter Alt+ArrowLeft Alt+ArrowRight",
    onclick: () => open(card.id, false),
    oncontextmenu: (e) => { e.preventDefault(); menu(card, e.clientX, e.clientY); },
    onkeydown: (e) => keyOnCard(e, card),
  },
  card.blocked ? el("span.edge") : null,
  el("div.t", { text: card.title }),
  card.blocked ? note("blocked", "Blocked:", card.blocked) : null,
  back ? note("back", "Sent back:", back) : null,
  card.column === "review" && card.result ? note("done", "Handed in:", card.result) : null,
  card.column === "review" ? el("div.acts", {},
    el("button.act.ok", { title: "Accept", "aria-label": `Accept ${card.id}`,
      onclick: (e) => { e.stopPropagation(); act({ op: "accept", card: card.id }); } }, icon(GLYPH.check, 13)),
    el("button.act.no", { title: "Send back…", "aria-label": `Send ${card.id} back`,
      onclick: (e) => { e.stopPropagation(); open(card.id, true); } }, icon(GLYPH.back, 13))) : null,
  el("div.m", {}, el("span.id", { text: card.id }), owner(card), el("span.age", { text: age(moved), "data-at": moved })),
  offer && card.column !== "review" ? el(`button.quick${offer.plain ? ".plain" : ""}`, {
    tabindex: "-1", onclick: (e) => { e.stopPropagation(); act(offer.action); } }, icon(GLYPH[offer.glyph], 10), offer.label) : null);
  node.addEventListener("dragstart", (e) => {
    dragged = card.id;
    slotEl = el("div.slot");
    slotEl.style.height = `${node.offsetHeight}px`;
    e.dataTransfer.setData("text/plain", card.id);
    e.dataTransfer.effectAllowed = "move";
    requestAnimationFrame(() => node.classList.add("dragging"));
  });
  node.addEventListener("dragend", () => {
    dragged = null;
    node.classList.remove("dragging");
    slot();
    slotEl = null;
    if (repaint) queuePaint();
  });
  return node;
}

// Keys on a focused card: Enter opens it, ⌥← and ⌥→ move it a column, the arrows walk the board.
function keyOnCard(e, card) {
  // A key on a button inside the card is the button's own.
  if (e.target !== e.currentTarget) return;
  const step = e.key === "ArrowLeft" ? -1 : e.key === "ArrowRight" ? 1 : 0;
  if (e.key === "Enter" || e.key === " ") { e.preventDefault(); open(card.id, false); return; }
  if (e.altKey && step) {
    e.preventDefault();
    const to = ORDER[ORDER.indexOf(card.column) + step];
    if (to) { folds.set(to, false); act({ op: "move", card: card.id, to }); }
    return;
  }
  const here = e.currentTarget;
  if (e.key === "ArrowDown" || e.key === "ArrowUp") {
    e.preventDefault();
    const list = [...here.parentElement.querySelectorAll(".card")];
    const next = list[list.indexOf(here) + (e.key === "ArrowDown" ? 1 : -1)];
    if (next) next.focus();
  } else if (step) {
    e.preventDefault();
    // The next column that holds a card: an empty one is passed over, never a wall.
    const lanes = [...document.querySelectorAll(".lane")];
    let next = null;
    for (let at = lanes.indexOf(here.closest(".lane")) + step; lanes[at] && !next; at += step) next = lanes[at].querySelector(".card");
    if (next) next.focus();
  } else if (e.key === "Backspace") {
    e.preventDefault();
    open(card.id, false);
    arm();
  }
}

// The card in `list` the dragged one goes before, from where the pointer is. The slot already
// shown holds its place: the pointer passes a card's midpoint by 6px to move it past the card, and
// by 6px back to return, so a pointer resting on a midpoint does not make it flicker.
function before(list, y) {
  const cards = [...list.querySelectorAll(".card:not(.dragging)")];
  const here = slotEl && slotEl.parentElement === list;
  const next = cards.find((card) => {
    const box = card.getBoundingClientRect();
    const ahead = here && slotEl.compareDocumentPosition(card) & Node.DOCUMENT_POSITION_FOLLOWING;
    return y < box.top + box.height / 2 + (here ? (ahead ? 6 : -6) : 0);
  });
  return next ? next.dataset.id : null;
}
// Where the dragged card would land: the drag's one slot, moved between the cards, and taken out
// once it is dropped. It grows only when it comes into a column, not at each move within it.
let slotEl = null;
let slotAt = null;
function slot(list, next) {
  const at = list ? `${list.dataset.col}:${next}` : null;
  if (at === slotAt && (!slotEl || !list || slotEl.parentElement)) return;
  slotAt = at;
  for (const lane of document.querySelectorAll(".over")) if (!list || !lane.contains(list)) lane.classList.remove("over");
  if (!list) { if (slotEl) slotEl.remove(); return; }
  if (!slotEl) return;
  const entering = slotEl.parentElement !== list;
  const card = next && list.querySelector(`.card[data-id="${next}"]`);
  card ? list.insertBefore(slotEl, card) : list.append(slotEl);
  if (entering) {
    slotEl.classList.remove("enter");
    void slotEl.offsetWidth;
    slotEl.classList.add("enter");
  }
}
function droppable(node, column, list) {
  node.addEventListener("dragover", (e) => {
    if (!dragged) return;
    e.preventDefault();
    node.classList.add("over");
    if (list) slot(list, before(list, e.clientY));
  });
  node.addEventListener("dragleave", (e) => { if (!node.contains(e.relatedTarget)) { node.classList.remove("over"); if (list) slot(); } });
  node.addEventListener("drop", (e) => {
    e.preventDefault();
    const id = dragged || e.dataTransfer.getData("text/plain");
    const next = list ? before(list, e.clientY) : null;
    slot();
    node.classList.remove("over");
    if (!id || next === id) return;
    folds.set(column, false);
    act({ op: "move", card: id, to: column, before: next });
  });
}

function laneView([column, title]) {
  const cards = board.cards.filter((card) => card.column === column);
  if (folded(column)) {
    const rail = el("button.rail", { "data-col": column, title: `Show ${title}`, "aria-label": `Show ${title} (${cards.length})`,
      onclick: () => { folds.set(column, false); paint(); } },
    icon(GLYPH.open, 11), el("span.dot"), el("span.v", { text: title }), cards.length ? el("span.n", { text: String(cards.length) }) : null);
    droppable(rail, column, null);
    return rail;
  }
  const shown = column === "done"
    ? cards.map((card) => [since(card), card]).sort((a, b) => b[0] - a[0]).slice(0, 40).map(([, card]) => card)
    : cards;
  let pill = null;
  if (column === "doing") {
    pill = el("button.pill", { text: `${board.limit} each`, title: "Change how many cards a worker may hold in Doing",
      onclick: () => act({ op: "limit", limit: board.limit >= 3 ? 1 : board.limit + 1 }) });
  }
  const list = el("div.list", { "data-col": column }, shown.length ? shown.map(cardView) : el("div.empty", {}, icon(GLYPH[EMPTY_GLYPH[column]], 18), el("span", { text: EMPTY[column] })));
  const lane = el("section.lane", { "data-col": column, "aria-label": `${title}, ${cards.length}` },
    el("h2", { "aria-label": `${title}, ${cards.length}` }, el("span.dot"), title, el("span.n", { text: String(cards.length) }),
      el("span.end", {}, pill, el("button.fold", { title: `Fold ${title}`, "aria-label": `Fold ${title}`,
        onclick: () => { folds.set(column, true); paint(); } }, icon(GLYPH.fold, 11)))),
    list);
  droppable(lane, column, list);
  return lane;
}

function historyView(card) {
  return el("ul.history", {}, [...card.history].reverse().map((step) => el("li", {},
    avatar(step.by),
    el("span", {}, el("b", { text: step.by === "person" ? "You" : step.by }),
      step.to ? ` moved it to ${TITLES[step.to]}` : " wrote", ` · ${ago(step.at)}`),
    step.text ? el("span.said", { text: step.text }) : null)));
}

function save(card, field, value) {
  if ((card[field] || "") === value.trim() || (field === "title" && !value.trim())) return;
  act({ op: "edit", card: card.id, [field]: value });
}

// A box to write in, sent with its button or ⌘↩.
function writer(op, card, keep, label, button) {
  const box = el("textarea", { rows: 3, "data-keep": keep, "aria-label": label, placeholder: label });
  const send = () => {
    if (!box.value.trim()) return;
    act({ op, card: card.id, text: box.value.trim() });
    box.value = "";
    returning = false;
  };
  box.addEventListener("keydown", (e) => { if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) { e.preventDefault(); send(); } });
  return el("div", {}, box, el("div.row", { style: "margin-top:6px" },
    el("span.hint", { text: "⌘↩ sends", style: "margin-right:auto" }), el("button.btn.main", { text: button, onclick: send })));
}

const section = (label, ...kids) => el("div.sec", {}, el("label", { text: label }), ...kids);

// The open card's title as tall as its lines, measured once it is on the page.
function fitTitle() {
  const title = document.querySelector("aside .title");
  if (!title) return;
  title.style.height = "auto";
  title.style.height = title.scrollHeight + "px";
}
addEventListener("resize", fitTitle);

function inspector(card) {
  // The title wraps as it is long; Enter keeps it.
  const title = el("textarea.title", { rows: 1, "data-keep": "title", "aria-label": "Title" });
  title.value = card.title;
  title.addEventListener("input", fitTitle);
  title.addEventListener("keydown", (e) => { if (e.key === "Enter") { e.preventDefault(); title.blur(); } });
  title.addEventListener("blur", () => save(card, "title", title.value));
  const text = el("textarea", { rows: 4, "data-keep": "note", "aria-label": "Note", placeholder: "What it is about, and how to tell it is done" });
  text.value = card.note || "";
  text.addEventListener("blur", () => save(card, "note", text.value));
  // Where it is: the status, a click on it the system's menu of the columns.
  const status = el("button.status", { "data-col": card.column, title: "Move the card to another column", "aria-haspopup": "menu",
    onclick: (e) => { const at = e.currentTarget.getBoundingClientRect(); systemMenu(at.left, at.bottom + 4, moves(card)); } },
    el("span.dot"), TITLES[card.column], icon(GLYPH.chevron, 10));
  const trash = removing
    ? el("button.btn.danger", { onclick: () => { act({ op: "remove", card: card.id }); close(); } }, icon(GLYPH.trash, 12), "Delete")
    : el("button.btn.plain.ico", { title: "Delete card", "aria-label": `Delete ${card.id}`, onclick: arm }, icon(GLYPH.trash, 13));
  return [
    el("div.top", {}, el("span.id", { text: card.id }), status, owner(card), el("span.gap"), trash,
      el("button.btn.plain.ico", { "aria-label": "Close", title: "Close (Esc)", onclick: close }, icon(GLYPH.close))),
    el("div.body", {},
      title,
      card.blocked ? section("Blocked", el("div.box.blocked", { text: card.blocked }),
        el("div.row", { style: "margin-top:6px" }, el("button.btn", { text: "Unblock", onclick: () => act({ op: "unblock", card: card.id }) }))) : null,
      card.column === "review" ? section("Handed In", card.result ? el("div.box", { text: card.result }) : null,
        el("div.row", { style: "margin-top:8px" }, el("button.btn.go", { onclick: () => act({ op: "accept", card: card.id }) }, icon(GLYPH.check, 11), "Accept"),
          returning ? null : el("button.btn", { text: "Send Back…", onclick: () => { returning = true; paint(); } }))) : null,
      card.column === "review" && returning ? section("What Is Still Wrong", writer("return", card, "return", "The agent reads this; the card goes back to the top of Todo.", "Send Back")) : null,
      section("Note", text),
      card.tab ? section("About", el("div.box", { text: `Tab ${card.tab}` })) : null,
      section("Comment", writer("comment", card, "comment", "The agent reads this.", "Comment")),
      section("History", historyView(card))),
  ];
}

// A menu as the system's (the window shows it, panel.rs): its items, each with what it does;
// the window hands the one picked back to `chosen`. A submenu is `{label, items}`, a line `{line}`.
let offered = [];
function systemMenu(x, y, entries) {
  offered = [];
  const items = (list) => list.filter(Boolean).map((entry) => {
    if (entry.line) return { line: true };
    if (entry.items) return { label: entry.label, items: items(entry.items) };
    offered.push(entry.run || (() => {}));
    return { label: entry.label, marked: !!entry.marked, off: !!entry.off };
  });
  const said = items(entries);
  post("m " + Math.round(x) + " " + Math.round(y) + " " + JSON.stringify(said));
}
window.chosen = (place) => { const run = offered[place]; offered = []; if (run) run(); return null; };

// Where a card can go: its column checked.
const moves = (card) => COLUMNS.map(([column, name]) => ({
  label: name, marked: column === card.column, off: column === card.column,
  run: () => { folds.set(column, false); act({ op: "move", card: card.id, to: column }); },
}));

// The card's menu: what the person may do with it, where it can go, and giving it up.
function menu(card, x, y) {
  const closed = card.column === "done" || card.column === "discarded";
  systemMenu(x, y, [
    { label: "Open", run: () => open(card.id, false) },
    card.column === "review" ? { label: "Accept", run: () => act({ op: "accept", card: card.id }) } : null,
    card.column === "review" ? { label: "Send Back…", run: () => open(card.id, true) } : null,
    card.blocked ? { label: "Unblock", run: () => act({ op: "unblock", card: card.id }) } : null,
    { line: true },
    { label: "Move To", items: moves(card) },
    closed ? null : { label: "Discard", run: () => { folds.set("discarded", false); act({ op: "move", card: card.id, to: "discarded" }); } },
    { line: true },
    { label: "Delete…", run: () => { open(card.id, false); arm(); } },
  ]);
}

// Deleting asks twice: the trash turns into a Delete button for a few seconds.
let disarm = 0;
function arm() {
  removing = true;
  clearTimeout(disarm);
  disarm = setTimeout(() => { removing = false; paint(); }, 4000);
  paint();
}

function open(id, toReturn) {
  if (opened !== id) removing = false;
  opened = id;
  returning = !!toReturn;
  paint();
  if (toReturn) requestAnimationFrame(() => { const box = document.querySelector('[data-keep="return"]'); if (box) box.focus(); });
}
function close() {
  const was = opened;
  opened = null; returning = false; removing = false;
  paint();
  const card = was && document.querySelector(`.card[data-id="${was}"]`);
  if (card) card.focus({ preventScroll: true });
}

// The board drawn again, each card gliding from where it was to where it is now.
// Full snapshots that arrive together show only their newest state. A native drag or IME
// composition owns its DOM until it finishes: an agent's update must not replace its target.
function queuePaint() {
  repaint = true;
  if (paintFrame || dragged || composing) return;
  paintFrame = requestAnimationFrame(paint);
  paintTimer = setTimeout(paint, 150);
}
document.addEventListener("compositionstart", () => { composing = true; });
document.addEventListener("compositionend", () => {
  composing = false;
  if (repaint) queuePaint();
});
function paint() {
  cancelAnimationFrame(paintFrame);
  clearTimeout(paintTimer);
  paintFrame = paintTimer = 0;
  if (dragged || composing) { repaint = true; return; }
  repaint = false;
  const held = kept();
  // Each column's scroll and the open card's, put back once they are drawn again.
  const scrolled = new Map([...document.querySelectorAll(".list[data-col]")].map((list) => [list.dataset.col, list.scrollTop]));
  const inspected = document.querySelector("aside .body");
  const inspectedAt = inspected && inspected.scrollTop;
  const inspectedId = document.getElementById("card").dataset.open;
  const before = new Map([...document.querySelectorAll(".card[data-id]")].map((card) => [card.dataset.id, card.getBoundingClientRect()]));
  const count = (column) => board.cards.filter((card) => card.column === column).length;
  // How far the work is: the ideas in Backlog are not work yet, and what was given up is not.
  const all = board.cards.length - count("backlog") - count("discarded"), done = count("done");
  document.getElementById("h").textContent = board.project ? `Board · ${board.project}` : "Board";
  document.getElementById("ring").style.strokeDashoffset = String(43.98 * (1 - (all ? done / all : 0)));
  document.getElementById("sub").textContent = all ? `${done} of ${all} done` : "No cards yet";
  document.getElementById("lanes").replaceChildren(...COLUMNS.map(laneView));
  for (const list of document.querySelectorAll(".list[data-col]")) list.scrollTop = scrolled.get(list.dataset.col) || 0;
  seen = new Set(board.cards.map((card) => card.id));
  if (!motion.matches) {
    // One curve for every card that moves, read once: a style read per card would lay the
    // board out again for each.
    const easing = getComputedStyle(document.documentElement).getPropertyValue("--spring").trim() || "ease-out";
    for (const card of document.querySelectorAll(".card[data-id]")) {
      const was = before.get(card.dataset.id);
      if (!was) continue;
      const is = card.getBoundingClientRect();
      const dx = was.left - is.left, dy = was.top - is.top;
      if (Math.abs(dx) > 1 || Math.abs(dy) > 1) {
        card.animate([{ transform: `translate(${dx}px,${dy}px)` }, { transform: "none" }], { duration: 260, easing });
      }
    }
  }
  const aside = document.getElementById("card");
  const card = board.cards.find((each) => each.id === opened);
  if (!card) opened = null;
  aside.classList.toggle("on", !!card);
  document.body.classList.toggle("inspecting", !!card);
  aside.setAttribute("aria-hidden", card ? "false" : "true");
  aside.replaceChildren(...(card ? inspector(card) : []));
  aside.dataset.open = card ? card.id : "";
  fitTitle();
  // The same card still open keeps its place in what it shows; another starts at the top.
  const body = card && document.querySelector("aside .body");
  if (body && inspectedId === card.id) body.scrollTop = inspectedAt;
  restore(held);
}

// The desk paints the board whole.
function B(state) {
  board = state;
  paintedAt = Date.now();
  const next = JSON.stringify([state.key, state.project, state.limit, state.cards]);
  if (next === boardSig) return;
  boardSig = next;
  queuePaint();
}
// The desk refused what the person did, saying why.
let toastTimer = 0;
function E(why) {
  const toast = document.getElementById("toast");
  toast.textContent = why;
  toast.classList.add("on");
  later();
}
// The notice waits while the pointer is on it, and goes on a click.
function later() {
  const toast = document.getElementById("toast");
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => toast.classList.remove("on"), 4000);
}
{
  const toast = document.getElementById("toast");
  toast.addEventListener("mouseenter", () => clearTimeout(toastTimer));
  toast.addEventListener("mouseleave", later);
  toast.addEventListener("click", () => { clearTimeout(toastTimer); toast.classList.remove("on"); });
}
document.getElementById("add").addEventListener("keydown", (e) => {
  if (e.key === "Escape") { e.target.value = ""; e.target.blur(); return; }
  if (e.key !== "Enter" || !e.target.value.trim()) return;
  // A new card is an idea first: the person moves it to Todo when it is to be done.
  act({ op: "add", title: e.target.value.trim(), to: "backlog" });
  e.target.value = "";
});
// The ages move on between paints.
setInterval(() => {
  for (const node of document.querySelectorAll(".age[data-at]")) node.textContent = age(Number(node.dataset.at));
}, 30000);
addEventListener("keydown", (e) => {
  const typing = /^(INPUT|TEXTAREA)$/.test((document.activeElement || {}).tagName || "");
  if (e.key === "Escape") {
    if (typing) document.activeElement.blur();
    if (opened) close();
  } else if ((e.code === "KeyN" || e.key.toLowerCase() === "n") && !typing && !e.metaKey && !e.ctrlKey && !e.altKey) {
    e.preventDefault();
    document.getElementById("add").focus();
  }
});
paint();
// Ready: the desk paints the board.
post("r 0 0");
