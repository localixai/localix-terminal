  // The snapshot: what a person can see and use, each control under a ref an action names it by.
  // Characters with no width (zero-width spaces and joiners, soft hyphens) that sites put inside
  // words: a name read with them never matches the word as typed.
  const UNSEEN = /[\u200B-\u200D\u2060\uFEFF\u00AD]/g
  const MAX_ELEMENTS = 250
  const MAX_TEXT = 3000

  const ids = new WeakMap()
  const nodes = new Map()
  let next = 1
  const refOf = (e) => {
    if (!ids.has(e)) ids.set(e, next++)
    const id = ids.get(e)
    nodes.set(id, e)
    return 'e' + id
  }
  const nodeOf = (ref) => nodes.get(Number(String(ref).slice(1)))
  // Which document this is, read once as the probe starts in it: WebKit works the time origin out
  // again from the wall clock on each read, so a later read can differ by a millisecond on the
  // same page, and a ref would then be refused as stale.
  const EPOCH = String(performance.timeOrigin)
  const epoch = () => EPOCH

  // Whether this document is being replaced: a link, a form or a script started a navigation of
  // the main frame. Seen here first; the native side only hears of it once the server answers.
  // How many times the document's words or nodes changed, anywhere: an action that changed only
  // what is out of sight (a counter scrolled above, a message shown again in the same words)
  // changed something. Attributes are left out: an animation changes them all the time.
  let revision = 0
  new MutationObserver((records) => { revision += records.length })
    .observe(document, { subtree: true, childList: true, characterData: true })

  let leaving = false
  addEventListener('beforeunload', () => { leaving = true }, { capture: true })
  window.navigation?.addEventListener('navigate', (event) => {
    if (!event.destination?.sameDocument && !event.hashChange && !event.downloadRequest) leaving = true
  })
  // Known at the moment of the action, before any of the above: a link click or a form submit
  // the page did not cancel will replace the document (a new tab opens in this window, too).
  // Listening on window, registered first, these run after the page's own handlers.
  addEventListener('click', (event) => {
    const link = event.target?.closest?.('a[href]')
    if (!link || event.defaultPrevented || link.hasAttribute('download')) return
    const to = new URL(link.href, location.href)
    if (!/^https?:$/.test(to.protocol)) return
    if (to.href.split('#')[0] !== location.href.split('#')[0]) leaving = true
  })
  addEventListener('submit', (event) => {
    if (!event.defaultPrevented) leaving = true
  })

  // `checkVisibility` is Safari 17.4 and later: an older system WebKit gets the same test by hand.
  const rendered = (e, opacity) => {
    if (typeof e.checkVisibility === 'function') return e.checkVisibility({ checkOpacity: opacity, checkVisibilityCSS: true })
    const view = e.ownerDocument.defaultView
    if (!e.getClientRects().length || view.getComputedStyle(e).visibility !== 'visible') return false
    for (let n = opacity ? e : null; n; n = n.parentElement) if (view.getComputedStyle(n).opacity === '0') return false
    return true
  }
  // An element with `display: contents` (a framework's mount point) has no box, so
  // checkVisibility() calls it hidden and its whole subtree with it: only its children can be.
  const boxless = (e) => e.ownerDocument.defaultView.getComputedStyle(e).display === 'contents'
  // A transparent checkbox or radio is a styled control (its picture is a sibling), not a hidden one.
  const visible = (e) =>
    !e.closest('[aria-hidden="true"],[inert]') &&
    (boxless(e) || rendered(e, !/^(checkbox|radio)$/.test(e.type)))
  // A password field is offered for typing, but its value is never read back.
  const secret = (e) => ['file', 'hidden'].includes(e.type)

  const ROLES = ['button', 'link', 'checkbox', 'radio', 'switch', 'tab', 'menuitem',
    'menuitemradio', 'menuitemcheckbox', 'option', 'gridcell', 'combobox', 'textbox', 'searchbox',
    'spinbutton', 'slider', 'treeitem']
  const SELECTOR = 'a[href],button,input,textarea,select,summary,[contenteditable="true"],' +
    ROLES.map((role) => `[role="${role}"]`).join(',')

  // A control a page made from a plain element: a `<p class="level">` or a `<div>` whose click
  // handler was added by script shows a pointer cursor; an `onclick` or a `tabindex` says so too.
  // Only the outermost of a pointer group (its children inherit the cursor), and never one that
  // holds a real control or another such control (a game's focusable frame around its levels):
  // those are offered instead.
  const PLAIN = /^(BODY|HTML|SVG|PATH|IFRAME|FRAME|CANVAS|VIDEO|SCRIPT|STYLE|LABEL|OPTION)$/i
  const pointer = (e) => e.ownerDocument.defaultView.getComputedStyle(e).cursor === 'pointer'
  const custom = (e) => {
    if (PLAIN.test(e.tagName) || e.matches(SELECTOR) || e.closest('label')) return false
    const marked = e.hasAttribute('onclick') || (e.hasAttribute('tabindex') && e.tabIndex >= 0)
    if (!marked && !(pointer(e) && !(e.parentElement && pointer(e.parentElement)))) return false
    if (e.querySelector(SELECTOR)) return false
    const inner = [...e.querySelectorAll('[onclick],[tabindex]')].concat([...e.querySelectorAll('*')].slice(0, 2000))
    return !inner.some((d) => d.hasAttribute('onclick') || (d.hasAttribute('tabindex') && d.tabIndex >= 0) ||
      (pointer(d) && !pointer(d.parentElement)))
  }
  const isControl = (e) => e.matches(SELECTOR) || custom(e)

  const roleOf = (e) => {
    const explicit = e.getAttribute('role')
    if (ROLES.includes(explicit)) return explicit
    const tag = e.tagName
    if (tag === 'BUTTON' || tag === 'SUMMARY') return 'button'
    if (tag === 'A') return 'link'
    if (tag === 'SELECT') return 'combobox'
    if (tag === 'TEXTAREA' || e.isContentEditable) return 'textbox'
    if (tag !== 'INPUT') return custom(e) ? 'button' : null
    if (e.type === 'checkbox' || e.type === 'radio') return e.type
    if (['button', 'submit', 'reset', 'image'].includes(e.type)) return 'button'
    if (e.type === 'search') return 'searchbox'
    if (e.type === 'number') return 'spinbutton'
    if (e.type === 'range') return 'slider'
    if (e.type === 'password') return 'textbox'
    return 'textbox'
  }

  // Accessible name, in the order the spec uses.
  const nameOf = (e, seen = new Set()) => {
    if (!e || seen.has(e)) return ''
    seen.add(e)
    const labelled = (e.getAttribute('aria-labelledby') || '').split(/\s+/).filter(Boolean)
      .map((id) => nameOf(e.ownerDocument.getElementById(id), seen)).filter(Boolean).join(' ')
    const own = labelled || e.getAttribute('aria-label') ||
      [...(e.labels || [])].map((label) => label.innerText.trim()).filter(Boolean).join(' ') ||
      // An input button reads its value; a <button>'s value is data for the form, not its name.
      (e.tagName === 'INPUT' && ['button', 'submit', 'reset'].includes(e.type) ? e.value : '') ||
      e.getAttribute('alt') ||
      (['INPUT', 'TEXTAREA', 'SELECT'].includes(e.tagName) ? '' : (e.innerText || '').trim()) ||
      e.getAttribute('placeholder') || e.getAttribute('title') || e.getAttribute('name') || ''
    return own.replace(UNSEEN, '').replace(/\s+/g, ' ').trim().slice(0, 120)
  }

  // A nameless control (a row's checkbox, an icon button) is known by the text of its row.
  // Or, for a bare checkbox followed by its words (`<input type=checkbox> checkbox 1<br>`), by the
  // text right after it, else right before it.
  // What a control is called: its own name, else the text of its row, or for a nameless control
  // the page made (an icon in a <div>) the words of its id and class ("mute"): the text beside
  // it is the page's content, not its name.
  const labelOf = (e) => nameOf(e) || (e.matches(SELECTOR) ? contextOf(e) : hintOf(e))
  const hintOf = (e) => `${e.id} ${typeof e.className === 'string' ? e.className : ''}`
    .replace(/[-_]+/g, ' ').replace(/\s+/g, ' ').trim().slice(0, 40)

  const contextOf = (e) => {
    const row = e.closest('li,tr,[role="row"],[role="listitem"],label,fieldset')
    if (row) return (row.innerText || '').replace(/\s+/g, ' ').trim().slice(0, 80)
    const beside = (node, step) => {
      for (let n = node[step]; n; n = n[step]) {
        if (n.nodeType === 1 && (n.matches(SELECTOR) || n.tagName === 'BR')) return ''
        const text = (n.textContent || '').replace(/\s+/g, ' ').trim()
        if (text) return text.slice(0, 80)
      }
      return ''
    }
    return beside(e, 'nextSibling') || beside(e, 'previousSibling')
  }

  // A calendar day reads "15" in every month a picker shows. Name it with its date when the page
  // says it (data-iso, data-date, datetime), else with its month's heading: "15 (2026-11-15)".
  const ISO = /\d{4}-\d{2}-\d{2}/
  const MONTH = /(jan|feb|mar|apr|may|jun|jul|aug|sep|oct|nov|dec)/i
  const dayOf = (e, name) => {
    if (!/^\s*\d{1,2}(\D|$)/.test(name) || name.length > 40) return name
    for (let n = e; n && n !== document.body; n = n.parentElement) {
      for (const key of ['data-iso', 'data-date', 'datetime', 'data-day', 'data-value']) {
        const date = n.getAttribute?.(key)?.match(ISO)
        if (date) return `${name} (${date[0]})`
      }
      const group = n.matches('[role="rowgroup"],[role="grid"],table')
      const heading = group && ((n.getAttribute('aria-label') || '') ||
        (n.querySelector('caption,[role="heading"],h1,h2,h3,h4,h5,h6')?.innerText || '') ||
        (n.innerText || '').split('\n')[0])
      if (heading && MONTH.test(heading) && heading.length < 40) return `${name} (${heading.trim()})`
    }
    return name
  }

  const editableOf = (e, role) =>
    !e.readOnly && e.getAttribute('aria-readonly') !== 'true' &&
    (['textbox', 'searchbox', 'spinbutton'].includes(role) ||
      (role === 'combobox' && ['INPUT', 'TEXTAREA'].includes(e.tagName)) || e.isContentEditable)

  const valueOf = (e, role) => {
    if (e.type === 'password') return e.value ? '••••••' : ''
    if (e.type === 'checkbox' || e.type === 'radio') return ''
    if (e.tagName === 'SELECT') return [...e.selectedOptions].map((o) => o.label).join(', ')
    if ('value' in e && e.tagName !== 'BUTTON' && e.tagName !== 'LI') return String(e.value)
    if (e.isContentEditable || role === 'combobox') return (e.innerText || '').trim().slice(0, 120)
    return ''
  }

  const statesOf = (e) => {
    const states = []
    if (e.matches(':disabled') || e.closest('[aria-disabled="true"]')) states.push('disabled')
    const expanded = e.getAttribute('aria-expanded')
    if (expanded) states.push(expanded === 'true' ? 'expanded' : 'collapsed')
    if (e.checked === true || e.getAttribute('aria-checked') === 'true') states.push('checked')
    if (e.getAttribute('aria-selected') === 'true' || e.getAttribute('aria-current')) states.push('selected')
    if (e.required || e.getAttribute('aria-required') === 'true') states.push('required')
    if (e.readOnly || e.getAttribute('aria-readonly') === 'true') states.push('readonly')
    const invalid = invalidOf(e)
    if (invalid !== null) states.push(invalid ? `invalid: ${invalid}` : 'invalid')
    return states
  }

  const inViewport = (r) =>
    r.width > 0 && r.height > 0 && r.bottom > 0 && r.right > 0 &&
    r.top < innerHeight && r.left < innerWidth
  // Up to one screen below the fold: a control there (a cookie banner's Accept, a form's Submit)
  // is clicked directly, scrolled into view by the click, instead of costing a scroll first.
  const justBelow = (r) =>
    r.width > 0 && r.height > 0 && r.right > 0 && r.left < innerWidth &&
    r.top >= innerHeight && r.top < innerHeight * 2

  const DIALOG = 'dialog[open],[role="dialog"],[role="alertdialog"],[aria-modal="true"]'

  // Every element under `root`, and under the open shadow roots inside it, each shadow tree right
  // after its host: a consent dialog or a web component keeps its controls in one.
  function* deep(root) {
    for (const e of root.querySelectorAll('*')) {
      yield e
      if (e.shadowRoot) yield* deep(e.shadowRoot)
    }
  }
  // `closest`, across shadow boundaries: up through each shadow tree's host.
  const closestDeep = (e, selector) => {
    for (let n = e; n; n = n.parentElement || n.getRootNode?.().host) if (n.matches?.(selector)) return n
    return null
  }

  // The controls a snapshot offers, in its order: visible, with a role, on screen or just below
  // it. An overlay frame first: it is what the page is about now, and the element cap must not
  // cut it.
  function* offered(frames) {
    const docs = [...frames.filter((f) => f.overlay), { doc: document, frame: null }, ...frames.filter((f) => !f.overlay)]
    for (const { doc, frame, ...at } of docs) for (const e of deep(doc.body)) {
      // The cheap test first: most elements are far from the screen, and the others cost style reads.
      const own = e.getBoundingClientRect()
      // Inside a frame, only what its own viewport shows.
      if (frame && (own.bottom <= 0 || own.right <= 0 || own.top >= frame.clientHeight || own.left >= frame.clientWidth)) continue
      const box = shifted(own, frame && at)
      if (!(inViewport(box) || justBelow(box))) continue
      if (!isControl(e) || secret(e) || !visible(e)) continue
      const role = roleOf(e)
      if (!role) continue
      if (role === 'gridcell' && e.querySelector('button,[role="button"]')) continue
      yield { e, role, box, at }
    }
  }

  // Each form a name of its own (`f1`): Enter in a field sends the form its controls are in.
  const formNames = new WeakMap()
  let formCount = 0
  function formOf(e) {
    const form = e.form || e.closest?.('form')
    if (!form) return null
    if (!formNames.has(form)) formNames.set(form, `f${++formCount}`)
    return formNames.get(form)
  }

  function snapshot() {
    if (!document.body) return null
    for (const [id, e] of nodes) if (!e.isConnected || !e.ownerDocument.defaultView) nodes.delete(id)
    const elements = []
    const frames = subframes()
    for (const { frame } of frames) quiet(frame.contentWindow)
    for (const { e, role, box, at } of offered(frames)) {
      const element = { ref: refOf(e), role, name: dayOf(e, labelOf(e)) }
      const value = valueOf(e, role)
      if (value) element.value = value.slice(0, 200)
      const states = statesOf(e)
      if (!inViewport(box)) states.push('below')
      if (states.length) element.states = states
      if (editableOf(e, role)) element.editable = true
      if (closestDeep(e, DIALOG) || at.overlay) element.dialog = true
      const form = formOf(e)
      if (form) element.form = form
      if (e.tagName === 'SELECT') {
        element.options = [...e.options].filter((o) => !o.disabled).slice(0, 50)
          .map((o) => ({ value: o.value, label: o.label }))
      }
      elements.push(element)
      if (elements.length >= MAX_ELEMENTS) break
    }
    mark(elements)
    const text = [...frames.filter((f) => f.overlay).map((f) => visibleText(f.doc, f)), visibleText()].join('\n').slice(0, MAX_TEXT)
    return {
      epoch: epoch(),
      url: location.href.slice(0, 2000),
      title: document.title.slice(0, 200),
      text,
      // Overlay frames first (they cover the page), then the page, then the frames in its flow.
      outline: [...frames.filter((f) => f.overlay).map((f) => outline(f.doc, f)), outline(), ...frames.filter((f) => !f.overlay).map((f) => outline(f.doc, f))].filter(Boolean).join('\n'),
      viewport: { w: innerWidth, h: innerHeight },
      scroll: { y: Math.round(scrollY), height: document.documentElement.scrollHeight },
      elements,
      busy: busy(text),
      revision,
    }
  }

  // Whether the page says it is still working: a document still parsing or fetching its own
  // data (network.js), a busy region, a progress bar, or words such as "Loading…" on screen.
  function busy(text) {
    // Still parsing, or fetching its own data. Images, videos and ad frames still loading into a
    // parsed page are not content a call waits for.
    if (document.readyState === 'loading' || fetching()) return true
    const shown = (selector) => [...document.querySelectorAll(selector)]
      .some((e) => visible(e) && inViewport(e.getBoundingClientRect()))
    return shown('[aria-busy="true"],[role="progressbar"],progress') ||
      /\b(loading|please wait|searching)\b/i.test(text.slice(0, 2000))
  }

  // The words a person can read in the viewport; off-screen bodies and footers stay out.
  function visibleText(doc = document, frame = null) {
    const words = []
    const walker = doc.createTreeWalker(doc.body, NodeFilter.SHOW_TEXT)
    const range = doc.createRange()
    let length = 0
    for (let node = walker.nextNode(); node && length < MAX_TEXT; node = walker.nextNode()) {
      const value = node.textContent.replace(UNSEEN, '').replace(/\s+/g, ' ').trim()
      const parent = node.parentElement
      if (!value || !parent || parent.closest('script,style,noscript,template') || !visible(parent)) continue
      range.selectNodeContents(node)
      if (!inViewport(shifted(range.getBoundingClientRect(), frame))) continue
      words.push(value)
      length += value.length + 1
    }
    return words.join('\n').slice(0, MAX_TEXT)
  }

