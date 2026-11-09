// The page side of the browser tool (files concatenated into one script by host.rs): an atomic
// snapshot of what a person can see and use, the execution of one action on an element that
// snapshot observed, and the person's comments. Installed before any page script runs, so what
// is kept here is the original of each builtin, not what a page puts in its place later: the IPC
// post, `JSON.stringify`, `Reflect.apply` and the DOM methods the comment card is built with.
// Two secrets come in with the script, and no page can read them out of this closure: the run's
// token, which seals every message to the native side, and the key a call must carry to be the
// native side's (a page may call `__dloop_call` too, and is refused). What a caller hands in does
// reach these files: a screenshot's CSS `selector` (`locate`) and the caller's script (`settled`);
// an action otherwise names an observed element by its ref.
;(() => {
  if (window.top !== window || window.__dloop_call) return
  const TOKEN = '__TOKEN__'
  const KEY = '__CALL_KEY__'
  // An invisible window nobody can see must not take what a person types elsewhere: real
  // (trusted) keys and text input are dropped. The probe's own events are synthetic and pass.
  if (__HEADLESS__) {
    for (const type of ['keydown', 'keypress', 'keyup', 'beforeinput', 'input', 'paste', 'compositionstart']) {
      addEventListener(type, (event) => {
        if (!event.isTrusted) return
        event.stopImmediatePropagation()
        event.preventDefault()
      }, { capture: true })
    }
  }

  // The builtins, as they are now. A page may later replace a prototype's method, getter or
  // iterator; the probe calls these instead, so what it hands them (its own elements, its
  // listeners, its replies) never reaches the page's code. Each is called as `f(self, ...args)`.
  const apply = Reflect.apply
  const described = Reflect.getOwnPropertyDescriptor
  const keysOf = Object.keys
  const isArray = Array.isArray
  const bare = () => Object.setPrototypeOf([], null)
  const hasOwn = Object.hasOwn || ((o, k) => apply(Object.prototype.hasOwnProperty, o, [k]))
  const part = (proto, name, kind) => {
    const d = proto && described(proto, name)
    const f = d && d[kind]
    return typeof f === 'function' ? (self, ...args) => apply(f, self, args) : null
  }
  const method = (proto, name) => part(proto, name, 'value')
  const getter = (proto, name) => part(proto, name, 'get')
  const setter = (proto, name) => part(proto, name, 'set')
  const later = setTimeout
  const nextFrame = requestAnimationFrame
  const then = Promise.prototype.then
  const resolved = Promise.resolve.bind(Promise)
  const dom = {
    make: method(Document.prototype, 'createElement'),
    pointed: method(Document.prototype, 'elementFromPoint'),
    allIn: method(Document.prototype, 'querySelectorAll'),
    count: getter(NodeList.prototype, 'length'),
    attr: method(Element.prototype, 'setAttribute'),
    shadow: method(Element.prototype, 'attachShadow'),
    remove: method(Element.prototype, 'remove'),
    matches: method(Element.prototype, 'matches'),
    rect: method(Element.prototype, 'getBoundingClientRect'),
    append: method(Node.prototype, 'appendChild'),
    contains: method(Node.prototype, 'contains'),
    parent: getter(Node.prototype, 'parentNode'),
    connected: getter(Node.prototype, 'isConnected'),
    text: setter(Node.prototype, 'textContent'),
    focus: method(HTMLElement.prototype, 'focus'),
    showPopover: method(HTMLElement.prototype, 'showPopover'),
    hidePopover: method(HTMLElement.prototype, 'hidePopover'),
    value: getter(HTMLTextAreaElement.prototype, 'value'),
    listen: method(EventTarget.prototype, 'addEventListener'),
    path: method(Event.prototype, 'composedPath'),
    prevent: method(Event.prototype, 'preventDefault'),
    stop: method(Event.prototype, 'stopPropagation'),
    stopAll: method(Event.prototype, 'stopImmediatePropagation'),
    key: getter(KeyboardEvent.prototype, 'key'),
    shift: getter(KeyboardEvent.prototype, 'shiftKey'),
  }

  // The native handler itself where there is one: `window.ipc.postMessage` looks it up again on
  // each call, so a page could put its own in between and read what is sent.
  const handler = window.webkit?.messageHandlers?.ipc
  const post = handler
    ? Function.prototype.call.bind(handler.postMessage, handler)
    : window.ipc.postMessage.bind(window.ipc)
  const stringify = JSON.stringify
  // A string cut in the middle of an emoji ends in half a surrogate pair, which JSON.stringify
  // writes as an escape the native side cannot parse: the whole reply would be dropped.
  const toWellFormed = method(String.prototype, 'toWellFormed')
  const replace = method(String.prototype, 'replace')
  const wellFormed = (s) => (toWellFormed
    ? toWellFormed(s)
    : replace(s, /[\ud800-\udbff][\udc00-\udfff]|[\ud800-\udfff]/g, (m) => (m.length === 2 ? m : '�')))
  // A message as data alone, before it is written: objects and arrays copied with no prototype,
  // so neither a page's `toJSON` on Object or Array nor a setter it put there rewrites a reply,
  // and functions left out as JSON leaves them out.
  const MAX_DEPTH = 64
  const plain = (v, depth = 0) => {
    if (typeof v === 'string') return wellFormed(v)
    if (typeof v === 'function' || typeof v === 'symbol') return undefined
    if (typeof v !== 'object' || v === null) return v
    if (depth > MAX_DEPTH) throw new TypeError('the reply is nested too deep')
    if (isArray(v)) {
      const out = bare()
      for (let i = 0; i < v.length; i++) out[i] = plain(v[i], depth + 1)
      return out
    }
    const out = Object.create(null)
    const names = keysOf(v)
    for (let i = 0; i < names.length; i++) out[wellFormed(names[i])] = plain(v[names[i]], depth + 1)
    return out
  }
  // Every message to the native side: the token in front as plain text (an object holding it
  // could be read by a page's `toJSON`), then the JSON.
  const send = (message) => post(`${TOKEN}\n${stringify(plain(message))}`)
