  // `find`: the toolbar's find row over this page (Deck::seek), its query matched in the text a
  // person can see, case ignored, as words and never as a pattern. The matches are lit by two
  // highlights over ranges (every one, and the one shown above them), their look one sheet in
  // the document's adopted sheets: no node of the page is added, wrapped or changed. The match
  // shown is scrolled to when it is out of sight. Each step says back, sealed, which match is
  // shown of how many, for the tab the window named (`{found: {tab, query, at, of}}`); `clear`
  // takes every mark away. Where the engine has no highlights the match shown is the selection.
  const FIND_MOST = 1000
  const LIT = 'llxterm-find'
  const SHOWN = 'llxterm-find-shown'
  const ACCENT = '#0a84ff'
  const SHOW_TEXT = 4
  // Text a person never reads as the page's words.
  const UNREAD = { __proto__: null, script: 1, style: 1, noscript: 1, template: 1, textarea: 1, title: 1 }
  const Lit = typeof Highlight === 'function' ? Highlight : null
  const registry = typeof CSS === 'object' && CSS && CSS.highlights ? CSS.highlights : null
  const Registry = registry && Object.getPrototypeOf(registry)
  const Sheet = typeof CSSStyleSheet === 'function' ? CSSStyleSheet : null
  const RangeOf = typeof Range === 'function' ? Range : null
  const Walker = typeof TreeWalker === 'function' ? TreeWalker : null
  const construct = Reflect.construct
  // A builtin as it is now, where the platform keeps it: on the prototype named or one it
  // inherits from (WebKit keeps where a range starts on AbstractRange's, not Range's).
  const kept = (kind, proto, name) => {
    for (let at = proto; at; at = Object.getPrototypeOf(at)) {
      const f = part(at, name, kind)
      if (f) return f
    }
    return null
  }
  const found = {
    walk: kept('value', Document.prototype, 'createTreeWalker'),
    range: kept('value', Document.prototype, 'createRange'),
    next: Walker && kept('value', Walker.prototype, 'nextNode'),
    data: kept('get', Text.prototype, 'data'),
    above: kept('get', Node.prototype, 'parentElement'),
    rects: kept('value', Element.prototype, 'getClientRects'),
    scroll: kept('value', Element.prototype, 'scrollIntoView'),
    name: kept('get', Element.prototype, 'localName'),
    from: RangeOf && kept('value', RangeOf.prototype, 'setStart'),
    to: RangeOf && kept('value', RangeOf.prototype, 'setEnd'),
    box: RangeOf && kept('value', RangeOf.prototype, 'getBoundingClientRect'),
    order: RangeOf && kept('value', RangeOf.prototype, 'compareBoundaryPoints'),
    start: RangeOf && kept('get', RangeOf.prototype, 'startContainer'),
    light: Registry && kept('value', Registry, 'set'),
    unlight: Registry && kept('value', Registry, 'delete'),
    style: Sheet && kept('value', Sheet.prototype, 'replaceSync'),
    adopted: kept('get', Document.prototype, 'adoptedStyleSheets'),
    adopt: kept('set', Document.prototype, 'adoptedStyleSheets'),
    exec: kept('value', RegExp.prototype, 'exec'),
    lower: kept('value', String.prototype, 'toLowerCase'),
    escape: (s) => replace(s, /[.*+?^${}()|[\]\\]/g, '\\$&'),
  }
  // What this document holds of a find: its query, its matches, the one shown, its sheet.
  let sought = { query: '', ranges: bare(), at: -1, sheet: null }

  // The text nodes under the body, in the document's order.
  function texts() {
    const out = bare()
    if (!document.body || !found.next) return out
    const walker = found.walk(document, document.body, SHOW_TEXT, null)
    for (let node = found.next(walker); node; node = found.next(walker)) out[out.length] = node
    return out
  }
  // Whether a person can see this text: not a script's or a style's, in a box that is drawn.
  function seen(node) {
    const box = found.above(node)
    if (!box || UNREAD[found.lower(found.name(box) || '')]) return false
    return found.rects(box).length > 0
  }
  // Every match of `query` a person can see, as ranges in order, at most FIND_MOST of them.
  function matches(query) {
    const ranges = bare()
    if (!query || !found.range) return ranges
    const pattern = new RegExp(found.escape(query), 'giu')
    const nodes = texts()
    for (let i = 0; i < nodes.length && ranges.length < FIND_MOST; i++) {
      const text = found.data(nodes[i]) || ''
      pattern.lastIndex = 0
      let looked = null
      for (let m = found.exec(pattern, text); m && ranges.length < FIND_MOST; m = found.exec(pattern, text)) {
        if (looked === null) looked = seen(nodes[i])
        if (!looked) break
        const range = found.range(document)
        found.from(range, nodes[i], m.index)
        found.to(range, nodes[i], m.index + m[0].length)
        ranges[ranges.length] = range
      }
    }
    return ranges
  }
  // Which match to show of `ranges`: the first at or after the one shown before (`was`), so
  // typing on stays where it is; with none before, the first that is not above the view.
  function settledAt(ranges, was) {
    if (!ranges.length) return -1
    for (let i = 0; i < ranges.length; i++) {
      if (was ? found.order(ranges[i], 0, was) >= 0 : found.box(ranges[i]).bottom >= 0) return i
    }
    return was ? 0 : ranges.length - 1
  }
  // The look of the marks: the window's accent (a #rrggbb, else the default) at the terminal's
  // two steps.
  function look(accent) {
    const colour = /^#[0-9a-f]{6}$/i.test(accent) ? accent : ACCENT
    const rgb = [1, 3, 5].map((at) => parseInt(colour.slice(at, at + 2), 16)).join(' ')
    return `::highlight(${LIT}){background-color:rgb(${rgb} / 25%)}` +
      `::highlight(${SHOWN}){background-color:rgb(${rgb} / 60%)}`
  }
  // The document's adopted sheets with the marks' (`on`) or without it; a page's own stay.
  function adopted(sheet, on) {
    if (!sheet || !found.adopted || !found.adopt) return
    const sheets = found.adopted(document) || []
    // An ordinary array: the setter takes what it can iterate.
    const kept = []
    for (let i = 0; i < sheets.length; i++) if (sheets[i] !== sheet) kept[kept.length] = sheets[i]
    if (on) kept[kept.length] = sheet
    found.adopt(document, kept)
  }
  // The matches lit, the one shown above them; where there are no highlights, it is selected.
  function paint(accent) {
    if (Lit && registry && found.light && Sheet && found.style) {
      sought.sheet = sought.sheet || construct(Sheet, [])
      found.style(sought.sheet, look(accent))
      adopted(sought.sheet, true)
      found.light(registry, LIT, construct(Lit, sought.ranges))
      const shown = construct(Lit, sought.at < 0 ? [] : [sought.ranges[sought.at]])
      shown.priority = 1
      found.light(registry, SHOWN, shown)
      return
    }
    const selection = getSelection()
    selection?.removeAllRanges()
    if (sought.at >= 0) selection?.addRange(sought.ranges[sought.at])
  }
  function unpaint() {
    if (registry && found.unlight) {
      found.unlight(registry, LIT)
      found.unlight(registry, SHOWN)
    }
    adopted(sought.sheet, false)
  }
  // The match shown brought into view, unless it is in view already.
  function reveal() {
    const range = sought.ranges[sought.at]
    if (!range) return
    const box = found.box(range)
    if (box.top >= 0 && box.bottom <= innerHeight && box.left >= 0 && box.right <= innerWidth) return
    const element = found.above(found.start(range))
    if (element) found.scroll(element, { block: 'center', inline: 'nearest' })
  }

  // One step: `type` (the query as it is typed), `next`, `previous`, `again` (the same query in
  // what the page holds now, nothing moved), `clear`.
  function find(args) {
    const tab = typeof args?.tab === 'number' ? args.tab : 0
    const query = typeof args?.query === 'string' ? args.query : ''
    const accent = typeof args?.accent === 'string' ? args.accent : ACCENT
    const step = args?.step
    if (step === 'clear' || !query) {
      unpaint()
      sought = { query: '', ranges: bare(), at: -1, sheet: sought.sheet }
      if (step !== 'clear') send({ found: { tab, query, at: -1, of: 0 } })
      return null
    }
    const walking = (step === 'next' || step === 'previous') && query === sought.query && sought.ranges.length
    if (walking) {
      const n = sought.ranges.length
      sought.at = (sought.at + (step === 'next' ? 1 : n - 1)) % n
    } else {
      const was = query === sought.query || step === 'type' ? sought.ranges[sought.at] : null
      const ranges = matches(query)
      sought = { query, ranges, at: settledAt(ranges, was), sheet: sought.sheet }
    }
    paint(accent)
    if (step !== 'again') reveal()
    send({ found: { tab, query, at: sought.at, of: sought.ranges.length } })
    return null
  }
