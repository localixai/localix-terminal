// Find over a web page (deck_find.rs): the row is the browser toolbar's, under the address. Each
// step goes to the window (`find type|next|previous #accent query`, `find close`), which has the
// page find and tells the row what it found (`FS`): the tab whose row it is, its query and the
// match shown of how many. The accent goes with each step, since a page cannot read it.
let findTab = null;
function pageFindShown(on) {
  findBar.hidden = !on;
  document.documentElement.toggleAttribute("data-find", on);
}
function pageFindPost(step) {
  window.ipc.postMessage("find " + step + " " + hex(channels("var(--accent)")) + " " + findField.value);
}
findRow({
  show: pageFindShown,
  step: (back, typing) => pageFindPost(typing ? "type" : back ? "previous" : "next"),
  close: () => {
    pageFindShown(false);
    findCount.textContent = "";
    window.ipc.postMessage("find close");
  },
});
// The page shown's find, as the window keeps it: its row open with its query and what it found
// (none counted yet: the count stays as it was), or no row (null).
function FS(state) {
  const was = findTab;
  findTab = state ? state.tab : null;
  pageFindShown(!!state);
  if (!state) return void (findCount.textContent = "");
  if (state.tab !== was) findField.value = state.query;
  if (!state.query) findCount.textContent = "";
  else if (state.of !== null) findCount.textContent = counted(state.at, state.of);
}
