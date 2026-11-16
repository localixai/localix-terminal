// Find in the terminal's output: the find row (page/find.html, its words and keys page/find.js)
// under the screen, as the editor's find row. The search addon reads the buffer shown: at a
// shell its scrollback and screen, in a full-screen program the screen alone, which has no
// scrollback. Every match is lit and the one shown more, case ignored. Esc gives the keys back
// to the terminal; what is typed in the row never reaches the program.
const search = new SearchAddon.SearchAddon({ highlightLimit: FOUND_MOST });
term.loadAddon(search);
// The matches' colours: the accent over the terminal's ground, at the editor's two steps (every
// match 25%, the one shown 60%, its edge the accent itself). The addon takes #rrggbb only, so the
// tokens are mixed here; they follow the theme and the inactive window's grey accent as the
// tokens do.
function decorations() {
  const accent = channels("var(--accent)"), ground = channels("var(--bg)");
  const over = (share) => hex(accent.map((c, i) => c * share + ground[i] * (1 - share)));
  const edge = hex(accent);
  return {
    matchBackground: over(0.25), activeMatchBackground: over(0.6), activeMatchBorder: edge,
    matchOverviewRuler: edge, activeMatchColorOverviewRuler: edge,
  };
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
findRow({ show: findShown, step: findStep, close: findClose });
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
