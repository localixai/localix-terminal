// The Store in the window. The app tells the page what there is (S and a name: begun, found,
// shown, row, sources, said), and what the person does is posted back as one JSON
// action ("k" and it). Each thing told is one event of what changed: a card is set right where
// it lies, and a list is read again only when its listings changed. Everything a listing says
// came from whoever published it: it is set as text, never parsed as markup, and an address is
// opened by the app, never by the page.
const $ = (id) => document.getElementById(id);
// Line glyphs on the 16-point grid.
const G = {
  discover: "M8 2.5a5.5 5.5 0 1 0 0 11a5.5 5.5 0 1 0 0-11zM10.3 5.7 9 9 5.7 10.3 7 7z",
  plugin: "M6 2.5v3M10 2.5v3M4.5 5.5h7v2a3.5 3.5 0 0 1-7 0zM8 11v2.5",
  skill: "M8 2.5l1.3 3.2 3.2 1.3-3.2 1.3L8 11.5 6.7 8.3 3.5 7l3.2-1.3zM12.5 11v2.5M11.25 12.25h2.5",
  server: "M3 3.5h10V7H3zM3 9h10v3.5H3zM5.3 5.25h.01M5.3 10.75h.01",
  installed: "M8 2.5 13 5v6l-5 2.5L3 11V5zM3 5l5 2.5L13 5M8 7.5v6",
  updates: "M12.6 6.2A5 5 0 1 0 13 8M12.8 3v3.3H9.5",
  sources: "M8 2.5v7M5 7l3 3 3-3M3 11v2h10v-2",
  down: "M4.5 6.5 8 10l3.5-3.5",
  dots: "M4 8h.01M8 8h.01M12 8h.01",
  back: "M9.8 3.5 5.3 8l4.5 4.5",
  check: "M3.5 8.5l3 3 6-7",
  sure: "M8 2.5a5.5 5.5 0 1 0 0 11a5.5 5.5 0 1 0 0-11zM5.6 8.2l1.7 1.7 3.1-3.6",
};
const KINDS = ["plugin", "skill", "server"];
const NAMES = { discover: "Discover", plugin: "Plugins", skill: "Skills", server: "MCP Servers", installed: "Installed", updates: "Updates", sources: "Sources" };
const LIBRARY = ["installed", "updates", "sources"];
const KINDLY = { plugin: "plugins", skill: "skills", server: "MCP servers" };
// The agents' marks, as the window gives them to its pages: SVG by the agent's id, in the
// text's colour. An agent without one shows no mark.
const MARKS = JSON.parse(document.getElementById("marks").textContent);
function mark(agent) {
  const holder = el("span.mk", { "aria-hidden": "true" });
  const svg = new DOMParser().parseFromString(MARKS[agent] || "", "image/svg+xml").documentElement;
  if (svg.nodeName === "svg") holder.append(document.importNode(svg, true));
  return holder;
}
const WHAT = { plugin: "Plugin", skill: "Skill", server: "MCP Server" };
const HOLDS = { plugin: "Plugins", skill: "Skills", server: "MCP servers" };
// One line of what each place is, under its name.
const NOTES = {
  plugin: "Skills, commands, hooks and servers in one package that an agent's own program installs.",
  skill: "Instructions an agent reads when they apply. A skill runs nothing by itself.",
  server: "Tools an agent calls: a program it starts on this Mac, or an address it reaches.",
  installed: "What the Store put into your agents.",
  updates: "What is listed newer than what you have.",
  sources: "Where the Store finds what it offers.",
};
const TRUST = { 3: "Official", 2: "Community", 1: "Open registry" };
const PAGE = 60, SOME = 6;
const asks = new Map(), filters = new Map();
let moved = false, lastSaid = "", saidFor = 0;
let at = "discover", within = null, counts = {}, rows = [], total = 0, facets = [], seq = 0, turn = 0;
let drawn = false, stale = false, again = null, sources = [], removed = 0, busy = false;
let picked = null, opened = null, one = null, got = false, waiting = false;

// A line glyph on the 16-point grid, sized by the sheet.
const glyph = (d) => svgPath(d);
const many = (n) => Number(n || 0).toLocaleString("en-US");
const typed = () => $("q").value.trim();
// What narrows the list shown: a category and a source, kept for each place.
function filter() {
  const key = within ? "in:" + within.title : at;
  if (!filters.has(key)) filters.set(key, { category: null, source: null });
  return filters.get(key);
}
// The two letters a listing is drawn as: the first of its first two words.
function letters(title) {
  const words = String(title).split(/[^\p{L}\p{N}]+/u).filter(Boolean);
  const two = words.length > 1 ? words[0][0] + words[1][0] : (words[0] || "?").slice(0, 2);
  return two.toUpperCase();
}
// A listing's tile: its letters in a hue that comes from its id, so the same listing is the
// same colour everywhere. The yellow-green arc is skipped: at a tile's darkness it is olive.
function hue(id) {
  let hash = 0x811c9dc5;
  for (let at = 0; at < id.length; at += 1) hash = Math.imul(hash ^ id.charCodeAt(at), 0x01000193);
  const turn = Math.floor((((hash >>> 0) % 360) * 300) / 360);
  return turn < 55 ? turn : turn + 60;
}
function tile(listing) {
  const made = el("span.tile", { text: letters(listing.title), "aria-hidden": "true" });
  made.style.setProperty("--hue", String(hue(listing.what + "/" + listing.id)));
  return made;
}
function sure(trust) {
  return trust >= 3 ? el("span.sure", { title: "Kept by its vendor", role: "img", "aria-label": "Official" }, glyph(G.sure)) : null;
}
// How long ago, in words: `ago` in seconds.
function ago(seconds) {
  if (seconds === null || seconds === undefined) return "not read yet";
  if (seconds < 90) return "read just now";
  if (seconds < 3600) return "read " + Math.round(seconds / 60) + " min ago";
  if (seconds < 172800) return "read " + Math.round(seconds / 3600) + " hr ago";
  return "read " + Math.round(seconds / 86400) + " days ago";
}

// A button that opens a menu, as a tab's or a field's neighbour.
function opener(spec, words, label, entries) {
  const button = el(spec, { "aria-haspopup": "menu", "aria-label": label, title: words ? null : label,
    onclick: (event) => { event.stopPropagation(); menuUnder(button, entries()); } },
    words ? el("span", { text: words }) : null, glyph(words ? G.down : G.dots));
  return button;
}

function drawNav() {
  const tab = (id) => el("button.tab", { role: "tab", "aria-selected": String(at === id && !within && !typed()), onclick: () => go(id) },
    glyph(G[id]), el("span", { text: NAMES[id] }));
  $("kinds").replaceChildren(tab("discover"), ...KINDS.map(tab));
  rowInView($("kinds"), $("kinds").querySelector('[aria-selected="true"]'));
  rowEdges($("kinds"));
  $("lib").replaceChildren(...LIBRARY.map((id) => el("button.nav", { "aria-current": at === id && !within && !typed() ? "page" : null, title: NAMES[id], "aria-label": NAMES[id], onclick: () => go(id) },
    glyph(G[id]), el("span", { text: NAMES[id] }), id === "updates" && counts.updates ? el("span.n", { text: many(counts.updates) }) : null)));
}
// Another place of the Store is shown; `inside`, a list within it (a source's, a category's).
function go(id, inside) {
  at = id;
  within = inside || null;
  $("q").value = "";
  if (one) { opened = one = null; drawOne(); }
  drawNav();
  load();
}
// Listings asked of the app: `then` hears the answer to this ask.
function ask(params, then) {
  seq += 1;
  asks.set(seq, then);
  act({ do: "search", seq, ...params });
}
// What is asked for the place shown.
function wanted() {
  const text = $("q").value, f = filter();
  if (typed()) return { text, show: "all" };
  if (within) return { text: "", show: "all", source: within.source, category: within.category || f.category };
  if (at === "installed" || at === "updates") return { text: "", show: at };
  return { text: "", show: at, category: f.category, source: f.source };
}
// What is shown is read anew: a few of each kind under Discover, the sources, else the
// listings of the place, a page of them at a time. An answer to an older turn is passed over.
function load(keep) {
  turn += 1;
  const mine = turn;
  if (typed() || within) return more(0, keep);
  if (at === "sources") { act({ do: "sources" }); return drawSources(keep); }
  if (at !== "discover") return more(0, keep);
  const parts = {};
  const done = () => { if (mine === turn && parts.all && KINDS.every((kind) => parts[kind])) drawSome(parts, keep); };
  for (const kind of KINDS) ask({ text: "", show: kind, offset: 0, limit: SOME }, (found) => { parts[kind] = found; done(); });
  ask({ text: "", show: "all", offset: 0, limit: 1 }, (found) => { parts.all = found; done(); });
}
function more(offset, keep) {
  const mine = turn;
  waiting = true;
  ask({ ...wanted(), offset, limit: PAGE }, (found) => {
    if (mine !== turn) return;
    waiting = false;
    total = found.total;
    if (offset) {
      rows = rows.concat(found.listings);
      const grid = $("list").querySelector(".grid");
      if (grid) grid.append(...found.listings.map(card));
      return;
    }
    rows = found.listings;
    if (found.facets) facets = found.facets.filter((facet) => facet[1] > 0);
    drawFound(keep);
  });
}
// Where the Store stands with a listing, in its button's place.
function stand(listing) {
  if (listing.busy) return el("span.doing", { role: "progressbar", "aria-label": "Working on " + listing.title }, el("i"));
  if (listing.update) return el("button.btn", { text: "Update", tabindex: "-1", "data-get": true, "aria-label": "Update " + listing.title });
  if (listing.has) return el("span.has", null, glyph(G.check), el("span", { text: "Installed" }));
  return el("button.btn", { text: "Get", tabindex: "-1", "data-get": true, "aria-label": "Get " + listing.title });
}
function card(listing) {
  const mixed = !!typed() || !!within || at === "installed" || at === "updates";
  const meta = [listing.author, mixed && WHAT[listing.what], listing.category, listing.trust === 2 && "Community"].filter(Boolean).join(" · ");
  return el("div.card", { role: "option", "aria-selected": String(listing.id === picked), "data-id": listing.id },
    tile(listing),
    el("div.w", null,
      el("div.name", null, el("span", { text: listing.title }), sure(listing.trust)),
      el("div.u", { text: listing.summary || listing.name }),
      el("div.m", { text: meta })),
    stand(listing));
}
function head(name, note, ...acts) {
  return el("div.head", null, el("div.w", null, el("h2", { text: name }), note ? el("p", { text: note }) : null),
    acts.some(Boolean) ? el("div.do", null, ...acts) : null);
}
function grid(name, listings) {
  return el("div.grid", { role: "listbox", "aria-label": name }, ...listings.map(card));
}
function put(nodes, keep, redo) {
  back();
  drawn = true;
  again = redo;
  const top = $("list").scrollTop;
  $("list").replaceChildren(...nodes.filter(Boolean));
  $("list").scrollTop = keep ? top : 0;
}
// What waits to be updated, said at the head of Discover and of Updates.
function waits() {
  if (!counts.updates) return null;
  const words = counts.updates === 1 ? "1 update is ready" : many(counts.updates) + " updates are ready";
  return el("div.note-row", null, el("span", { text: words }),
    at !== "updates" ? el("button.btn.plain", { text: "See Updates", onclick: () => go("updates") }) : null,
    el("button.btn", { text: "Update All", onclick: () => act({ do: "update-all" }) }));
}
function drawSome(parts, keep) {
  rows = KINDS.flatMap((kind) => parts[kind].listings);
  total = rows.length;
  const shelves = KINDS.filter((kind) => parts[kind].listings.length).flatMap((kind) => [
    el("div.sub", null, el("h3", { text: NAMES[kind] }), el("span.n", { "data-count": kind, text: many(Math.max(parts[kind].total, counts[kind] || 0)) }), el("span.gap"),
      el("button.btn.plain", { text: "See All", "aria-label": "See All " + NAMES[kind], onclick: () => go(kind) })),
    grid(NAMES[kind], parts[kind].listings),
  ]);
  if (shelves.length) shelves[0].style.marginTop = "0";
  const kinds = (parts.all.facets || []).filter((facet) => facet[1] > 0);
  const byKind = kinds.length > 1 ? [el("div.sub", null, el("h3", { text: "Categories" })),
    el("div.kinds", null, ...kinds.map(([name, count]) => el("button.kind", { onclick: () => go("discover", { title: name, category: name }) },
      el("span", { text: name }), el("span.n", { text: many(count) }))))] : [];
  const nodes = [waits(), ...shelves, ...byKind];
  put(nodes.some(Boolean) ? nodes : [el("div.empty", { text: busy ? "Reading the sources…" : "No listings yet" })], keep, () => drawSome(parts, true));
}
// A shelf's categories as tabs, the busiest first, in a row that scrolls sideways when they do
// not fit; and its sources, a menu at the row's end.
function narrowing() {
  const f = filter();
  const set = (name, value) => { f[name] = value; load(); };
  const cats = within && within.category ? [] : facets;
  const from = within || typed() ? [] : sources.filter((source) => source.holds === HOLDS[at]);
  const chosen = from.find((source) => source.id === f.source);
  if (cats.length < 2 && from.length < 2) return null;
  const tab = (name, count, on) => el("button.tab", { role: "tab", "aria-selected": String(on), onclick: () => set("category", name) },
    el("span", { text: name || "All" }), count ? el("span.n", { text: many(count) }) : null);
  const was = document.querySelector(".cats .row");
  const row = el("div.row", { role: "tablist", "aria-label": "Categories" },
    ...(cats.length > 1 ? [tab(null, 0, !f.category), ...cats.map(([name, count]) => tab(name, count, name === f.category))] : []));
  return el("div.tabs.cats", null, across(row, '[aria-selected="true"]', was ? was.scrollLeft : 0),
    from.length > 1 ? el("div.end", null, opener("button.tab.pick", chosen ? chosen.title : "Source", chosen ? "Source: " + chosen.title : "Source", () => [
      { label: "Any Source", marked: !chosen, run: () => set("source", null) }, { line: true },
      ...from.map((source) => ({ label: source.title + " (" + many(source.listings) + ")", marked: source === chosen, run: () => set("source", source.id) }))])) : null);
}
function drawFound(keep) {
  const text = typed(), f = filter();
  const name = text ? "Results for “" + text + "”" : within ? within.title : NAMES[at];
  const note = text ? many(total) + (total === 1 ? " listing" : " listings") : within ? within.note : NOTES[at];
  const none = text ? "No listing matches" : f.category || f.source ? "Nothing here matches"
    : at === "installed" ? "What you install from the Store is listed here"
    : at === "updates" ? "Everything installed is up to date" : busy ? "Reading the sources…" : "No listings yet";
  put([head(name, note), at === "updates" && !text && !within ? waits() : null, text ? null : narrowing(),
    rows.length ? grid(name, rows) : el("div.empty", { text: none })], keep, () => drawFound(true));
}
function drawSources(keep) {
  rows = [];
  total = 0;
  const row = (source) => {
    const show = () => go("sources", { title: source.title, note: source.repo, source: source.id });
    return el("div.src", { "data-id": source.id, role: "button", tabindex: "0", "aria-label": "Show what " + source.title + " lists", onclick: show,
      onkeydown: (event) => { if (event.key === "Enter" && event.target === event.currentTarget) show(); } },
      el("span.tile", { "aria-hidden": "true" }, glyph(G.sources)),
      el("div.w", null,
        el("div.t", null, el("span", { text: source.title }), el("span.n", { text: TRUST[source.trust] || "" })),
        el("div.s", { text: [many(source.listings) + (source.listings === 1 ? " listing" : " listings"), ago(source.ago), source.repo].join(" · ") }),
        source.problem ? el("div.p", { text: "Could not be read: " + source.problem, title: source.problem }) : null),
      opener("button.ico", null, source.title + " Actions", () => [
        { label: "Update Now", run: () => act({ do: "source-read", id: source.id }) }, { line: true },
        { label: "Remove…", run: () => act({ do: "source-remove", id: source.id }) }]));
  };
  const groups = Object.values(HOLDS).filter((holds) => sources.some((source) => source.holds === holds)).flatMap((holds) => [
    el("div.label", { text: holds }), ...sources.filter((source) => source.holds === holds).map(row)]);
  put([head(NAMES.sources, NOTES.sources,
      removed ? el("button.btn.plain", { text: "Restore Defaults", onclick: () => act({ do: "sources-restore" }) }) : null,
      el("button.ico", { title: "Update All Sources Now", "aria-label": "Update All Sources Now", onclick: () => act({ do: "refresh" }) }, glyph(G.updates)),
      el("button.btn", { text: "Add Source…", "aria-haspopup": "dialog", onclick: () => act({ do: "source-add" }) })),
    ...(groups.length ? groups : [el("div.empty", { text: "No sources" })])], keep, () => drawSources(true));
}
// The card the keys are at.
function pick(id) {
  picked = id;
  for (const item of $("list").querySelectorAll(".card")) {
    item.setAttribute("aria-selected", String(item.dataset.id === id));
    if (item.dataset.id === id) item.scrollIntoView({ block: "nearest" });
  }
}
// A listing's own view, in the shelf's place; `get` when it was opened to be installed.
function open(id, get) {
  pick(id);
  opened = id;
  moved = true;
  got = !!get;
  act({ do: "show", id });
}
function shut() {
  opened = one = null;
  drawOne();
  $("list").focus();
  if (stale) load(true);
  stale = false;
}
// The way back, in the bar: from a listing to where it was opened, from a narrowed shelf to
// the shelf. It is off where there is nowhere to go back to.
function back() {
  const to = one ? (typed() ? "Results" : within ? within.title : NAMES[at]) : within && !typed() ? NAMES[at] : null;
  $("shut").disabled = !to;
  $("shut").title = to ? "Back to " + to : "Back";
  $("shut").setAttribute("aria-label", $("shut").title);
}
function drawOne() {
  $("one").hidden = !one;
  $("list").hidden = !!one;
  back();
  if (!one) return;
  const l = one.listing;
  const meta = [l.author, WHAT[l.what], l.category].filter(Boolean).join(" · ");
  // Its one action, under its name: Install (into the one agent that takes it, or the agent
  // picked from the menu), Update where a newer one waits; once it is held, in whom, and a
  // menu of the rest (another agent, an update, a removal). Never a row for each agent.
  const noun = KINDLY[l.what];
  const usable = one.agents.filter((agent) => agent.takes && agent.present);
  const held = usable.filter((agent) => agent.has !== null), free = usable.filter((agent) => agent.has === null);
  const stale = held.filter((agent) => agent.update), working = one.agents.find((agent) => agent.busy);
  const run = (what, agent) => act({ do: what, id: l.id, agent: agent.id });
  const marks = (agents, words) => el("span.in", { role: "img", "aria-label": words + " " + agents.map((agent) => agent.name).join(", "), title: words + " " + agents.map((agent) => agent.name).join(", ") },
    ...agents.map((agent) => mark(agent.id)));
  // The primary button: straight to the one agent it can mean, else a menu of them.
  const primary = (words, what, agents) => agents.length === 1
    ? el("button.btn.go", { text: words + (usable.length > 1 || what === "plan" && !held.length ? " in " + agents[0].name : ""), onclick: () => run(what, agents[0]) })
    : (opener("button.btn.go", words, words + " " + l.title, () => agents.map((agent) => ({ label: agent.name, run: () => run(what, agent) }))));
  const rest = () => [
    ...free.map((agent) => ({ label: "Install in " + agent.name, run: () => run("plan", agent) })),
    ...(stale.length > 1 ? stale.map((agent) => ({ label: "Update in " + agent.name, run: () => run("plan", agent) })) : []),
    free.length || stale.length > 1 ? null : undefined,
    ...held.map((agent) => ({ label: held.length > 1 ? "Remove from " + agent.name : "Remove", run: () => run("remove", agent) })),
  ].filter((entry) => entry !== undefined);
  const action = el("div.act", null, ...(working
    ? [el("span.doing", { role: "progressbar", "aria-label": working.busy }, el("i")), el("span.at", { text: working.busy })]
    : !usable.length ? [el("span.at", { text: "No agent on this Mac takes " + noun })]
    : !held.length ? [primary("Install", "plan", free), free.length > 1 ? marks(free, "Installs in") : null]
    : [stale.length ? primary("Update", "plan", stale) : null,
      el("span.at", null, glyph(G.check), el("span", { text: "Installed" })), marks(held, "Installed in"),
      opener("button.btn.plain.ico", "", l.title + " Options", rest)]).filter(Boolean));
  const said = one.agents.filter((agent) => agent.said && !agent.busy).map((agent) => el("p.said", { text: agent.name + ": " + agent.said }));
  const link = (words, url) => {
    const browse = () => act({ do: "browse", url });
    return el("a", { text: words, role: "link", tabindex: "0", onclick: browse, onkeydown: (event) => { if (event.key === "Enter") browse(); } });
  };
  const short = (url) => url.replace(/^https?:\/\/(www\.)?/, "").replace(/\/$/, "");
  const fact = (name, value) => value ? el("div.fact", null, el("dt", { text: name }), el("dd", { title: typeof value === "string" ? value : null }, value)) : null;
  const repo = l.origin && l.origin.startsWith("github.com/") ? l.origin.split(/[@#]/)[0] : "";
  const facts = across(el("dl.facts", { "aria-label": "Information" },
    fact("Kind", WHAT[l.what]),
    fact("Category", l.category),
    fact("Version", l.version),
    fact("Publisher", l.author),
    fact("Source", [one.source, TRUST[l.trust]].filter(Boolean).join(" · ")),
    fact("Also In", one.also && one.also.length ? one.also.join(", ") : ""),
    fact("Runs Code", one.runs_code ? "Yes, on This Mac" : "No"),
    repo ? fact("Repository", link(repo.slice(11), "https://" + repo)) : null,
    l.homepage && !(repo && short(l.homepage).startsWith(repo)) ? fact("Website", link(short(l.homepage), l.homepage)) : null));
  const like = (one.more || []).filter((other) => other.id !== l.id);
  $("one-body").replaceChildren(...[
    el("div.top", null,
      tile(l),
      el("div.w", null, el("div.name", null, el("h1", { text: l.title }), sure(l.trust)), el("div.meta", { text: meta }), action)),
    el("p.sum", { text: l.summary || "Its publisher says nothing of it." }),
    ...said,
    facts,
    like.length ? el("div.like", null,
      el("div.sub", null, el("h3", { text: "More in " + (l.category || NAMES[l.what]) })),
      grid("More in " + (l.category || NAMES[l.what]), like)) : null,
  ].filter(Boolean));
  $("one-body").scrollTop = moved ? 0 : $("one-body").scrollTop;
  moved = false;
  const first = got && $("one-body").querySelector(".act button");
  got = false;
  if (first) first.focus();
}
// The Store's line, and how far a read of the sources is: `place` of how many.
function say(words, running, place) {
  busy = !!running;
  // Only what was not said yet is shown, and it leaves after a while.
  const news = !busy && !!words && words !== lastSaid;
  if (!busy) lastSaid = words;
  if (news || busy || !words) {
    clearTimeout(saidFor);
    $("said").textContent = news ? words : "";
    $("said").hidden = !news;
    if (news) saidFor = setTimeout(() => { $("said").hidden = true; }, 8000);
  }
  const part = !busy ? 1 : place ? (place[0] + 0.5) / place[1] : 0.06;
  $("prog").toggleAttribute("data-on", busy);
  $("prog").title = busy ? words : "";
  $("prog").setAttribute("aria-valuenow", String(Math.round(part * 100)));
  $("prog").setAttribute("aria-valuetext", words);
  $("prog").querySelector(".v").setAttribute("stroke-dasharray", Math.round(part * 100) + " 100");
}

// What the app tells the page.
const TOLD = {
  begun(state) {
    counts = state.counts;
    drawNav();
    say(state.said, state.busy, state.at);
    for (const n of document.querySelectorAll("[data-count]")) n.textContent = many(counts[n.dataset.count] || 0);
    if (!drawn) act({ do: "sources" });
    // What changed is read again, unless the person is far down a list or in a listing.
    if (one) stale = stale || state.fresh;
    else if (!drawn || (state.fresh && rows.length <= PAGE)) load(drawn);
  },
  found(found) {
    const then = asks.get(found.seq);
    asks.delete(found.seq);
    if (then) then(found);
  },
  shown(shown) {
    if (shown.listing.id !== opened) return;
    one = shown;
    drawOne();
  },
  // One listing stands otherwise: its card is set right where it lies.
  row(now) {
    const listing = rows.find((row) => row.id === now.id) || (one && (one.more || []).find((row) => row.id === now.id));
    if (listing) Object.assign(listing, now);
    for (const item of document.querySelectorAll("#list .card, #one .card")) {
      if (item.dataset.id === now.id && listing) item.replaceChild(stand(listing), item.lastChild);
    }
    // What is installed, or waits to be updated, is another list once the work is done.
    if (!now.busy && (at === "installed" || at === "updates") && !typed() && !within) {
      if (one) stale = true; else load(true);
    }
  },
  sources(told) {
    const first = !sources.length;
    sources = told.sources;
    removed = told.removed;
    if (one || typed()) return;
    if (at === "sources" && !within) drawSources(true);
    else if (first && again && KINDS.includes(at)) again();
  },
  said(words) { say(words.said, words.busy); },
};
window.S = (name, data) => { if (TOLD[name]) TOLD[name](data); };

let typing = 0;
$("q").addEventListener("input", () => {
  clearTimeout(typing);
  typing = setTimeout(() => { if (one) shut(); drawNav(); load(); }, 90);
});
$("q").addEventListener("keydown", (event) => {
  if (event.key === "ArrowDown" && rows.length && !one) { event.preventDefault(); $("list").focus(); pick(picked && rows.some((listing) => listing.id === picked) ? picked : rows[0].id); }
  if (event.key === "Escape" && $("q").value) { $("q").value = ""; drawNav(); load(); }
});
$("list").addEventListener("click", (event) => {
  const item = event.target.closest(".card");
  if (item) open(item.dataset.id, !!event.target.closest("[data-get]"));
});
// The keys walk the cards as they lie: across a row, and a row up or down.
$("list").addEventListener("keydown", (event) => {
  if (event.target !== $("list")) return;
  const cards = [...$("list").querySelectorAll(".card")];
  const here = cards.findIndex((item) => item.dataset.id === picked);
  if (event.key === "Enter" && here >= 0) { event.preventDefault(); open(picked); return; }
  const across = cards.filter((item) => here >= 0 && item.parentNode === cards[here].parentNode && item.offsetTop === cards[here].offsetTop).length || 1;
  const to = event.key === "ArrowRight" ? here + 1 : event.key === "ArrowLeft" ? here - 1
    : event.key === "ArrowDown" ? (here < 0 ? 0 : here + across) : event.key === "ArrowUp" ? here - across
    : event.key === "Home" ? 0 : event.key === "End" ? cards.length - 1 : null;
  if (to === null || !cards.length) return;
  event.preventDefault();
  if (to < 0) { $("q").focus(); return; }
  pick(cards[Math.min(to, cards.length - 1)].dataset.id);
});
// More of what was found as the list nears its end.
$("list").addEventListener("scroll", () => {
  const list = $("list");
  const listed = typed() || within || (at !== "discover" && at !== "sources");
  if (listed && !waiting && rows.length < total && list.scrollTop + list.clientHeight > list.scrollHeight - 320) more(rows.length);
});
$("shut").addEventListener("click", () => { if (one) shut(); else if (within) go(at); });
$("one-body").addEventListener("click", (event) => {
  const item = event.target.closest(".card");
  if (item) open(item.dataset.id, !!event.target.closest("[data-get]"));
});
document.addEventListener("keydown", (event) => {
  if (event.key === "Escape" && one && document.activeElement !== $("q")) { event.preventDefault(); shut(); return; }
  if (event.key === "Escape" && within && document.activeElement !== $("q")) { event.preventDefault(); go(at); return; }
  if ((event.metaKey && event.key === "f") || (event.key === "/" && document.activeElement.tagName !== "INPUT")) { event.preventDefault(); $("q").focus(); $("q").select(); }
});
// The page lays itself out by its own width: how many categories are tabs follows it.
across($("kinds"), '[aria-selected="true"]');
new ResizeObserver(() => {
  const wide = $("page").clientWidth, now = wide < 560 ? "narrow" : wide < 900 ? "medium" : "wide";
  if (now === $("page").dataset.w) return;
  $("page").dataset.w = now;
  if (again && !one) again();
}).observe($("page"));
drawNav();
// The page's size, once it has loaded: the app paints it whole when it hears this.
post("r 0 0");
