//! A page for the probe's checks, with no browser: a realm of its own (a page may change its
//! prototypes without touching the test's), the few DOM classes the probe keeps and calls, and the
//! probe installed first, as WebKit installs it, from the files host.rs concatenates.

import { readFileSync } from 'node:fs'
import { createContext, runInContext } from 'node:vm'

const HOST = readFileSync(new URL('../host.rs', import.meta.url), 'utf8')
const PROBE = [...HOST.slice(HOST.indexOf('pub(super) const PROBE')).matchAll(/include_str!\("probe\/([a-z]+\.js)"\)\s*,?\s*(\)?)/g)]
  .map(([, file]) => readFileSync(new URL(`./${file}`, import.meta.url), 'utf8'))
  .join('')

export const TOKEN = 'tok3n'
export const KEY = 'k3y'

// The DOM, in the page's realm: a class per prototype the probe reads a method from.
const DOM = String.raw`
// Native DOM methods never call a page's builtins: the fake's keep their own.
const kept = (f) => (list, ...args) => Reflect.apply(f, list, args)
const push = kept(Array.prototype.push), cut = kept(Array.prototype.splice), place = kept(Array.prototype.indexOf)
class EventTarget {
  constructor() { Object.defineProperty(this, '_l', { value: new Map() }) }
  addEventListener(type, f) { push(this._l.get(type) || this._l.set(type, []).get(type), f) }
  removeEventListener() {}
}
class Node extends EventTarget {
  constructor() { super(); this.childNodes = []; this._parent = null; this.nodeType = 1; this._text = '' }
  appendChild(child) {
    if (!(child instanceof Node)) throw new TypeError('not a node')
    if (child._parent) cut(child._parent.childNodes, place(child._parent.childNodes, child), 1)
    child._parent = this
    push(this.childNodes, child)
    return child
  }
  get parentNode() { return this._parent }
  contains(node) {
    if (!(node instanceof Node)) throw new TypeError('not a node')
    for (let n = node; n; n = n.parentNode) if (n === this) return true
    return false
  }
  get isConnected() { for (let n = this; n; n = n.parentNode) if (n === document) return true; return false }
  set textContent(text) { this._text = String(text) }
  get textContent() { return this._text + this.childNodes.map((c) => c.textContent).join('') }
}
class NodeList { get length() { return this._n } }
const listOf = (items) => { const list = new NodeList(); items.forEach((e, i) => { list[i] = e }); list._n = items.length; return list }
class Element extends Node {
  constructor(name) { super(); this.localName = name; this.tagName = name.toUpperCase(); this._attrs = new Map(); this._rect = { left: 0, top: 0, width: 0, height: 0 }; this.style = {} }
  setAttribute(name, value) { this._attrs.set(name, String(value)) }
  getAttribute(name) { return this._attrs.has(name) ? this._attrs.get(name) : null }
  hasAttribute(name) { return this._attrs.has(name) }
  get attributes() { return [...this._attrs].map(([name, value]) => ({ name, value })) }
  get id() { return this.getAttribute('id') || '' }
  get className() { return this.getAttribute('class') || '' }
  get parentElement() { return this.parentNode instanceof Element ? this.parentNode : null }
  get children() { return this.childNodes.filter((c) => c instanceof Element) }
  get innerText() { return this.textContent }
  attachShadow(init) {
    const root = new ShadowRoot(this)
    if (init.mode === 'open') this.shadowRoot = root
    push(roots, root)
    return root
  }
  remove() { if (this._parent) cut(this._parent.childNodes, place(this._parent.childNodes, this), 1); this._parent = null }
  matches() { return false }
  closest() { return null }
  querySelector() { return null }
  querySelectorAll() { return listOf([]) }
  getBoundingClientRect() { const r = this._rect; return { ...r, right: r.left + r.width, bottom: r.top + r.height } }
}
class HTMLElement extends Element {
  focus() { focused = this }
  showPopover() {}
  hidePopover() {}
}
class HTMLTextAreaElement extends HTMLElement {
  get value() { return this._value || '' }
  set value(v) { this._value = String(v) }
}
class DocumentFragment extends Node {}
class ShadowRoot extends DocumentFragment { constructor(host) { super(); this._host = host } get host() { return this._host } }
class Document extends Node {
  constructor() { super(); this.readyState = 'complete' }
  createElement(name) { return name === 'textarea' ? new HTMLTextAreaElement(name) : new HTMLElement(name) }
  elementFromPoint() { return pointed }
  querySelectorAll() { return listOf([]) }
  querySelector() { return null }
}
class Event {
  constructor(type, init = {}) { this.type = type; Object.assign(this, init); Object.defineProperty(this, 'isTrusted', { value: !!init.isTrusted }) }
  composedPath() { return this._path || [] }
  preventDefault() { this.defaultPrevented = true }
  stopPropagation() {}
  stopImmediatePropagation() {}
}
class KeyboardEvent extends Event { get key() { return this._key } get shiftKey() { return false } }
class MouseEvent extends Event {}
class MutationObserver { observe() {} }
class XMLHttpRequest { open() {} send() {} }
class HTMLCollection {}
const roots = []
let pointed = null
let focused = null
const posted = []
var document = new Document()
document.documentElement = document.appendChild(new HTMLElement('html'))
document.body = document.documentElement.appendChild(new HTMLElement('body'))
const listeners = new Map()
function addEventListener(type, f) { (listeners.get(type) || listeners.set(type, []).get(type)).push(f) }
var webkit = { messageHandlers: { ipc: { postMessage(body) { posted.push(body) } } } }
var performance = { timeOrigin: 1, now: () => 0 }
var location = { href: 'https://shop.test/', hostname: 'shop.test', protocol: 'https:' }
var history = { pushState() {}, replaceState() {} }
var innerWidth = 1000, innerHeight = 800
var CSS = { escape: (s) => s }
var getComputedStyle = () => ({ getPropertyValue: () => '', visibility: 'visible', display: 'block' })
var requestAnimationFrame = (f) => setTimeout(f, 0)
var fetch = () => Promise.resolve({})
`

/** A page with the probe installed; what it posted, and how the person and the native side act on it. */
export function page() {
  const sandbox: Record<string, unknown> = { setTimeout, clearTimeout, console }
  sandbox.window = sandbox
  sandbox.top = sandbox
  sandbox.self = sandbox
  const realm = createContext(sandbox)
  runInContext(DOM, realm)
  runInContext(PROBE.replace('__TOKEN__', TOKEN).replace('__CALL_KEY__', KEY).replace('__HEADLESS__', 'false'), realm)
  const run = (source: string) => runInContext(source, realm)
  let calls = 0
  return {
    run,
    /** Every message the probe posted, parsed, with whether it was sealed with the run's token. */
    posted: (): { sealed: boolean; message: Record<string, any> }[] =>
      run('posted').map((body: string) => {
        const [seal, ...json] = body.split('\n')
        return { sealed: seal === TOKEN, message: JSON.parse(json.join('\n')) }
      }),
    /** The native side's call, as webview.rs writes it: its key, a random id and its word. */
    call(name: string, args: unknown) {
      const id = `${++calls}${'7'.repeat(17)}`
      const word = `w${calls}`
      run(`window.__dloop_call(${JSON.stringify(KEY)}, ${JSON.stringify(id)}, ${JSON.stringify(word)}, ${JSON.stringify(name)}, ${JSON.stringify(args)})`)
      return { id, word }
    },
    /** A press of the person's (trusted) at `target`, from pointerdown to click. */
    press(target: string) {
      run(`pointed = ${target}
        for (const type of ['pointerdown', 'mousedown', 'pointerup', 'mouseup', 'click']) {
          const event = new MouseEvent(type, { isTrusted: true, clientX: 5, clientY: 5, _path: [pointed, document, window] })
          for (const f of listeners.get(type) || []) f(event)
        }`)
    },
  }
}

/** Let the page's promises and timers run. */
export const settled = () => new Promise((resolve) => setTimeout(resolve, 20))
