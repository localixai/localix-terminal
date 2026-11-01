  // What the page tells the person besides its content: a native dialog (alert, confirm, prompt),
  // which a window nobody sits at never shows, and words it announces in a live region after an
  // action (a toast, "Saved", "Added to cart"), which may be gone by the next look. Each reply of
  // the probe carries what was told since the last one (`told`, drained in evaluate.js), so the
  // native side keeps it whichever document the tab goes on to. A confirm or a prompt is answered
  // Cancel unless the caller armed an answer (`answer`): nothing is agreed to on a page's say-so.
  const MAX_TOLD = 8
  const told = []
  const tell = (notice) => {
    const last = told[told.length - 1]
    if (told.length < MAX_TOLD && !(last && last.kind === notice.kind && last.text === notice.text)) told.push(notice)
  }
  const clip = (s, n = 300) => String(s ?? '').replace(/\s+/g, ' ').trim().slice(0, n)
  let accept = false
  let promptText = null

  // The dialogs of `view` (this window, or a same-origin frame's) answered here.
  function quiet(view) {
    try {
      if (!view || view.__dloop_quiet) return
      Object.defineProperty(view, '__dloop_quiet', { value: true })
      view.alert = function (message) { tell({ kind: 'alert', text: clip(message) }) }
      view.confirm = function (message) {
        tell({ kind: 'confirm', text: clip(message), answer: accept ? 'OK' : 'Cancel' })
        return accept
      }
      view.prompt = function (message, value) {
        const given = accept ? String(promptText ?? value ?? '') : null
        tell({ kind: 'prompt', text: clip(message), answer: given === null ? 'Cancel' : `OK, "${clip(given, 80)}"` })
        return given
      }
    } catch {}
  }
  quiet(window)

  // `answer`: how this document answers a confirm or a prompt until told otherwise: OK (with
  // `text` for a prompt), or Cancel.
  function answer(args) {
    accept = !!args?.accept
    promptText = typeof args?.text === 'string' ? args.text : null
    quietFrames()
    return { accept }
  }

  // The same-origin frames' dialogs, answered here too: a frame's own document has no probe.
  function quietFrames() {
    try {
      for (const { frame } of subframes()) quiet(frame.contentWindow)
    } catch {}
  }

  // A live region's words, told when they change while an action's aftermath lasts.
  const LIVE = '[aria-live]:not([aria-live="off"]),[role="alert"],[role="status"],[role="log"],[role="alertdialog"],output'
  const ANNOUNCE_MS = 5000
  const CONTROLS = 'a[href],button,input,select,textarea,[role="button"],[role="link"],[role="checkbox"]'
  let listening = -Infinity
  // What each region last said, within one action's aftermath: the same toast shown again by a
  // later action (a second "Order placed") is told again.
  let announced = new WeakMap()
  const listen = () => {
    listening = performance.now() + ANNOUNCE_MS
    announced = new WeakMap()
  }
  new MutationObserver((records) => {
    if (performance.now() > listening) return
    const regions = new Set()
    for (const record of records) {
      const at = record.target.nodeType === 1 ? record.target : record.target.parentElement
      const region = at?.closest?.(LIVE)
      if (region) regions.add(region)
      for (const node of record.addedNodes) {
        const inner = node.nodeType === 1 && (node.matches(LIVE) ? node : node.querySelector(LIVE))
        if (inner) regions.add(inner)
      }
    }
    for (const region of regions) {
      // A region with controls in it is content (a feed, a list marked live), not a message: its
      // changes are in the snapshot, and its words read as an announcement only mislead.
      if (region.querySelector(CONTROLS)) continue
      const text = clip(region.innerText || region.textContent, 200)
      if (!text || announced.get(region) === text || /^(loading|please wait|searching)\b/i.test(text)) continue
      announced.set(region, text)
      tell({ kind: 'said', text })
    }
  }).observe(document, { subtree: true, childList: true, characterData: true })

  // A field the page refused: marked invalid (`aria-invalid`), or one the browser found invalid
  // when its form was sent (the `invalid` event: the browser's own bubble is never seen here),
  // with the message that goes with it. Null for a field that is not refused.
  const refused = new WeakSet()
  addEventListener('invalid', (event) => { if (event.target?.nodeType === 1) refused.add(event.target) }, { capture: true })
  const userInvalid = (e) => { try { return e.matches(':user-invalid') } catch { return false } }
  const wordsOf = (e, attribute) => (e.getAttribute(attribute) || '').split(/\s+/).filter(Boolean)
    .map((id) => e.ownerDocument.getElementById(id)).filter((n) => n && visible(n))
    .map((n) => n.innerText).join(' ')
  function invalidOf(e) {
    const native = e.validity && !e.validity.valid && (refused.has(e) || userInvalid(e))
    if (e.getAttribute('aria-invalid') !== 'true' && !native) return null
    const box = e.closest('label,fieldset,.form-group,.field') || e.parentElement
    const near = [...(box?.querySelectorAll('[role="alert"],.invalid-feedback,.error,.error-message,.field-error') || [])]
      .filter((n) => n !== e && visible(n)).map((n) => n.innerText).join(' ')
    return clip(wordsOf(e, 'aria-errormessage') || near || wordsOf(e, 'aria-describedby') || e.validationMessage, 100)
  }
