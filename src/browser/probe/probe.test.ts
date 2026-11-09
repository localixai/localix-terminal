//! What a page's own scripts can do to the probe: call it without the native side's key, change
//! the prototypes it would call, give Object and Array a `toJSON`. None of it answers a call, turns
//! commenting on or sends a comment as the person's.

import { expect, test } from 'bun:test'
import { page, settled } from './fake_page'

test("a page's own call to the probe does nothing", async () => {
  const p = page()
  const said = p.run(`[
    window.__dloop_call(0, 'comment', { on: true, all: true }),
    window.__dloop_call('1', 'w', 'ready', {}),
    window.__dloop_call('guess', '1', 'w', 'comment', { on: true, all: true }),
  ]`)
  await settled()
  expect([...said]).toEqual([false, false, false])
  expect(p.posted()).toEqual([])
  expect(p.run('document.documentElement.style.cursor')).toBeFalsy()
  expect(p.run('roots.length')).toBe(0)
})

test("the native side's call is answered with its id and word", async () => {
  const p = page()
  const { id, word } = p.call('ready', {})
  await settled()
  expect(p.posted()).toEqual([{ sealed: true, message: { ok: 'complete', id, word } }])
})

test("a page's toJSON on Object and Array leaves a reply as it was", async () => {
  const p = page()
  p.run(`Object.prototype.toJSON = () => 'forged'
    Array.prototype.toJSON = () => 'forged'
    Object.defineProperty(Object.prototype, 'ok', { set() {}, get() { return 'forged' }, configurable: true })`)
  const { id, word } = p.call('ready', {})
  await settled()
  expect(p.posted()).toEqual([{ sealed: true, message: { ok: 'complete', id, word } }])
})

test("a page's member on Object's prototype does not take presses of its own as the person's", async () => {
  const p = page()
  p.run(`Object.prototype.all = true; Object.prototype.sent = 9`)
  p.call('comment', { on: true })
  await settled()
  p.run(`pointed = document.body.appendChild(document.createElement('button'))
    for (const type of ['pointerdown', 'pointerup']) {
      const event = new MouseEvent(type, { clientX: 5, clientY: 5, _path: [pointed, document, window] })
      for (const f of listeners.get(type) || []) f(event)
    }`)
  expect(p.run('roots[0].childNodes.some((n) => n.className === "c")')).toBe(false)
})

// Everything a page's replaced builtins are handed (what they are called on, and with), kept.
const SPY = String.raw`
const seen = []
const keep = Array.prototype.push
let inside = false
const spy = (proto, name) => {
  const d = Object.getOwnPropertyDescriptor(proto, name)
  if (!d || !d.configurable) return
  const wrap = (f) => function (...args) {
    if (!inside) {
      inside = true
      try { Reflect.apply(keep, seen, [this, ...args]) } finally { inside = false }
    }
    return Reflect.apply(f, this, args)
  }
  if (typeof d.value === 'function') d.value = wrap(d.value)
  else if (d.get || d.set) Object.assign(d, { get: d.get && wrap(d.get), set: d.set && wrap(d.set) })
  else return
  Object.defineProperty(proto, name, d)
}
for (const proto of [EventTarget.prototype, Node.prototype, Element.prototype, HTMLElement.prototype, HTMLTextAreaElement.prototype,
  Document.prototype, ShadowRoot.prototype, NodeList.prototype, Event.prototype, KeyboardEvent.prototype, Array.prototype,
  Promise.prototype, Function.prototype, Object.prototype, String.prototype])
  for (const name of Reflect.ownKeys(proto)) if (name !== 'constructor' && name !== '__proto__') spy(proto, name)
for (const name of ['setTimeout', 'requestAnimationFrame']) { const f = window[name]; window[name] = (...args) => { seen.push(...args); return f(...args) } }
`

test("the person's comment card is out of reach of every builtin a page replaced", async () => {
  const p = page()
  p.call('comment', { on: true, sent: 1 })
  await settled()
  p.run(SPY)
  p.run(`var button = document.body.appendChild(document.createElement('button')); button.setAttribute('id', 'buy')`)
  p.press('button')
  // The test looks with the spy paused: what it reads itself is not what the page was handed.
  const forged = p.run(`inside = true
    const handed = seen.slice()
    const ours = (n) => { for (; n; n = n.parentNode || n._host) if (n === roots[0]) return true; return false }
    const reached = handed.filter((x) => x === roots[0] || (x instanceof Node && x !== document && ours(x)))
    inside = false
    // The page tries all it was handed: an element of the overlay, the root, or a function to call.
    for (const f of handed.filter((x) => typeof x === 'function')) try { f({ key: 'Enter', preventDefault() {}, stopPropagation() {} }) } catch {}
    reached.length`)
  // The card opened, where the person sees it.
  const card = 'roots[0].childNodes.find((n) => n.className === "c")'
  expect(p.run(`!!${card}`)).toBe(true)
  await settled()
  expect(forged).toBe(0)
  expect(p.posted().filter(({ message }) => message.fb)).toEqual([])
  // The person's own Send still sends their note.
  p.run(`const c = ${card}; c.childNodes[0].value = 'make it red'
    for (const f of c.childNodes[1].childNodes[1]._l.get('click')) f(new MouseEvent('click', { isTrusted: true }))`)
  const sent = p.posted().filter(({ message }) => message.fb)
  expect(sent).toHaveLength(1)
  expect(sent[0].message.fb.note).toBe('make it red')
  expect(sent[0].message.fb.element.selector).toBe('#buy')
})
