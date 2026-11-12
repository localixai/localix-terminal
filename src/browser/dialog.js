// The window's dialog (dialog.rs): one look for everything the app asks or announces. Built
// element by element; every word is set as text. `DLG(dialog)` shows it, `DLG(null)` takes it
// away; the answer goes back as `dialog yes` or `dialog no`, a yes with what was typed into its
// fields after it as one JSON object.
const DLG_MARKS = {
  ask: [["path", { d: "M6 6.2a2 2 0 1 1 3.1 1.7c-.7.5-1.1.9-1.1 1.7" }], ["path", { d: "M8 11.9v.1" }]],
  app: [["rect", { x: 2.5, y: 3, width: 11, height: 10, rx: 1.6 }], ["path", { d: "M2.5 6h11M6 6v7" }]],
  up: [["path", { d: "M8 12.5v-9M4.5 7 8 3.5 11.5 7" }]],
};
const SVG = "http://www.w3.org/2000/svg";
function dlgNode(tag, name, text) {
  const node = document.createElement(tag);
  if (name) node.className = name;
  if (text != null) node.textContent = text;
  return node;
}
function dlgShape(tag, attrs, name) {
  const shape = document.createElementNS(SVG, tag);
  for (const [key, value] of Object.entries(attrs)) shape.setAttribute(key, value);
  if (name) shape.setAttribute("class", name);
  return shape;
}
function dlgGlyph(name) {
  const svg = dlgShape("svg", { viewBox: "0 0 16 16" });
  for (const [tag, attrs] of DLG_MARKS[name] ?? DLG_MARKS.ask) svg.append(dlgShape(tag, attrs));
  return svg;
}
// The stage: the mark in its ring, and for news the arrow and the version.
function dlgStage(dialog) {
  const stage = dlgNode("div", "stage");
  stage.setAttribute("aria-hidden", "true");
  const mark = dlgNode("span", "mark");
  const ring = dlgShape("svg", { viewBox: "0 0 100 100" }, "ring");
  const circle = { cx: 50, cy: 50, r: 46, pathLength: 1 };
  ring.append(dlgShape("circle", circle, "track"), dlgShape("circle", circle, "fill"));
  const tile = dlgNode("span", "tile");
  tile.append(dlgGlyph(dialog.mark));
  mark.append(ring, tile);
  stage.append(mark);
  if (dialog.versions) {
    const badge = dlgNode("span", "badge");
    badge.append(dlgGlyph("up"));
    mark.append(badge);
    const version = dlgNode("span", "ver");
    version.append(dlgNode("span", "from", dialog.versions[0]), dlgNode("span", "to", dialog.versions[1]));
    stage.append(version);
  }
  return stage;
}
let dlgSig = null;
function DLG(dialog) {
  const sig = JSON.stringify(dialog);
  if (sig === dlgSig) return;
  dlgSig = sig;
  document.getElementById("dlg")?.remove();
  if (!dialog) return;
  const root = dlgNode("div");
  root.id = "dlg";
  const card = dlgNode("div", "card" + (dialog.versions ? " news" : ""));
  card.dataset.tone = dialog.tone;
  card.setAttribute("role", "alertdialog");
  card.setAttribute("aria-modal", "true");
  card.setAttribute("aria-labelledby", "dlg-t");
  card.setAttribute("aria-describedby", "dlg-d");
  const says = dlgNode("div", "says");
  const title = dlgNode("h2", "", dialog.title);
  title.id = "dlg-t";
  const detail = dlgNode("p", "", dialog.detail);
  detail.id = "dlg-d";
  says.append(title, detail);
  card.append(dlgStage(dialog), says);
  if (dialog.notes.length) {
    const notes = dlgNode("section", "notes");
    notes.setAttribute("aria-label", dialog.heading);
    notes.tabIndex = 0;
    notes.append(dlgNode("h3", "", dialog.heading));
    for (const note of dialog.notes) {
      if (note.head != null) notes.append(dlgNode("h4", "", note.head));
      else if (note.item != null) notes.append(dlgNode("div", "pt", note.item));
      else notes.append(dlgNode("p", "", note.text));
    }
    card.append(notes);
  }
  // What it needs typed: a field each, its name over it.
  const inputs = dialog.fields.map((field) => {
    const input = dlgNode("input");
    input.type = field.secret ? "password" : "text";
    input.name = field.name;
    input.placeholder = field.hint;
    input.required = field.required;
    input.autocomplete = "off";
    input.spellcheck = false;
    return input;
  });
  if (inputs.length) {
    const fields = dlgNode("div", "fields");
    dialog.fields.forEach((field, at) => {
      const row = dlgNode("label");
      const box = dlgNode("span", "field");
      box.append(inputs[at]);
      row.append(dlgNode("span", "", field.label), box);
      fields.append(row);
    });
    card.append(fields);
  }
  const acts = dlgNode("div", "acts");
  const no = dlgNode("button", "btn", dialog.no);
  const yes = dlgNode("button", "btn go", dialog.yes);
  yes.id = "dlg-yes";
  no.addEventListener("click", () => window.ipc.postMessage("dialog no"));
  // A yes waits for what is required: the keys go to the first field left empty.
  yes.addEventListener("click", () => {
    const empty = inputs.find((input) => input.required && !input.value.trim());
    if (empty) return empty.focus();
    if (!inputs.length) return window.ipc.postMessage("dialog yes");
    const given = {};
    for (const input of inputs) if (input.value.trim()) given[input.name] = input.value.trim();
    window.ipc.postMessage("dialog yes " + JSON.stringify(given));
  });
  acts.append(no, yes);
  card.append(acts);
  root.append(card);
  document.body.append(root);
  // A dialog the person asked for takes the keys: in its first field, else on the button that
  // does it.
  if (dialog.asked) (inputs[0] ?? yes).focus();
}
// While a dialog is up the keys are its own: Escape is its other button, Return in a field is
// the one that does it, and Tab stays within it.
document.addEventListener("keydown", (event) => {
  const root = document.getElementById("dlg");
  if (!root) return;
  event.stopPropagation();
  if (event.key === "Escape") {
    event.preventDefault();
    window.ipc.postMessage("dialog no");
  } else if (event.key === "Enter" && event.target instanceof HTMLInputElement) {
    event.preventDefault();
    document.getElementById("dlg-yes")?.click();
  } else if (event.key === "Tab") {
    const stops = [...root.querySelectorAll("button,.notes,input")];
    const at = stops.indexOf(document.activeElement);
    event.preventDefault();
    stops[(at + (event.shiftKey ? stops.length - 1 : 1) + stops.length) % stops.length]?.focus();
  }
}, true);
