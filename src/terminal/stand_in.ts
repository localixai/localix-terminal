//! The terminal page's own script (the glue terminal.rs runs after xterm.js and its search addon)
//! in a stand-in page, for the bun tests: a stand-in terminal, search addon and document, with
//! what the script posts, the keys it is handed and the elements it changes kept to look at.

import { readFileSync } from 'node:fs'
import { runInNewContext } from 'node:vm'

// The glue's files, in the order terminal.rs joins them (`GLUE`).
const module = readFileSync(new URL('../terminal.rs', import.meta.url), 'utf8')
const joined = module.slice(module.indexOf('pub(crate) const GLUE'), module.indexOf('pub(crate) const GLUE_SHA384'))
const glue = [...joined.matchAll(/include_str!\("([^"]+)"\)/g)]
  .map(([, path]) => readFileSync(new URL('../' + path, import.meta.url), 'utf8'))
  .join('\n')

export type Key = { type?: string, key?: string, code?: string, altKey?: boolean, shiftKey?: boolean,
  metaKey?: boolean, ctrlKey?: boolean, isComposing?: boolean }
type Run = (event: Record<string, unknown>) => void
type Event = Record<string, unknown> & { defaultPrevented?: boolean }

/// What the page's colours resolve to: the system's accent and the terminal's dark ground.
const RESOLVED: Record<string, string> = { 'var(--accent)': 'rgb(10, 132, 255)', 'var(--bg)': 'rgb(30, 30, 30)' }

export function page(look: Record<string, unknown> = {}) {
  const posted: string[] = []
  const listeners = new Map<string, Run[]>()
  const counts = { cleared: 0, unselected: 0 }
  const osc = new Map<number, (data: string) => boolean>()
  const buffers: ((buffer: { type: string }) => void)[] = []
  const searched: unknown[][] = []
  let results: (result: { resultIndex: number, resultCount: number }) => void = () => {}
  let keys: (event: unknown) => boolean = () => true
  let alternate = false
  const at: { active: Element | null } = { active: null }
  class Element {
    hidden = false
    value = ''
    textContent = ''
    style: Record<string, string> = {}
    attributes = new Set<string>()
    heard = new Map<string, Run[]>()
    constructor(public id = '') {}
    append() {}
    setAttribute(name: string) { this.attributes.add(name) }
    toggleAttribute(name: string, on: boolean) { on ? this.attributes.add(name) : this.attributes.delete(name) }
    addEventListener(kind: string, run: Run) { this.heard.set(kind, [...(this.heard.get(kind) ?? []), run]) }
    focus() { at.active = this }
    select() { this.attributes.add('selected') }
    // An event at this element, as the page's own listeners take it.
    dispatch(kind: string, event: Record<string, unknown> = {}) {
      const told: Event = { target: this, ...event }
      told.preventDefault = () => { told.defaultPrevented = true }
      told.stopPropagation = () => {}
      for (const run of this.heard.get(kind) ?? []) run(told)
      return told
    }
  }
  const elements = new Map<string, Element>()
  const byId = (id: string) => elements.get(id) ?? elements.set(id, new Element(id)).get(id)!
  byId('find').hidden = true
  const root = new Element('html')
  class Terminal {
    options: Record<string, unknown>
    textarea = new Element('textarea')
    buffer = {
      get active() { return { type: alternate ? 'alternate' : 'normal', getLine: () => undefined } },
      onBufferChange(run: (buffer: { type: string }) => void) { buffers.push(run) },
    }
    parser = {
      registerCsiHandler() {},
      registerOscHandler(id: number, handler: (data: string) => boolean) { osc.set(id, handler) },
    }
    cols = 80
    rows = 24
    constructor(options: Record<string, unknown>) { this.options = { ...options } }
    open() {}
    onData() {}
    onResize() {}
    registerLinkProvider() {}
    attachCustomKeyEventHandler(handler: (event: unknown) => boolean) { keys = handler }
    loadAddon(addon: { activate(term: Terminal): void }) { addon.activate(this) }
    clear() { counts.cleared++ }
    clearSelection() { counts.unselected++ }
    focus() { at.active = this.textarea }
    hasSelection() { return false }
    selectAll() {}
    scrollToTop() {}
    scrollToBottom() {}
  }
  class SearchAddon {
    constructor(public options: Record<string, unknown> = {}) {}
    activate() {}
    findNext(query: string, options: unknown) { searched.push(['next', query, options]); return true }
    findPrevious(query: string, options: unknown) { searched.push(['previous', query, options]); return true }
    clearDecorations() { searched.push(['clear']) }
    onDidChangeResults(run: typeof results) { results = run }
  }
  const context: Record<string, unknown> = {
    Terminal,
    SearchAddon: { SearchAddon },
    __deskLook: { mono: 'Menlo', size: 13, reader: false, ...look },
    ipc: { postMessage: (text: string) => posted.push(text) },
    document: {
      title: 'session',
      documentElement: root,
      createElement: () => new Element(),
      getElementById: byId,
      addEventListener() {},
      fonts: { ready: new Promise(() => {}) },
      get activeElement() { return at.active },
    },
    getComputedStyle: (element: Element) => ({
      getPropertyValue: () => '#000000',
      color: RESOLVED[element?.style?.color ?? ''] ?? '',
    }),
    matchMedia: () => ({ addEventListener() {} }),
    MutationObserver: class { observe() {} },
    addEventListener: (kind: string, run: Run) => {
      listeners.set(kind, [...(listeners.get(kind) ?? []), run])
    },
    requestAnimationFrame: () => 0,
    cancelAnimationFrame() {},
    setTimeout: () => 0,
    clearTimeout() {},
    innerWidth: 0,
    innerHeight: 0,
  }
  context.window = context
  runInNewContext(glue + '\nthis.term = term', context)
  posted.length = 0
  const term = context.term as Terminal
  const tell = (kind: string, event: unknown = {}) => (listeners.get(kind) ?? []).forEach(run => run(event as Record<string, unknown>))
  // A key as WebKit hands it to xterm's handler: whether xterm goes on with it (true) or the
  // page took it (false).
  const press = (event: Key) => keys({ type: 'keydown', preventDefault() {}, ...event })
  return {
    posted, counts, term, osc, press, tell, searched, root, at,
    find: byId,
    // The page's own call, as the window makes it (`Deck::find_text`).
    findText: (step: string) => (context.findText as (step: string) => void)(step),
    results: (resultIndex: number, resultCount: number) => results({ resultIndex, resultCount }),
    full: (on: boolean) => {
      alternate = on
      buffers.forEach(run => run({ type: on ? 'alternate' : 'normal' }))
    },
  }
}
