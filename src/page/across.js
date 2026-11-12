// A row that holds more than fits: it scrolls sideways with no bar of its own, and an edge with
// more past it fades out (`.across` and `data-more` in frame.css). Inlined into every page that
// has such a row, before the page's own script.

// Which edges of `row` have more past them (data-more: left, right, both; none: all fits).
function rowEdges(row) {
  const left = row.scrollLeft > 1, right = row.scrollLeft + row.clientWidth < row.scrollWidth - 1;
  const more = left && right ? "both" : left ? "left" : right ? "right" : "";
  if (more) row.dataset.more = more;
  else delete row.dataset.more;
}
// `item` brought into the part of `row` that shows, clear of an edge's fade, by the shortest
// way; at once, since it follows a press or an opening.
function rowInView(row, item) {
  if (!item) return;
  const it = item.getBoundingClientRect(), box = row.getBoundingClientRect();
  if (it.left < box.left + 24) row.scrollLeft += it.left - box.left - 24;
  else if (it.right > box.right - 24) row.scrollLeft += it.right - box.right + 24;
}
// `row` made such a row. A wheel that only turns up and down moves it sideways; a trackpad
// moves it as it is. As the row is laid out or its room changes, it stands where `left` says
// (a row drawn again keeps its place) and what `lit` selects stays in view.
function across(row, lit, left) {
  row.classList.add("across");
  row.addEventListener("wheel", (event) => {
    if (Math.abs(event.deltaY) <= Math.abs(event.deltaX) || row.scrollWidth <= row.clientWidth) return;
    row.scrollLeft += event.deltaY;
    event.preventDefault();
  }, { passive: false });
  row.addEventListener("scroll", () => rowEdges(row), { passive: true });
  new ResizeObserver(() => {
    if (left) row.scrollLeft = left;
    left = 0;
    if (lit) rowInView(row, row.querySelector(lit));
    rowEdges(row);
  }).observe(row);
  return row;
}
