// Find in the terminal's output: a row under the screen (#find), as the editor's find row, opened
// by the window's Find (⌘F), Find Next (⌘G) and Find Previous (⇧⌘G), which call `findText` with
// the step; the page keeps the query and says the count. The search addon reads the buffer
// shown: at a shell its scrollback and screen, in a full-screen program the screen alone, which
// has no scrollback. Every match is lit and the one shown more, case ignored. In the field
// Return and Shift+Return walk the matches and Esc closes the row, giving the keys back to the
// terminal; what is typed there never reaches the program.
const FOUND_MOST = 1000;
const search = new SearchAddon.SearchAddon({ highlightLimit: FOUND_MOST });
term.loadAddon(search);
const findBar = document.getElementById("find");
const findField = document.getElementById("find-q");
const findCount = document.getElementById("find-n");
function finding() {
  return !findBar.hidden;
}
// The matches' colours: the accent over the terminal's ground, at the editor's two steps (every
// match 25%, the one shown 60%, its edge the accent itself). The addon takes #rrggbb only, so the
// tokens are read through an element and mixed here; they follow the theme and the inactive
// window's grey accent as the tokens do.
const tint = document.createElement("i");
tint.style.cssText = "display:none";
document.documentElement.append(tint);
function channels(colour) {
  tint.style.color = colour;
  const said = getComputedStyle(tint).color || "";
  const n = (said.match(/[\d.]+/g) || ["0", "0", "0"]).slice(0, 3).map(Number);
  // `color(srgb r g b)` gives each channel from 0 to 1.
  return said.startsWith("color(") ? n.map((c) => c * 255) : n;
}
const hex = (n) => "#" + n.map((c) => Math.round(c).toString(16).padStart(2, "0")).join("");
function decorations() {
  const accent = channels("var(--accent)"), ground = channels("var(--bg)");
  const over = (share) => hex(accent.map((c, i) => c * share + ground[i] * (1 - share)));
  const edge = hex(accent);
  return {
    matchBackground: over(0.25), activeMatchBackground: over(0.6), activeMatchBorder: edge,
    matchOverviewRuler: edge, activeMatchColorOverviewRuler: edge,
  };
}
// Where the match shown is of how many: "3 of 12"; past the most the addon lights, "1,000+".
function counted(index, count) {
  if (!count) return "No results";
  const many = count >= FOUND_MOST ? FOUND_MOST.toLocaleString("en-US") + "+" : String(count);
  if (index < 0) return count >= FOUND_MOST ? many : count + " found";
  return (index + 1).toLocaleString("en-US") + " of " + many;
}
search.onDidChangeResults(({ resultIndex, resultCount }) => {
  if (findField.value) findCount.textContent = counted(resultIndex, resultCount);
});
// One step: from the match shown (`back` to the one before), or, as the query is typed, from
// where it stands. An empty field lights nothing.
function findStep(back, typing) {
  const query = findField.value;
  if (!query) {
    search.clearDecorations();
    term.clearSelection();
    findCount.textContent = "";
    return;
  }
  const options = { caseSensitive: false, incremental: typing, decorations: decorations() };
  if (back) search.findPrevious(query, options);
  else search.findNext(query, options);
}
// The row opens, or closes: the screen ends over it, so it covers no line (the pty is resized
// with the grid, once each way).
function findShown(on) {
  findBar.hidden = !on;
  document.documentElement.toggleAttribute("data-find", on);
  queueFit();
}
function findClose() {
  findShown(false);
  search.clearDecorations();
  term.clearSelection();
  findCount.textContent = "";
  term.focus();
}
// What the window's Find, Find Next and Find Previous do: open the row with its query selected
// to type over, and with one in it show its matches; the next or previous one.
function findText(step) {
  const opening = !finding();
  if (opening) findShown(true);
  if (step === "open" || opening) {
    findField.focus();
    findField.select();
  }
  if (step === "open") {
    if (opening) findStep(false, true);
    return;
  }
  findStep(step === "previous", false);
}
findField.addEventListener("input", () => findStep(false, true));
findField.addEventListener("keydown", (event) => {
  if (event.isComposing) return;
  if (event.key === "Enter") findStep(event.shiftKey, false);
  else if (event.key === "Escape") findClose();
  else return;
  event.preventDefault();
});
document.getElementById("find-previous").addEventListener("click", () => findText("previous"));
document.getElementById("find-next").addEventListener("click", () => findText("next"));
document.getElementById("find-close").addEventListener("click", findClose);
// A press in the row stays there: the page's own press goes to the terminal, and one on a button
// leaves the keys in the field. Its menu is not the terminal's, whose Paste types to the program.
findBar.addEventListener("mousedown", (event) => {
  event.stopPropagation();
  if (event.target !== findField) event.preventDefault();
});
findBar.addEventListener("contextmenu", (event) => {
  event.stopPropagation();
  event.preventDefault();
});
// Another screen (a full-screen program came or went) is searched anew from its top, and new
// colours (the theme, the window inactive) light the matches again.
function findAgain() {
  if (!finding() || !findField.value) return;
  search.clearDecorations();
  findStep(false, false);
}
term.buffer.onBufferChange(findAgain);
matchMedia("(prefers-color-scheme: dark)").addEventListener("change", findAgain);
new MutationObserver(findAgain).observe(document.documentElement, {
  attributes: true, attributeFilter: ["data-inactive"],
});
