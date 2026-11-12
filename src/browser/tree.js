// The side's tree of files (tree.rs): the shown project's folder with the entries of its open
// folders, as the app last told them. The page holds what it was told and is given only what
// changed; it draws only the rows in view, each one height, so a folder of thousands costs what
// a screenful does. What the person does is said to the window (`tree open src`), which answers
// with the tree as it then is: a folder shut closes at once, one opened fills as its entries come.
if (document.documentElement.dataset.part === "side") {
  const ROW = 24, OVER = 8, INDENT = 12;
  const head = document.getElementById("fh"), box = document.getElementById("ft"), sheet = document.getElementById("fr");
  const say = (what) => window.ipc.postMessage("tree " + what);
  // What the page holds: the project's key, the open folders' paths, and each one's entries.
  let model = null;
  // The rows from top to bottom, the path of the one picked, and the rows drawn by their paths.
  let flat = [], pick = null, made = 0;
  const drawn = new Map();
  // Folders asked to open whose entries have not come: their disclosure turns at once.
  const opening = new Map();
  head.querySelector(".ch").append(glyph("down"));
  const settled = (path) => {
    clearTimeout(opening.get(path));
    opening.delete(path);
  };
  function flatten() {
    flat = [];
    const walk = (path, depth) => {
      const dir = model.dirs.get(path);
      if (!dir) return;
      const under = path ? path + "/" : "";
      for (const name of dir.d) {
        const p = under + name, open = model.open.has(p) && model.dirs.has(p);
        flat.push({ p, name, depth, dir: true, open });
        if (open) walk(p, depth + 1);
      }
      for (const [name, icon] of dir.f) flat.push({ p: under + name, name, depth, icon });
      // No name holds a line break: this row's key is no entry's path.
      if (dir.m) flat.push({ p: under + "\n", name: dir.m.toLocaleString("en-US") + " more", depth, more: true });
    };
    if (model) walk("", 0);
    if (pick !== null && !flat.some((item) => item.p === pick)) pick = null;
  }
  // Drawn at most once a frame; a concealed window, whose frames WebKit may hold, still draws.
  let frame = 0, late = 0;
  function draw() {
    if (frame) return;
    frame = requestAnimationFrame(paint);
    late = setTimeout(paint, 150);
  }
  function paint() {
    cancelAnimationFrame(frame);
    clearTimeout(late);
    frame = late = 0;
    if (!document.documentElement.hasAttribute("data-files")) return;
    if (!flat.length) {
      drawn.clear();
      sheet.style.height = "";
      box.removeAttribute("aria-activedescendant");
      return void sheet.replaceChildren(...(model ? [emptied("No Files")] : []));
    }
    sheet.querySelector(".empty")?.remove();
    sheet.style.height = flat.length * ROW + "px";
    const from = Math.max(0, Math.floor(box.scrollTop / ROW) - OVER);
    const to = Math.min(flat.length, Math.ceil((box.scrollTop + box.clientHeight) / ROW) + OVER);
    const wanted = new Set();
    let active = "";
    for (let at = from; at < to; at++) {
      const item = flat[at];
      wanted.add(item.p);
      let row = drawn.get(item.p);
      if (!row) {
        row = entry();
        drawn.set(item.p, row);
        sheet.append(row);
      }
      patch(row, item, at);
      if (item.p === pick) active = row.id;
    }
    for (const [path, row] of drawn) {
      if (wanted.has(path)) continue;
      row.remove();
      drawn.delete(path);
    }
    if (active) box.setAttribute("aria-activedescendant", active);
    else box.removeAttribute("aria-activedescendant");
  }
  function entry() {
    const row = document.createElement("div");
    row.id = "fe" + ++made;
    for (const part of ["ch", "fi", "fn"]) row.append(Object.assign(document.createElement("span"), { className: part }));
    return row;
  }
  // A row set from its entry: only what differs is written, so a row under the pointer stays.
  function patch(row, item, at) {
    const set = (name, value) => {
      if (value === null) row.removeAttribute(name);
      else if (row.getAttribute(name) !== value) row.setAttribute(name, value);
    };
    const top = at * ROW + "px", pad = 4 + item.depth * INDENT + "px";
    if (row.style.top !== top) row.style.top = top;
    if (row.style.paddingLeft !== pad) row.style.paddingLeft = pad;
    const kind = item.more ? "fe more" : "fe";
    if (row.className !== kind) row.className = kind;
    const [turn, picture, name] = row.children;
    if (!!item.dir !== !!turn.firstChild) turn.replaceChildren(...(item.dir ? [glyph("down")] : []));
    const open = item.dir && (item.open || opening.has(item.p));
    const look = item.more ? "" : item.dir ? (open ? "folder-open" : "folder") : item.icon;
    if (picture.dataset.i !== look) {
      picture.dataset.i = look;
      picture.replaceChildren(...(look ? [pictured(look)] : []));
    }
    if (name.textContent !== item.name) name.textContent = item.name;
    set("role", item.more ? "none" : "treeitem");
    set("aria-level", item.more ? null : String(item.depth + 1));
    set("aria-expanded", item.dir ? String(open) : null);
    set("aria-selected", item.more ? null : String(item.p === pick));
    set("data-pick", item.p === pick ? "" : null);
    set("title", item.more ? null : item.p);
    row.dataset.p = item.p;
  }
  const itemOf = (node) => {
    const path = node instanceof Element ? node.closest(".fe")?.dataset.p : undefined;
    const item = path === undefined ? undefined : flat.find((one) => one.p === path);
    return item && !item.more ? item : undefined;
  };
  function toggle(item) {
    if (item.open) {
      for (const path of [...model.open]) if (path === item.p || path.startsWith(item.p + "/")) model.open.delete(path);
      settled(item.p);
      flatten();
      say("shut " + item.p);
    } else if (!opening.has(item.p)) {
      opening.set(item.p, setTimeout(() => {
        opening.delete(item.p);
        draw();
      }, 1500));
      say("open " + item.p);
    }
    draw();
  }
  // The pick moved to row `at`, and the tree scrolled so that it shows.
  function pickAt(at) {
    const item = flat[at];
    if (!item || item.more) return;
    pick = item.p;
    const top = at * ROW;
    if (top < box.scrollTop) box.scrollTop = top;
    else if (top + ROW > box.scrollTop + box.clientHeight) box.scrollTop = top + ROW - box.clientHeight;
    draw();
  }
  // The pictures (file_icons.rs), each a drawing given as text and read as data, never as script.
  // One copy of each is kept out of sight, made the first time a row names it; a row's own picture
  // points at it, so a thousand files of one kind are one drawing.
  const NS = "http://www.w3.org/2000/svg", kept = document.getElementById("fp"), placed = new Set();
  let pictures = {};
  function pictured(name) {
    const mark = document.createElementNS(NS, "svg"), use = document.createElementNS(NS, "use");
    if (!placed.has(name)) {
      const drawing = new DOMParser().parseFromString(pictures[name] || "", "image/svg+xml").documentElement;
      if (drawing.nodeName !== "svg") return mark;
      const symbol = document.createElementNS(NS, "symbol");
      symbol.id = "fi-" + name;
      symbol.setAttribute("viewBox", drawing.getAttribute("viewBox") || "0 0 32 32");
      symbol.append(...[...drawing.childNodes].map((node) => document.importNode(node, true)));
      kept.append(symbol);
      placed.add(name);
    }
    use.setAttribute("href", "#fi-" + name);
    mark.append(use);
    return mark;
  }
  window.FI = (given) => {
    pictures = given;
    for (const row of drawn.values()) delete row.children[1].dataset.i;
    draw();
  };
  // Whether the files show.
  window.FO = (shown) => {
    document.documentElement.toggleAttribute("data-files", shown);
    head.setAttribute("aria-expanded", String(shown));
    titled(head, shown ? "Hide the files" : "Show the files");
    if (shown) draw();
  };
  // The tree as told: whole, or what changed of the one the page holds; none takes it away.
  window.TR = (told) => {
    if (!told) {
      model = null;
      for (const path of [...opening.keys()]) settled(path);
    } else {
      const other = !model || model.k !== told.k;
      if (other && !told.whole) return;
      if (other) {
        pick = null;
        box.scrollTop = 0;
        for (const path of [...opening.keys()]) settled(path);
      }
      if (told.whole) model = { k: told.k, dirs: new Map() };
      model.open = new Set(told.open);
      for (const dir of told.dirs) model.dirs.set(dir.p, dir);
      for (const path of [...model.dirs.keys()]) if (!model.open.has(path)) model.dirs.delete(path);
      for (const path of [...opening.keys()]) if (model.open.has(path)) settled(path);
      box.setAttribute("aria-label", told.name ? "Files of " + told.name : "Files");
    }
    flatten();
    draw();
  };
  head.addEventListener("click", () => say("files"));
  head.addEventListener("keydown", (event) => {
    if (event.key !== "Enter" && event.key !== " ") return;
    event.preventDefault();
    say("files");
  });
  box.addEventListener("scroll", draw, { passive: true });
  new ResizeObserver(draw).observe(box);
  // A click picks an entry and opens or shuts a folder; a double click opens a file.
  box.addEventListener("click", (event) => {
    const item = itemOf(event.target);
    pick = item ? item.p : null;
    if (item?.dir) toggle(item);
    else draw();
  });
  box.addEventListener("dblclick", (event) => {
    const item = itemOf(event.target);
    if (item && !item.dir) say("act " + item.p);
  });
  // The menu of the entry under the pointer, or of the project's folder where there is none.
  box.addEventListener("contextmenu", (event) => {
    event.preventDefault();
    if (!model) return;
    const item = itemOf(event.target);
    pick = item ? item.p : null;
    draw();
    say("menu " + Math.round(event.clientX) + " " + Math.round(event.clientY) + (item ? " " + item.p : ""));
  });
  box.addEventListener("focus", () => {
    if (pick === null) pickAt(0);
  });
  // The keyboard: the arrows up and down move the pick, right opens a folder or goes into it,
  // left shuts it or goes to the folder it is in; Return or Space opens a file or turns a folder.
  box.addEventListener("keydown", (event) => {
    if (event.metaKey || event.ctrlKey || event.altKey || !flat.length) return;
    const at = flat.findIndex((item) => item.p === pick), item = flat[at];
    const step = (by) => {
      let to = at + by;
      while (flat[to]?.more) to += by;
      return flat[to] ? to : at;
    };
    const last = flat.findLastIndex((one) => !one.more);
    let to;
    if (event.key === "ArrowDown") to = at < 0 ? 0 : step(1);
    else if (event.key === "ArrowUp") to = at < 0 ? 0 : step(-1);
    else if (event.key === "Home") to = 0;
    else if (event.key === "End") to = last;
    else if (event.key === "ArrowRight" && item?.dir) to = item.open ? (flat[at + 1]?.depth > item.depth ? step(1) : at) : (toggle(item), at);
    else if (event.key === "ArrowLeft" && item) {
      if (item.dir && item.open) toggle(item), to = at;
      else to = flat.findLastIndex((one, place) => place < at && one.depth < item.depth);
    } else if ((event.key === "Enter" || event.key === " ") && item) {
      if (item.dir) toggle(item);
      else say("act " + item.p);
      to = at;
    } else if (event.key === "Escape") return void box.blur();
    else return;
    event.preventDefault();
    if (to >= 0 && to !== at) pickAt(to);
  });
}
