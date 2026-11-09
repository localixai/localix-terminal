  // `evaluate`: the caller's script runs in the page realm and its value comes back
  // as compact text (see `script_forms` in script.rs for how it is started).
  const MAX_EVAL = 8000
  function serialize(v) {
    const cap = (s) => (s.length > MAX_EVAL ? `${s.slice(0, MAX_EVAL)}… [truncated, ${s.length} chars]` : s)
    if (v === undefined) return 'undefined'
    if (v === null || ['number', 'boolean', 'bigint'].includes(typeof v)) return String(v)
    if (typeof v === 'string') return cap(v)
    if (v instanceof Error) return cap(`${v.name}: ${v.message}`)
    if (v instanceof Element) return cap(`<${v.tagName.toLowerCase()}> ${(v.textContent || '').trim().replace(/\s+/g, ' ')}`)
    if (v instanceof NodeList || v instanceof HTMLCollection) return cap([...v].map(serialize).join('\n'))
    try {
      // The objects from the root down to the one being written: only one that holds itself is
      // circular, an object met twice in different places is written twice. `this` is the object
      // a value is a member of, so the way back up is known; `held` is what stands for a value in
      // the text (a Map as its entries), which is what its members are members of.
      const path = []
      return cap(stringify(v, function (_k, x) {
        while (path.length && path[path.length - 1].held !== this) path.pop()
        if (typeof x === 'bigint') return String(x)
        if (typeof x !== 'object' || x === null) return x
        if (path.some((step) => step.from === x)) return '[Circular]'
        const held = x instanceof Map ? [...x.entries()] : x instanceof Set ? [...x]
          : x instanceof Error ? `${x.name}: ${x.message}` : x
        path.push({ from: x, held })
        return held
      }, 2) ?? '[unserializable]')
    } catch {
      return '[unserializable]'
    }
  }
  // The caller's script, compiled by the native side (which a page's CSP cannot block), is
  // started here once the probe watches what it sets off (its requests, the page's
  // announcements); its value comes back over IPC like any other answer.
  async function settled(start) {
    begin()
    try {
      return { result: serialize(await start()) }
    } catch (error) {
      throw new Error(`the script threw: ${error?.message || error}`)
    }
  }

  // A popup that closes itself (a sign-in done) has its tab closed: WebKit tells no one of it, so
  // the page says so, with its address, which names its tab.
  if (window.opener) {
    Object.defineProperty(window, 'close', {
      value() { send({ close: location.href }) },
    })
  }

  const ready = () => document.readyState
  const CALLS = { __proto__: null, snapshot, epoch, act, settle, read, settled, ready, keys, locate, comment, answer }
  // The one way in from the native side: a call carries the run's call key (a page's own call to
  // this function, without it, does nothing), its id, and the word its reply must carry back,
  // which only the native side and the page it was sent to ever see: a page in another tab never
  // answers for this one. What the page told since the last reply goes with this one (told.js).
  Object.defineProperty(window, '__dloop_call', {
    value(key, id, word, name, args) {
      if (key !== KEY || typeof id !== 'string' || typeof word !== 'string') return false
      const call = CALLS[name]
      const reply = (body) => send({ ...body, id, word, ...(told.length ? { told: told.splice(0) } : {}) })
      const started = apply(then, resolved(), [() => call(args)])
      apply(then, started, [(ok) => reply({ ok: ok ?? null }), (error) => reply({ err: String(error?.message || error) })])
      return true
    },
  })
})()
