  // Visual feedback: while the person comments, the
  // page takes no presses. A click picks the element under the pointer, a drag marks a region and
  // the elements inside it, and a card anchored to it takes an optional note. What was picked goes
  // to the host as data ({fb}), where the agent reads it as what the person did in the window.
  // Everything is drawn in a closed shadow root the snapshot never sees. A modal dialog (a cart,
  // a sign-in) makes the rest of the page inert and is drawn above any z-index: the overlay goes
  // inside the topmost one and into the top layer, so its card takes typing there too.
  // The overlay is built and worked only through the builtins kept as the probe started (`dom`,
  // start.js), never a method looked up on a prototype now, and its lists have no prototype: no
  // function a page put in place ever sees the root, the card or what sends it, so a page cannot
  // write a comment of its own and send it as the person's.
  const FB_MAX_TEXT = 300
  const FB_MAX_MEMBERS = 8
  const FB_DRAG = 6
  const FB_STYLES = ['display', 'position', 'color', 'background-color', 'font-size', 'font-weight', 'padding', 'margin', 'border-radius']
  const fb = {
    __proto__: null, on: false, all: false, sent: 0, host: null, root: null, opened: false,
    hover: null, card: null, note: null, drag: null, start: null, marks: bare(), count: 0,
  }
  const FB_HOST = 'all:initial;position:fixed;inset:0;z-index:2147483647;pointer-events:none'
  const FB_CSS = `.o{position:fixed;box-sizing:border-box;border:2px solid #4f7cff;border-radius:3px;background:#4f7cff14;pointer-events:none;display:none}
.d{border-style:dashed}
.m{position:fixed;box-sizing:border-box;border-radius:3px;background:#ffb02033;outline:1px solid #ffb020aa;pointer-events:none}
.m b{position:absolute;top:-9px;left:-9px;min-width:18px;height:18px;border-radius:9px;background:#ffb020;color:#000;font:600 11px/18px system-ui;text-align:center}
.c{position:fixed;width:280px;padding:8px;border-radius:10px;background:#1d1d1f;color:#f2f2f2;font:13px system-ui;box-shadow:0 8px 28px #0006;pointer-events:auto}
.c textarea{box-sizing:border-box;width:100%;height:64px;resize:none;border:0;border-radius:6px;padding:6px;font:inherit;background:#2c2c2e;color:inherit;outline:none}
.c div{display:flex;justify-content:flex-end;gap:6px;margin-top:6px}
.c button{border:0;border-radius:6px;padding:4px 10px;font:inherit;background:#3a3a3c;color:inherit;cursor:pointer}
.c button.s{background:#4f7cff;color:#fff}
`

  const fbClip = (s, n = FB_MAX_TEXT) => {
    const t = String(s || '').replace(/\s+/g, ' ').trim()
    return t.length > n ? `${t.slice(0, n)}…` : t
  }
  const fbBox = (r) => ({ x: Math.round(r.left), y: Math.round(r.top), width: Math.round(r.width), height: Math.round(r.height) })
  const fbOurs = (node) => {
    if (fb.root === null) return false
    if (node === fb.host) return true
    try {
      return dom.contains(fb.root, node)
    } catch {
      return false // not a node
    }
  }
  // An element of the overlay's own, of `kind`, its class `cls`, holding `text`.
  const fbMade = (kind, cls, text) => {
    const e = dom.make(document, kind)
    if (cls) dom.attr(e, 'class', cls)
    if (text !== undefined) dom.text(e, String(text))
    return e
  }

  // A stable selector: the nearest id, then each step's tag and place among its own kind.
  function fbSelector(e) {
    const steps = []
    for (let n = e; n && n.nodeType === 1 && n !== document.documentElement; n = n.parentElement) {
      if (n.id && !/\d{4,}/.test(n.id)) {
        steps.unshift(`#${CSS.escape(n.id)}`)
        break
      }
      const same = n.parentElement ? [...n.parentElement.children].filter((c) => c.tagName === n.tagName) : []
      const tag = n.tagName.toLowerCase()
      steps.unshift(same.length > 1 ? `${tag}:nth-of-type(${same.indexOf(n) + 1})` : tag)
    }
    return steps.join(' > ')
  }

  // How a person would name the way down: tags with their first class, at most six deep.
  function fbPath(e) {
    const steps = []
    for (let n = e; n && n.nodeType === 1 && steps.length < 6; n = n.parentElement) {
      const cls = typeof n.className === 'string' ? n.className.trim().split(/\s+/)[0] : ''
      steps.unshift(n.tagName.toLowerCase() + (n.id ? `#${n.id}` : cls ? `.${cls}` : ''))
    }
    return steps.join(' > ')
  }

  // Where in the source the element was written, when the page says: a stamped attribute, else
  // React's development fiber. Best effort; nothing when neither is there.
  function fbSource(e) {
    for (let n = e; n && n.nodeType === 1; n = n.parentElement) {
      const loc = n.getAttribute('data-source-loc') || n.getAttribute('data-inspector-relative-path') || n.getAttribute('data-source-file')
      if (loc) return { file: loc, line: n.getAttribute('data-inspector-line') || n.getAttribute('data-source-line') || undefined }
    }
    const key = Object.keys(e).find((k) => k.startsWith('__reactFiber$') || k.startsWith('__reactInternalInstance$'))
    let components = []
    for (let f = key ? e[key] : null, i = 0; f && i < 40; f = f.return, i++) {
      const name = typeof f.type === 'function' ? f.type.displayName || f.type.name : null
      if (name && components.length < 4) components.push(name)
      const s = f._debugSource || f.memoizedProps?.__source
      if (s?.fileName) return { file: s.fileName, line: s.lineNumber, column: s.columnNumber, components }
    }
    return components.length ? { components } : undefined
  }

  function fbStyles(e) {
    const style = getComputedStyle(e)
    const kept = {}
    for (const name of FB_STYLES) kept[name] = style.getPropertyValue(name)
    return kept
  }

  // The element's opening tag alone, written from its attributes: `outerHTML` would write the whole subtree.
  const fbTag = (e) => `<${e.localName}${[...e.attributes]
    .map((a) => ` ${a.name}="${a.value.replace(/&/g, '&amp;').replace(/"/g, '&quot;')}"`).join('')}>`.slice(0, 200)

  function fbElement(e) {
    return {
      tag: fbTag(e),
      role: e.getAttribute('role') || undefined,
      name: fbClip(e.getAttribute('aria-label') || e.getAttribute('title') || e.getAttribute('alt') || '', 120) || undefined,
      text: fbClip(e.innerText || e.textContent),
      selector: fbSelector(e),
      path: fbPath(e),
      box: fbBox(e.getBoundingClientRect()),
      styles: fbStyles(e),
      source: fbSource(e),
    }
  }

  // The leaves inside a region: elements wholly in it that hold no other element wholly in it.
  function fbMembers(r) {
    const inside = (b) => b.width > 0 && b.left >= r.left && b.top >= r.top && b.right <= r.left + r.width && b.bottom <= r.top + r.height
    const all = [...document.body.querySelectorAll('*')].filter((e) => !fbOurs(e) && inside(e.getBoundingClientRect()))
    // Each element marks the nearest one of `all` above it: that one marks its own the same way.
    const members = new Set(all)
    const holders = new Set()
    for (const e of all) {
      for (let n = e.parentElement; n; n = n.parentElement) {
        if (members.has(n)) {
          holders.add(n)
          break
        }
      }
    }
    const leaves = all.filter((e) => !holders.has(e))
    return leaves.slice(0, FB_MAX_MEMBERS).map((e) => ({ selector: fbSelector(e), text: fbClip(e.innerText || e.textContent, 80) }))
  }

  function fbMount() {
    if (fb.root) return
    const host = dom.make(document, 'desk-feedback')
    dom.attr(host, 'aria-hidden', 'true')
    dom.attr(host, 'style', FB_HOST)
    // A manual popover: in the top layer once shown, above the page's dialogs and popovers, and
    // closed by nothing the page does.
    if (dom.showPopover) dom.attr(host, 'popover', 'manual')
    // Closed, so the page never reaches it; open only for checks (`all`, the native side's alone).
    fb.opened = fb.all
    fb.host = host
    fb.root = dom.shadow(host, { __proto__: null, mode: fb.opened ? 'open' : 'closed' })
    dom.append(fb.root, fbMade('style', '', FB_CSS))
    fb.hover = dom.append(fb.root, fbMade('div', 'o'))
    fb.drag = dom.append(fb.root, fbMade('div', 'o d'))
  }

  // The overlay taken away whole, its marks with it.
  function fbUnmount() {
    dom.remove(fb.host)
    fb.host = fb.root = fb.card = fb.note = fb.hover = fb.drag = null
    fb.marks = bare()
  }

  // The modal dialog on top, if one is open: what is outside it is inert. The last open one in
  // the document stands for the top.
  function fbModal() {
    try {
      const open = dom.allIn(document, 'dialog:modal')
      return open[dom.count(open) - 1] || null
    } catch {
      return null // no :modal in this engine
    }
  }

  // The overlay where the person can reach it and above all the page shows: inside the modal
  // dialog open (else the page's root), shown again on top of the top layer. While the person
  // types in a card in its place, it stays as it is.
  function fbRaise() {
    if (!fb.root) return
    const host = fb.host
    const parent = fbModal() || document.documentElement
    const moved = dom.parent(host) !== parent
    if (moved) {
      try {
        dom.append(parent, host)
      } catch {
        return // not an element it can go in
      }
    }
    if (!dom.showPopover || (fb.card && !moved)) return
    try {
      if (dom.matches(host, ':popover-open')) dom.hidePopover(host)
      dom.showPopover(host)
    } catch {
      // Not shown as a popover: it is still drawn, at the top of the page's own layers.
    }
    if (moved && fb.note) dom.focus(fb.note, { __proto__: null, preventScroll: true })
  }

  const fbShow = (box, r) =>
    dom.attr(box, 'style', `display:block;left:${r.left}px;top:${r.top}px;width:${r.width}px;height:${r.height}px`)
  const fbHide = (box) => dom.attr(box, 'style', 'display:none')

  // The marks of comments sent, measured again as the page moves.
  function fbMeasure() {
    for (let i = 0; i < fb.marks.length; i++) {
      const mark = fb.marks[i]
      const r = mark.target ? dom.rect(mark.target) : mark.rect
      // Gone, or no longer drawn (its dialog closed): no mark in the corner for it.
      if (mark.target && (!dom.connected(mark.target) || (!r.width && !r.height))) fbHide(mark.box)
      else fbShow(mark.box, r)
    }
  }

  // The card closes, and the outline of what it was about with it.
  function fbClose() {
    if (fb.card) dom.remove(fb.card)
    fb.card = fb.note = null
    fbHide(fb.hover)
    fbHide(fb.drag)
  }

  // The card for one capture, beside what it is about: Enter sends, Escape drops it.
  function fbOpen(capture, rect, target) {
    fbClose()
    const card = fbMade('div', 'c')
    const note = dom.append(card, fbMade('textarea'))
    dom.attr(note, 'placeholder', 'What should change? (optional)')
    const row = dom.append(card, fbMade('div'))
    const cancel = dom.append(row, fbMade('button', '', 'Cancel'))
    const sendButton = dom.append(row, fbMade('button', 's', 'Send'))
    const left = Math.min(Math.max(8, rect.left), innerWidth - 296)
    const below = rect.bottom + 120 < innerHeight
    dom.attr(card, 'style', `left:${left}px;top:${below ? rect.bottom + 8 : Math.max(8, rect.top - 120)}px`)
    // The card and outline go first, and the mark comes later: the host takes the picture of
    // what was commented on from the screen, which must show the page alone.
    const submit = () => {
      const said = String(dom.value(note)).trim()
      fbClose()
      send({ fb: { ...capture, note: said, url: location.href, n: ++fb.count } })
      const n = fb.count
      later(() => {
        if (n > fb.count || !fb.root) return
        const box = fbMade('div', 'm')
        dom.append(box, fbMade('b', '', n))
        dom.append(fb.root, box)
        fb.marks[fb.marks.length] = { __proto__: null, box, target, rect }
        fbMeasure()
      }, 800)
    }
    dom.listen(sendButton, 'click', submit)
    dom.listen(cancel, 'click', fbClose)
    dom.listen(note, 'keydown', (event) => {
      dom.stop(event)
      const key = dom.key(event)
      if (key === 'Enter' && !dom.shift(event)) {
        dom.prevent(event)
        submit()
      } else if (key === 'Escape') {
        // The card's alone: a dialog under it stays open.
        dom.prevent(event)
        fbClose()
      }
    })
    dom.append(fb.root, card)
    fb.card = card
    fb.note = note
    dom.focus(note, { __proto__: null, preventScroll: true })
  }

  const fbAt = (x, y) => {
    const e = dom.pointed(document, x, y)
    return e && !fbOurs(e) && e !== document.documentElement ? e : null
  }

  // Whether `event` is on the overlay: its path, as the window sees it, holds the host.
  const fbOnOurs = (event) => {
    const path = dom.path(event)
    for (let i = 0; i < path.length; i++) if (fbOurs(path[i])) return true
    return false
  }

  // Presses on the page stop here while commenting: a control under the pointer is picked, not
  // operated. The card's own events pass.
  // Only the person's own presses (trusted): the page's scripts and the agent's actions go on
  // as ever. `all` takes synthetic ones too, for checks.
  function fbSwallow(event) {
    if (!fb.on || (!event.isTrusted && !fb.all) || fbOnOurs(event)) return false
    dom.prevent(event)
    dom.stopAll(event)
    return true
  }

  function fbDown(event) {
    if (!fbSwallow(event)) return
    fbRaise()
    fb.start = { x: event.clientX, y: event.clientY }
  }

  function fbMove(event) {
    if (!fb.on || !fb.root || fb.card || (!event.isTrusted && !fb.all)) return
    const s = fb.start
    if (s && Math.hypot(event.clientX - s.x, event.clientY - s.y) > FB_DRAG) {
      fbHide(fb.hover)
      const left = Math.min(s.x, event.clientX)
      const top = Math.min(s.y, event.clientY)
      return fbShow(fb.drag, { left, top, width: Math.abs(event.clientX - s.x), height: Math.abs(event.clientY - s.y) })
    }
    const e = fbAt(event.clientX, event.clientY)
    if (e) fbShow(fb.hover, dom.rect(e))
  }

  function fbUp(event) {
    if (!fbSwallow(event) || !fb.root) return
    const s = fb.start
    fb.start = null
    fbHide(fb.drag)
    fbHide(fb.hover)
    if (!s) return
    const width = Math.abs(event.clientX - s.x)
    const height = Math.abs(event.clientY - s.y)
    if (Math.hypot(width, height) > FB_DRAG) {
      const rect = { left: Math.min(s.x, event.clientX), top: Math.min(s.y, event.clientY), width, height }
      rect.right = rect.left + width
      rect.bottom = rect.top + height
      fbOpen({ region: fbBox(rect), members: fbMembers(rect) }, rect, null)
      return fbShow(fb.drag, rect)
    }
    const e = fbAt(event.clientX, event.clientY)
    if (!e) return
    fbOpen({ element: fbElement(e) }, dom.rect(e), e)
    fbShow(fb.hover, dom.rect(e))
  }

  for (const type of ['click', 'dblclick', 'auxclick', 'contextmenu', 'touchstart', 'touchend', 'submit']) {
    addEventListener(type, fbSwallow, { capture: true })
  }
  addEventListener('pointerdown', fbDown, { capture: true })
  addEventListener('mousedown', fbSwallow, { capture: true })
  addEventListener('mouseup', fbSwallow, { capture: true })
  addEventListener('pointerup', fbUp, { capture: true })
  addEventListener('pointermove', fbMove, { capture: true, passive: true })
  addEventListener('scroll', () => fb.marks.length && nextFrame(fbMeasure), { capture: true, passive: true })
  addEventListener('resize', () => fb.marks.length && nextFrame(fbMeasure))
  // A dialog closed or a popover opened or closed: the overlay finds its place again. Its own
  // toggles are its own.
  const fbMoved = (event) => {
    if (!fb.root || event.target === fb.host || (!fb.on && !fb.marks.length)) return
    fbRaise()
    if (fb.marks.length) nextFrame(fbMeasure)
  }
  addEventListener('close', fbMoved, { capture: true })
  addEventListener('toggle', fbMoved, { capture: true })

  // The native side turns commenting on or off; the marks of comments held stay until a batch is
  // sent. Only what the call itself holds is read, never a member a page put on Object's
  // prototype: `sent`, else the batch stays; `all`, else it stays as it was.
  function comment(args) {
    const given = (name, otherwise) =>
      (args !== null && typeof args === 'object' && hasOwn(args, name) ? args[name] : otherwise)
    fb.on = Boolean(given('on', false))
    fb.all = Boolean(given('all', fb.all))
    const sent = given('sent', fb.sent)
    if (sent !== fb.sent) {
      fb.sent = sent
      for (let i = 0; i < fb.marks.length; i++) dom.remove(fb.marks[i].box)
      fb.marks = bare()
      fb.count = 0
    }
    if (fb.on) {
      // A check reaches into an open root: one made for the other kind is made again.
      if (fb.root && fb.opened !== fb.all) fbUnmount()
      fbMount()
      fbRaise()
    } else if (fb.root) fbClose()
    if (fb.root) dom.attr(fb.host, 'style', fb.on ? `${FB_HOST};cursor:crosshair` : FB_HOST)
    document.documentElement.style.cursor = fb.on ? 'crosshair' : ''
    return fb.on
  }
