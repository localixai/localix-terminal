// The find row (page/find.html), one for every page that has one: the terminal's, under its
// screen, and the browser toolbar's, over a web page. Its words and its keys are here, so the two
// cannot drift; each page says what showing the row, a step and closing it do there
// (`findRow`). The window's Find (⌘F), Find Next (⌘G) and Find Previous (⇧⌘G) call `findText`
// with the step. In the field Return and Shift+Return walk the matches and Esc closes the row.
const FOUND_MOST = 1000;
const findBar = document.getElementById("find");
const findField = document.getElementById("find-q");
const findCount = document.getElementById("find-n");
let findSteps = null;
function finding() {
  return !findBar.hidden;
}
// Where the match shown is of how many: "3 of 12"; past the most a page lights, "1,000+".
function counted(index, count) {
  if (!count) return "No results";
  const many = count >= FOUND_MOST ? FOUND_MOST.toLocaleString("en-US") + "+" : String(count);
  if (index < 0) return count >= FOUND_MOST ? many : count + " found";
  return (index + 1).toLocaleString("en-US") + " of " + many;
}
// A colour of the page's tokens as its three channels, read through an element: what a token
// resolves to (the system's accent, the inactive window's grey) is known only once drawn.
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
// What the window's Find, Find Next and Find Previous do: open the row with its query selected
// to type over, and with one in it show its matches; the next or previous one.
function findText(step) {
  if (!findSteps) return;
  const opening = !finding();
  if (opening) findSteps.show(true);
  if (step === "open" || opening) {
    findField.focus();
    findField.select();
  }
  if (step === "open") {
    if (opening) findSteps.step(false, true);
    return;
  }
  findSteps.step(step === "previous", false);
}
// The row's own keys and buttons, given what a step (`back`, as the query is `typing`), showing
// the row and closing it do in this page.
function findRow(steps) {
  findSteps = steps;
  findField.addEventListener("input", () => steps.step(false, true));
  findField.addEventListener("keydown", (event) => {
    if (event.isComposing) return;
    if (event.key === "Enter") steps.step(event.shiftKey, false);
    else if (event.key === "Escape") steps.close();
    else return;
    event.preventDefault();
  });
  document.getElementById("find-previous").addEventListener("click", () => findText("previous"));
  document.getElementById("find-next").addEventListener("click", () => findText("next"));
  document.getElementById("find-close").addEventListener("click", () => steps.close());
  // A press in the row stays there, and one on a button leaves the keys in the field. Its menu
  // is not the page's (a terminal's Paste types to the program).
  findBar.addEventListener("mousedown", (event) => {
    event.stopPropagation();
    if (event.target !== findField) event.preventDefault();
  });
  findBar.addEventListener("contextmenu", (event) => {
    event.stopPropagation();
    event.preventDefault();
  });
}
