// The side's tree of files (tree.rs): the shown project's folder with the entries of its open
// folders, as the app last told them. The page holds what it was told and is given only what
// changed; it draws only the rows in view, each one height, so a folder of thousands costs what
// a screenful does. What the person does is said to the window (`tree open src`), which answers
// with the tree as it then is: a folder shut closes at once, one opened fills as its entries come.
// A name is written in the row itself (a new entry's in a row of its own); an entry is dragged
// into a folder of the tree, or out over the pages, where the window says what takes it; and
// what the app would not do is said under the entry it was asked of.
if (document.documentElement.dataset.part === "side") {
  const ROW = 24, OVER = 8, INDENT = 12;
  const head = document.getElementById("fh"), box = document.getElementById("ft"), sheet = document.getElementById("fr");
  const say = (what) => window.ipc.postMessage("tree " + what);
  // What the page holds: the project's key, the open folders' paths, and each one's entries.
  let model = null;
  // The rows from top to bottom, the path of the one picked, and the rows drawn by their paths.
  let flat = [], pick = null, made = 0;
  const drawn = new Map();
  // The name being written (`make`: of a new file or folder in folder `dir`; else entry `p`'s
  // own), what was refused and under which entry, and the entry carried by the pointer.
  let edit = null, note = null, carried = null;
  const plain = (item) => item && !item.more && !item.note && !item.edit;
  const above = (path) => path.slice(0, Math.max(0, path.lastIndexOf("/")));
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
      // No name holds a line break: these rows' keys are no entry's path.
      const said = (p) => {
        if (note?.p !== p || note.shown) return;
        note.shown = true;
        flat.push({ p: p + "\n!", name: note.why, depth, note: true });
      };
      if (edit?.make && edit.dir === path) flat.push({ p: under + "\n+", name: "", depth, dir: edit.make === "dir", edit: true });
      for (const name of dir.d) {
        const p = under + name, open = model.open.has(p) && model.dirs.has(p);
        flat.push({ p, name, depth, dir: true, open });
        said(p);
        if (open) walk(p, depth + 1);
      }
      for (const [name, icon] of dir.f) {
        flat.push({ p: under + name, name, depth, icon });
        said(under + name);
      }
      // No name holds a line break: this row's key is no entry's path.
      if (dir.m) flat.push({ p: under + "\n", name: dir.m.toLocaleString("en-US") + " more", depth, more: true });
    };
    if (note) note.shown = false;
    if (model) walk("", 0);
    // What was refused of an entry that is not in sight is said first of all.
    if (model && note && !note.shown) flat.unshift({ p: "\n!", name: note.why, depth: 0, note: true });
    if (edit && !edit.make && !flat.some((item) => item.p === edit.p)) edit = null;
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
    // The row a name is written in stays drawn wherever the tree is scrolled to.
    const places = Array.from({ length: Math.max(0, to - from) }, (_, at) => from + at);
    const writing = edit ? flat.findIndex(written) : -1;
    if (writing >= 0 && (writing < from || writing >= to)) places.push(writing);
    for (const at of places) {
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
    if (edit && !edit.focused && edit.input.isConnected) {
      edit.focused = true;
      edit.input.focus();
      const dot = edit.from.lastIndexOf(".");
      edit.input.setSelectionRange(0, dot > 0 ? dot : edit.from.length);
    }
  }
  const written = (item) => !!edit && (edit.make ? !!item.edit : item.p === edit.p);
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
    const kind = item.more ? "fe more" : item.note ? "fe note" : "fe";
    if (row.className !== kind) row.className = kind;
    const [turn, picture, name] = row.children;
    if (!!item.dir !== !!turn.firstChild) turn.replaceChildren(...(item.dir ? [glyph("down")] : []));
    const open = item.dir && (item.open || opening.has(item.p));
    const look = item.more ? "" : item.dir ? (open ? "folder-open" : "folder") : item.icon;
    if (picture.dataset.i !== look) {
      picture.dataset.i = look;
      picture.replaceChildren(...(look ? [pictured(look)] : []));
    }
    if (written(item)) {
      if (name.firstChild !== edit.input) name.replaceChildren(edit.input);
    } else if (name.firstElementChild || name.textContent !== item.name) name.textContent = item.name;
    const entry = plain(item);
    set("role", entry ? "treeitem" : item.note ? "alert" : "none");
    set("aria-level", entry ? String(item.depth + 1) : null);
    set("aria-expanded", item.dir && !item.edit ? String(open) : null);
    set("aria-selected", entry ? String(item.p === pick) : null);
    set("data-pick", item.p === pick ? "" : null);
    set("data-held", carried?.item.p === item.p ? "" : null);
    set("data-into", carried && carried.into === item.p && item.dir ? "" : null);
    set("title", entry ? item.p : item.note ? item.name : null);
    row.dataset.p = item.p;
  }
  const itemOf = (node) => {
    const path = node instanceof Element ? node.closest(".fe")?.dataset.p : undefined;
    const item = path === undefined ? undefined : flat.find((one) => one.p === path);
    return plain(item) ? item : undefined;
  };
  // What was refused is said till the person does the next thing in the tree.
  function unsaid() {
    if (!note) return;
    note = null;
    flatten();
  }
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
    if (!plain(item)) return;
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
    // The entry the person just made, named or moved is picked where it is now, and in sight.
    const made = told?.pick == null ? -1 : flat.findIndex((item) => item.p === told.pick);
    if (made >= 0) pickAt(made);
    draw();
  };
  // A name taken from the person, as the app asks for it: a new file's or folder's (`make`) in
  // folder `p`, in a row of its own at the head of that folder, which opens for it; else entry
  // `p`'s own, in its row, with what stands before its last dot selected. Return or leaving the
  // field keeps the name, Escape lets it go.
  window.TE = (told) => {
    if (!model || model.k !== told.k) return;
    wrote(false);
    unsaid();
    const from = told.make ? "" : told.p.slice(told.p.lastIndexOf("/") + 1);
    const input = Object.assign(document.createElement("input"), { className: "fx", value: from, spellcheck: false });
    for (const [name, value] of [["autocapitalize", "off"], ["autocomplete", "off"], ["autocorrect", "off"]]) input.setAttribute(name, value);
    input.setAttribute("aria-label", told.make === "dir" ? "Name of the new folder" : told.make ? "Name of the new file" : "New name of " + from);
    edit = told.make ? { make: told.make, dir: told.p, input, from } : { p: told.p, input, from };
    if (told.make && told.p && !(model.open.has(told.p) && model.dirs.has(told.p))) {
      const row = flat.find((item) => item.p === told.p);
      if (row) toggle(row);
      // A folder that does not open takes no name.
      const asked = edit;
      setTimeout(() => {
        if (edit === asked && !input.isConnected) wrote(false);
      }, 2000);
    }
    input.addEventListener("keydown", (event) => {
      event.stopPropagation();
      if (event.key !== "Enter" && event.key !== "Escape") return;
      event.preventDefault();
      wrote(event.key === "Enter", true);
    });
    input.addEventListener("blur", () => {
      if (edit?.input === input) wrote(true);
    });
    for (const kind of ["click", "dblclick", "pointerdown", "contextmenu"]) input.addEventListener(kind, (event) => event.stopPropagation());
    flatten();
    const at = flat.findIndex(written);
    if (at >= 0) {
      const top = at * ROW;
      if (top < box.scrollTop) box.scrollTop = top;
      else if (top + ROW > box.scrollTop + box.clientHeight) box.scrollTop = top + ROW - box.clientHeight;
    }
    draw();
  };
  // The name written is kept (said to the window, which answers with the tree as it then is) or
  // let go; the tree has the keyboard again when a key ended it.
  function wrote(keep, key) {
    if (!edit) return;
    const { input, make, dir, p, from } = edit, name = input.value.trim();
    edit = null;
    if (keep && name && name !== from) say(make ? "make " + make + " " + dir + "\n" + name : "name " + p + "\n" + name);
    flatten();
    if (key) box.focus();
    draw();
  }
  // What the app would not do, said under the entry it was asked of.
  window.TW = (told) => {
    if (!model || model.k !== told.k) return;
    note = { p: told.p, why: told.why };
    flatten();
    const at = flat.findIndex((item) => item.note);
    if (at * ROW < box.scrollTop) box.scrollTop = at * ROW;
    else if ((at + 1) * ROW > box.scrollTop + box.clientHeight) box.scrollTop = (at + 1) * ROW - box.clientHeight;
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
    // The click that ends a carry is the carry's.
    if (swallow) return void (swallow = false);
    unsaid();
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
    unsaid();
    const item = itemOf(event.target);
    pick = item ? item.p : null;
    draw();
    say("menu " + Math.round(event.clientX) + " " + Math.round(event.clientY) + (item ? " " + item.p : ""));
  });
  box.addEventListener("focus", () => {
    if (pick === null) pickAt(flat.findIndex(plain));
  });
  // The keyboard: the arrows up and down move the pick, right opens a folder or goes into it,
  // left shuts it or goes to the folder it is in; Return or Space opens a file or turns a folder;
  // F2 names the entry anew, and Command-Delete moves it to the Trash.
  box.addEventListener("keydown", (event) => {
    if (!flat.length || !model) return;
    const at = flat.findIndex((item) => item.p === pick), item = flat[at];
    if (event.metaKey && !event.ctrlKey && !event.altKey && event.key === "Backspace" && item) {
      event.preventDefault();
      unsaid();
      return void say("trash " + item.p);
    }
    if (event.metaKey || event.ctrlKey || event.altKey) return;
    if (event.key === "F2" && item) {
      event.preventDefault();
      return void window.TE({ k: model.k, p: item.p, make: null });
    }
    unsaid();
    const step = (by) => {
      let to = at + by;
      while (flat[to] && !plain(flat[to])) to += by;
      return flat[to] ? to : at;
    };
    const first = flat.findIndex(plain), last = flat.findLastIndex(plain);
    let to;
    if (event.key === "ArrowDown") to = at < 0 ? first : step(1);
    else if (event.key === "ArrowUp") to = at < 0 ? first : step(-1);
    else if (event.key === "Home") to = first;
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
  // An entry carried by the pointer. Its card is the row's picture and name, drawn where the row
  // is and told to the window as a tab's is (`drag-start`), so the app draws it where the pointer
  // is, over the pages and past the window too; this page's own follows only where the app's
  // does not (`CH()` says it does). Over the tree, the folder it would go into is lit: the folder
  // under the pointer, or the one the entry there is in; a shut folder opens when the pointer
  // rests on it, and the tree scrolls at its top and bottom edges. Let go there, it moves; let
  // go over the pages, the window says what takes it; Escape puts it back.
  const REST = 650, EDGE = 28;
  let press = null, swallow = false;
  const taken = window.CH;
  window.CH = () => {
    taken?.();
    if (carried) carried.card.style.visibility = "hidden";
  };
  function lift(event) {
    const row = drawn.get(press.item.p);
    if (!row) return void (press = null);
    wrote(true);
    unsaid();
    const from = row.children[1].getBoundingClientRect(), card = document.createElement("div");
    card.className = "fc";
    card.append(row.children[1].cloneNode(true), Object.assign(document.createElement("span"), { textContent: press.item.name }));
    const left = Math.max(0, from.left - 6), top = row.getBoundingClientRect().top;
    Object.assign(card.style, { left: left + "px", top: top + "px", maxWidth: Math.max(60, innerWidth - left - 4) + "px" });
    document.body.append(card);
    carried = { item: press.item, id: press.id, card, x: event.clientX, y: event.clientY, last: event, into: null, rest: null, tick: 0 };
    box.setPointerCapture(press.id);
    press = null;
    const size = card.getBoundingClientRect();
    window.ipc.postMessage("drag-start " + [event.clientX - left, event.clientY - top, left, top, size.width, size.height, innerHeight].map(Math.round).join(" "));
    document.documentElement.dataset.carry = "";
    carried.tick = requestAnimationFrame(carry);
    draw();
  }
  // Once a frame while an entry is carried, from where the pointer last was.
  function carry() {
    if (!carried) return;
    carried.tick = requestAnimationFrame(carry);
    const { last, item } = carried, room = box.getBoundingClientRect();
    carried.card.style.transform = "translate(" + (last.clientX - carried.x) + "px," + (last.clientY - carried.y) + "px)";
    const inside = last.clientX >= room.left && last.clientX < room.right && last.clientY >= room.top && last.clientY < room.bottom;
    let into = null, over;
    if (inside) {
      const near = Math.min(last.clientY - room.top, room.bottom - last.clientY);
      if (near < EDGE) box.scrollTop += (last.clientY - room.top < EDGE ? -1 : 1) * Math.ceil((EDGE - near) / 3);
      over = flat[Math.floor((last.clientY - room.top + box.scrollTop) / ROW)];
      into = !over ? "" : over.dir && !over.edit ? over.p : above(over.p);
      // Not where it is already, and a folder goes neither into itself nor under itself.
      if (into === above(item.p) || into === item.p || into.startsWith(item.p + "/")) into = null;
    }
    const resting = into !== null && over?.dir && !over.open && plain(over) ? over.p : null;
    if (resting !== carried.rest?.p) {
      clearTimeout(carried.rest?.timer);
      carried.rest = resting === null ? null : { p: resting, timer: setTimeout(() => {
        const shut = flat.find((one) => one.p === resting);
        if (carried && shut && !shut.open) toggle(shut);
      }, REST) };
    }
    const no = inside && into === null ? "no" : "";
    if (document.documentElement.dataset.carry !== no) document.documentElement.dataset.carry = no;
    if (into !== carried.into) {
      carried.into = into;
      box.toggleAttribute("data-into", into === "");
      draw();
    }
  }
  function drop(kept) {
    if (!carried) return;
    const { item, into, last, card, id, rest, tick } = carried;
    carried = null;
    cancelAnimationFrame(tick);
    clearTimeout(rest?.timer);
    card.remove();
    if (box.hasPointerCapture(id)) box.releasePointerCapture(id);
    delete document.documentElement.dataset.carry;
    box.removeAttribute("data-into");
    window.ipc.postMessage("drag-end");
    if (kept && into !== null) say("move " + item.p + "\n" + into);
    else if (kept && last.clientX >= innerWidth && last.clientY >= 0) say("drop " + Math.round(last.clientX) + " " + Math.round(last.clientY) + " " + item.p);
    swallow = true;
    setTimeout(() => (swallow = false), 0);
    draw();
  }
  box.addEventListener("pointerdown", (event) => {
    const item = event.button === 0 && !event.ctrlKey ? itemOf(event.target) : undefined;
    press = item ? { item, id: event.pointerId, x: event.clientX, y: event.clientY } : null;
  });
  box.addEventListener("pointermove", (event) => {
    if (carried) {
      carried.last = { clientX: event.clientX, clientY: event.clientY };
      return void window.ipc.postMessage("drag-at " + Math.round(event.screenX) + " " + Math.round(event.screenY));
    }
    if (press && event.buttons & 1 && Math.hypot(event.clientX - press.x, event.clientY - press.y) >= 5) lift(event);
  });
  box.addEventListener("pointerup", () => {
    press = null;
    drop(true);
  });
  box.addEventListener("pointercancel", () => {
    press = null;
    drop(false);
  });
  box.addEventListener("lostpointercapture", () => drop(false));
  window.addEventListener("keydown", (event) => {
    if (!carried || event.key !== "Escape") return;
    event.preventDefault();
    event.stopPropagation();
    drop(false);
  }, true);
  window.addEventListener("blur", () => drop(false));
}
