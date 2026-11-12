"use strict";
// A file edited in a tab. The desk opens it (O: its text as the disk holds it, and the text the
// person left unsaved), shows a line of it (L), says when the disk changed under it (D) or the
// file went (G), and answers a save (S). The page posts what the person did as one JSON message ("k" and it): `save` with
// the text, `draft` with the text while it differs from the disk's, and `clean` once it does not.
// The text is the person's until they save it: nothing the disk does replaces what they typed.
const post = (text) => window.ipc && window.ipc.postMessage(text);
const say = (what) => post("k" + JSON.stringify(what));
const { EditorState, Compartment, Text, Transaction, Prec } = CM.pkg["@codemirror/state"];
const { EditorView, keymap, lineNumbers, highlightActiveLine, highlightActiveLineGutter, drawSelection,
  highlightSpecialChars, rectangularSelection, dropCursor } = CM.pkg["@codemirror/view"];
const { syntaxHighlighting, HighlightStyle, indentOnInput, bracketMatching, indentUnit } = CM.pkg["@codemirror/language"];
const { tags } = CM.pkg["@lezer/highlight"];

// A language's words, coloured by the page's own tokens: light and dark are the sheet's.
const coloured = HighlightStyle.define([
  { tag: [tags.keyword, tags.modifier, tags.operatorKeyword, tags.controlKeyword, tags.definitionKeyword, tags.moduleKeyword], color: "var(--s-word)" },
  { tag: [tags.string, tags.special(tags.string), tags.character, tags.regexp, tags.escape], color: "var(--s-text)" },
  { tag: [tags.comment, tags.meta, tags.processingInstruction], color: "var(--s-note)" },
  { tag: [tags.number, tags.bool, tags.null, tags.atom, tags.constant(tags.name), tags.standard(tags.name), tags.propertyName, tags.attributeName, tags.unit, tags.color], color: "var(--s-value)" },
  { tag: [tags.function(tags.variableName), tags.function(tags.propertyName), tags.macroName, tags.labelName], color: "var(--s-call)" },
  { tag: [tags.typeName, tags.className, tags.namespace, tags.self, tags.annotation], color: "var(--s-kind)" },
  { tag: [tags.tagName, tags.inserted], color: "var(--s-tag)" },
  { tag: [tags.invalid, tags.deleted], color: "var(--s-bad)" },
  { tag: tags.heading, fontWeight: "600" },
  { tag: tags.strong, fontWeight: "600" },
  { tag: tags.emphasis, fontStyle: "italic" },
  { tag: tags.strikethrough, textDecoration: "line-through" },
  { tag: [tags.link, tags.url], color: "var(--s-value)", textDecoration: "underline" },
]);

const bar = document.getElementById("bar"), said = document.getElementById("said");
const dark = matchMedia("(prefers-color-scheme: dark)");
const scheme = new Compartment();
// The text as the disk holds it, the file's name, and what was sent to be saved and not answered.
let disk = Text.empty, name = "", sending = null, gone = false, draftTimer = 0, wasDirty = false;

const dirty = () => gone || !view.state.doc.eq(disk);
const textOf = (text) => Text.of(text.split("\n"));

// The indent a file is written with: its first indented line's, else the language's own.
function indentOf(text, lang) {
  const found = /^(\t+|[ ]{2,8})\S/m.exec(text.slice(0, 20000));
  if (found) return found[1][0] === "\t" ? "\t" : " ".repeat(Math.min(found[1].length, 8));
  return ["go"].includes(lang) ? "\t" : ["rust", "python", "java", "cpp", "php", "swift", "kotlin", "csharp"].includes(lang) ? "    " : "  ";
}

function save() {
  if (sending || !dirty()) return true;
  sending = view.state.doc;
  say({ t: "save", text: sending.toString() });
  return true;
}

// What the bar says, and the buttons under it: each a word and what it does; none shuts it.
function tell(words, ...buttons) {
  bar.querySelectorAll("button").forEach((button) => button.remove());
  said.textContent = words || "";
  bar.hidden = !words;
  for (const [word, press] of buttons) {
    const button = document.createElement("button");
    button.type = "button";
    button.textContent = word;
    button.onclick = () => { tell(""); press(); view.focus(); };
    bar.append(button);
  }
  view.requestMeasure();
}

// The tab's title says the file is edited, and the desk keeps what is unsaved: told a moment
// after the person stops typing, and at once when the text is the disk's again.
function settle() {
  const now = dirty();
  document.title = (now ? "● " : "") + name;
  clearTimeout(draftTimer);
  if (now) draftTimer = setTimeout(() => say({ t: "draft", text: view.state.doc.toString() }), 400);
  else if (wasDirty) say({ t: "clean" });
  wasDirty = now;
}

// A fresh state for a file: its text, its language's words, its own indent, and nothing to undo.
function started(text, told) {
  const lang = CM.langs[told.lang], unit = indentOf(text, told.lang);
  return EditorState.create({
    doc: text,
    extensions: [
      lineNumbers(), highlightActiveLineGutter(), highlightSpecialChars(), CM.commands.history(), drawSelection(), dropCursor(),
      EditorState.allowMultipleSelections.of(true), rectangularSelection(), indentOnInput(), bracketMatching(),
      CM.close.closeBrackets(), highlightActiveLine(), CM.search.highlightSelectionMatches(), CM.search.search({ top: false }),
      syntaxHighlighting(coloured), lang || [], told.wrap ? EditorView.lineWrapping : [],
      indentUnit.of(unit), EditorState.tabSize.of(unit === "\t" ? 4 : unit.length),
      scheme.of(EditorView.darkTheme.of(dark.matches)),
      EditorView.contentAttributes.of({ "aria-label": told.name || "File" }),
      Prec.highest(keymap.of([{ key: "Mod-s", run: save }])),
      keymap.of([...CM.close.closeBracketsKeymap, ...CM.commands.defaultKeymap, ...CM.search.searchKeymap, ...CM.commands.historyKeymap, CM.commands.indentWithTab]),
      EditorView.updateListener.of((update) => { if (update.docChanged) settle(); }),
    ],
  });
}

const view = new EditorView({ parent: document.getElementById("e"), state: started("", {}) });
dark.addEventListener("change", () => view.dispatch({ effects: scheme.reconfigure(EditorView.darkTheme.of(dark.matches)) }));
// The person's font changed (Settings): the text is measured again.
addEventListener("desk-look", () => view.requestMeasure());
addEventListener("focus", () => view.focus());

// A long line scrolled sideways passes under the line numbers, which stand on the window's own
// ground with none of theirs: what is under them is cut away (the page's sheet, `data-cut`).
const scroller = view.scrollDOM;
let cut = 0;
scroller.addEventListener("scroll", () => {
  if (scroller.scrollLeft === cut) return;
  cut = scroller.scrollLeft;
  scroller.toggleAttribute("data-cut", cut > 0);
  scroller.style.setProperty("--cut", cut + "px");
  scroller.style.setProperty("--edge", cut + view.contentDOM.offsetLeft + "px");
}, { passive: true });

// The one stretch where `now` differs from the editor's text, as a change of it.
function differing(now) {
  const was = view.state.doc, a = was.toString(), b = now.toString();
  let start = 0;
  const most = Math.min(a.length, b.length);
  while (start < most && a.charCodeAt(start) === b.charCodeAt(start)) start++;
  let endA = a.length, endB = b.length;
  while (endA > start && endB > start && a.charCodeAt(endA - 1) === b.charCodeAt(endB - 1)) { endA--; endB--; }
  return { from: start, to: endA, insert: b.slice(start, endB) };
}

// The editor's text made the disk's: by another hand (kept out of what Undo steps through, the
// cursor where it was) or by the person's choice (a step of its own they can undo, never joined
// to what they typed a moment before).
function take(byPerson) {
  if (view.state.doc.eq(disk)) return;
  const how = byPerson ? [Transaction.userEvent.of("reload")] : [Transaction.addToHistory.of(false), Transaction.remote.of(true)];
  view.dispatch({ changes: differing(disk), annotations: how });
}

function conflict(words) {
  tell(words, ["Reload", () => { take(true); settle(); }], ["Keep Mine", () => {}]);
}

// Line `n` shown, the cursor at its start.
function show(n) {
  const line = view.state.doc.line(Math.max(1, Math.min(n, view.state.doc.lines)));
  view.dispatch({ selection: { anchor: line.from }, effects: EditorView.scrollIntoView(line.from, { y: "center" }) });
}
window.L = show;

// The file opened: `told.text` is the disk's, `told.draft` what the person left unsaved (and
// `told.moved`: the disk changed since they did).
window.O = (told) => {
  name = told.name;
  disk = textOf(told.text);
  gone = false;
  sending = null;
  view.setState(started(told.draft == null ? told.text : told.draft, told));
  tell("");
  if (told.draft != null && told.moved && dirty()) conflict("This file changed on disk since you edited it.");
  if (told.line) show(told.line);
  wasDirty = false;
  settle();
  view.focus();
};

// The disk changed under the file: a text the person did not touch follows it; one they did
// stays theirs, and they are asked.
window.D = (told) => {
  const clean = !dirty();
  disk = textOf(told.text);
  gone = false;
  if (clean) take(false);
  else if (dirty()) conflict("This file changed on disk.");
  else tell("");
  settle();
};

// The file went from the disk: the text stays, to be saved again.
window.G = () => {
  gone = true;
  tell("This file was deleted or moved.", ["Save", save]);
  settle();
};

// A save answered: kept (the disk's text is what was sent), or why not.
window.S = (told) => {
  const sent = sending;
  sending = null;
  if (told.ok && sent) {
    disk = sent;
    gone = false;
    tell("");
  } else if (!told.conflict) {
    tell("Could not save: " + (told.why || "the disk refused it") + ".", ["Try Again", save]);
  }
  settle();
};

post("r 0 0");
