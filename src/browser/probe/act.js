  // The element on top at the point of `root` (a document), through open shadow roots.
  function topAt(root, x, y) {
    let top = root.elementFromPoint(x, y)
    while (top?.shadowRoot) {
      const deeper = top.shadowRoot.elementFromPoint(x, y)
      if (!deeper || deeper === top) break
      top = deeper
    }
    return top
  }

  // What covers a target, named so the next step can deal with it: the dialog it belongs to (by
  // its label or first heading), or the one beside it (a backdrop is the dialog's sibling, often
  // in the same shadow tree), else the element itself.
  const coveredBy = (top) => {
    const root = top.getRootNode()
    const dialog = closestDeep(top, DIALOG) || top.parentElement?.querySelector(DIALOG) ||
      (root instanceof ShadowRoot ? root.querySelector(DIALOG) : null)
    if (dialog) {
      const title = nameOf(dialog.querySelector('h1,h2,h3,[role="heading"]')) || dialog.getAttribute('aria-label') || ''
      return new Error(`covered: a dialog "${title.slice(0, 80)}" is on top; answer or close it first`)
    }
    return new Error(`covered: ${top.tagName.toLowerCase()} "${nameOf(top)}" is on top`)
  }

  // Resolve a ref for input: the same document, still attached, visible, enabled and on top. In a
  // frame, on top of its document and the frame on top of the page.
  function target(args) {
    if (args.epoch !== epoch()) throw new Error('stale: the page changed since the snapshot')
    const was = nodeOf(args.ref)
    const e = was?.isConnected && was.ownerDocument?.defaultView && visible(was) ? was : again(args.ref)
    const view = e?.ownerDocument?.defaultView
    if (!e || !view || !e.isConnected || !visible(e)) throw new Error('stale: the element is gone')
    if (e.matches(':disabled') || e.closest('[aria-disabled="true"]')) throw new Error('disabled (the page turned the control off)')
    e.scrollIntoView({ block: 'center', inline: 'center', behavior: 'instant' })
    const r = e.getBoundingClientRect()
    const x = Math.max(1, Math.min(r.left + r.width / 2, view.innerWidth - 1))
    const y = Math.max(1, Math.min(r.top + r.height / 2, view.innerHeight - 1))
    const top = topAt(e.ownerDocument, x, y)
    const label = top?.tagName === 'LABEL' && top.control === e
    if (top && !label && top !== e && !e.contains(top) && !top.contains(e)) throw coveredBy(top)
    const frame = frameOf(e)
    if (frame) {
      const outer = topAt(document, frame.x + x, frame.y + y)
      if (outer && outer !== frame.frame && !outer.contains(frame.frame)) throw coveredBy(outer)
    }
    return { e, x, y, hit: top && e.contains(top) ? top : e }
  }

  // Events and prototypes from the element's own window: a frame is another realm.
  function mouse(el, type, x, y) {
    const view = el.ownerDocument.defaultView
    const down = type.endsWith('down')
    const init = { bubbles: true, cancelable: true, composed: true, view, clientX: x,
      clientY: y, button: 0, buttons: down ? 1 : 0, detail: type.startsWith('pointer') ? 0 : 1 }
    el.dispatchEvent(type.startsWith('pointer')
      ? new view.PointerEvent(type, { ...init, pointerId: 1, isPrimary: true, pointerType: 'mouse' })
      : new view.MouseEvent(type, init))
  }

  function click({ e, x, y, hit }) {
    for (const type of ['pointerover', 'mouseover', 'pointerdown', 'mousedown']) mouse(hit, type, x, y)
    e.focus?.({ preventScroll: true })
    for (const type of ['pointerup', 'mouseup', 'click']) mouse(hit, type, x, y)
  }

  // Synthetic keys run no default action, so Enter submits the form itself unless a handler took it.
  function enter(e) {
    const init = { key: 'Enter', code: 'Enter', bubbles: true, cancelable: true, composed: true }
    const key = (type) => {
      const event = new (e.ownerDocument.defaultView.KeyboardEvent)(type, init)
      Object.defineProperty(event, 'keyCode', { get: () => 13 })
      Object.defineProperty(event, 'which', { get: () => 13 })
      return e.dispatchEvent(event)
    }
    const proceed = key('keydown') && key('keypress')
    key('keyup')
    if (proceed && e.form) e.form.requestSubmit()
  }

  // A target an overlay still covers, or a control not enabled yet, is often a moment from usable
  // (a backdrop fading out, a button the form enables as it validates): looked for again for a
  // short while before the action is refused.
  const READY_MS = 600
  const pause = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
  async function usable(args) {
    const until = performance.now() + READY_MS
    for (;;) {
      try {
        return target(args)
      } catch (error) {
        if (!/^(covered|disabled)/.test(error.message) || performance.now() >= until) throw error
      }
      await pause(50)
    }
  }

  async function act(args) {
    begin()
    switch (args.op) {
      case 'scroll_down':
      case 'scroll_up':
        scrollBy({ top: (args.op === 'scroll_down' ? 0.8 : -0.8) * innerHeight, behavior: 'instant' })
        return { ok: true }
      case 'press_enter': {
        let e = args.ref ? (await usable(args)).e : document.activeElement
        // Focus inside a same-origin frame (an editor, a banner's field) is the frame's own.
        while (!args.ref && e?.contentDocument?.activeElement) e = e.contentDocument.activeElement
        if (!e || e === e.ownerDocument.body) throw new Error('no focused field to press Enter in')
        enter(e)
        return { ok: true }
      }
      case 'click':
        click(await usable(args))
        return { ok: true }
      case 'type':
        await fill((await usable(args)).e, String(args.value ?? ''))
        return { ok: true }
      case 'select': {
        const { e } = await usable(args)
        if (e.tagName !== 'SELECT') throw new Error('not a dropdown: select works on a native <select> only')
        // The option's value first; else its label, as the outline shows it (`Large` for `L`).
        const wanted = String(args.value ?? '')
        const choices = [...e.options].filter((o) => !o.disabled)
        const label = (o) => (o.label || o.text).trim()
        const option = choices.find((o) => o.value === wanted) ||
          choices.find((o) => label(o).toLowerCase() === wanted.trim().toLowerCase())
        if (!option) {
          const names = choices.slice(0, 20).map((o) => (label(o) === o.value ? o.value : `${o.value} (${label(o)})`))
          throw new Error(`no option "${wanted.slice(0, 80)}" in this dropdown; its options are: ${names.join(', ') || 'none'}`)
        }
        // Picking the option already chosen fires nothing in a browser; pages rely on that.
        if (option.selected) return { ok: true, unchanged: true }
        e.focus?.({ preventScroll: true })
        e.value = option.value
        e.dispatchEvent(new Event('input', { bubbles: true }))
        e.dispatchEvent(new Event('change', { bubbles: true }))
        return { ok: true }
      }
      default:
        throw new Error('unknown operation ' + args.op)
    }
  }

  // After an action: until the DOM has been quiet for 100 ms (at least 30 ms), at most 700 ms;
  // while a request the action started is out (a search's results, a save), until it is answered
  // and the page has drawn the answer, at most 2 s; after typing into a combobox, until its
  // options show (suggestions come from the network: up to 1.5 s, and only as long as none are
  // shown). A page that unloads ends it. Polled on a timer: animation frames stop in a window the
  // system considers in the background.
  const SETTLE_QUIET = 100
  const SETTLE_MAX = 700
  const SETTLE_ASKED = 2000
  const SETTLE_SUGGESTING = 1500
  function settle(args) {
    const field = args.ref ? nodeOf(args.ref) : null
    const suggesting = args.op === 'type' && /combobox|searchbox/.test(field?.getAttribute('role') || field?.type || '')
    return new Promise((resolve) => {
      const start = performance.now()
      let last = start
      let answered = start
      let done = false
      const observer = new MutationObserver(() => { last = performance.now() })
      for (const doc of [document, ...subframes().map((f) => f.doc)]) {
        observer.observe(doc, { subtree: true, childList: true, attributes: true, characterData: true })
      }
      const finish = () => {
        if (done) return
        done = true
        observer.disconnect()
        removeEventListener('pagehide', finish)
        // With the Navigation API every navigation of this document is seen here (`leaving`): the
        // native side need not wait for one the page did not report (ad frames request many).
        resolve({ ok: true, navigating: leaving, watched: !!window.navigation, ms: Math.round(performance.now() - start) })
      }
      addEventListener('pagehide', finish, { once: true })
      const poll = () => {
        if (done) return
        const now = performance.now()
        const out = awaited()
        if (out) answered = now
        const shown = !suggesting || [...document.querySelectorAll('[role="option"]')]
          .some((o) => visible(o) && inViewport(o.getBoundingClientRect()))
        const quiet = now - start >= 30 && now - last >= SETTLE_QUIET && now - answered >= SETTLE_QUIET
        const most = out ? SETTLE_ASKED
          : Math.max(suggesting ? SETTLE_SUGGESTING : SETTLE_MAX, Math.min(SETTLE_ASKED, answered - start + 3 * SETTLE_QUIET))
        if ((quiet && shown && !out) || now - start >= most) finish()
        else setTimeout(poll, 15)
      }
      setTimeout(poll, 15)
    })
  }

  // The whole document's readable text, a window of it at a time.
  function read(args) {
    const text = (document.body?.innerText || '').replace(UNSEEN, '').replace(/[ \t]+/g, ' ').replace(/\n{3,}/g, '\n\n')
    const offset = Math.max(0, args?.offset || 0)
    let end = offset + (args?.limit || 6000)
    // A window never ends inside an emoji: the next one starts where this one ended.
    const high = text.charCodeAt(end - 1)
    if (end < text.length && end - 1 > offset && high >= 0xd800 && high <= 0xdbff) end--
    return { text: text.slice(offset, end), total: text.length, offset }
  }


  // Where the element a screenshot is cropped to sits, in viewport pixels: scrolled into view
  // first when it is not wholly there, with a little of what surrounds it, clipped to the viewport.
  const CROP_MARGIN = 8
  function locate(args) {
    let e
    try { e = document.querySelector(args.selector) } catch { throw new Error(`not a CSS selector: ${args.selector}`) }
    if (!e) throw new Error(`nothing on the page matches ${args.selector}`)
    const was = e.getBoundingClientRect()
    if (was.top < 0 || was.left < 0 || was.bottom > innerHeight || was.right > innerWidth) {
      e.scrollIntoView({ block: 'center', inline: 'center', behavior: 'instant' })
    }
    const r = e.getBoundingClientRect()
    const left = Math.max(0, r.left - CROP_MARGIN), top = Math.max(0, r.top - CROP_MARGIN)
    const right = Math.min(innerWidth, r.right + CROP_MARGIN), bottom = Math.min(innerHeight, r.bottom + CROP_MARGIN)
    if (r.width < 1 || r.height < 1 || right - left < 1 || bottom - top < 1) {
      throw new Error(`${args.selector} has no visible size`)
    }
    return { x: left, y: top, width: right - left, height: bottom - top }
  }
