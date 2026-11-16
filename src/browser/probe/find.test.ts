//! Find in a web page (find.js), in the probe's stand-in page: what the toolbar's find row asks
//! of the page shown (`find`, from `Deck::seek`), the count it says back sealed, the matches lit
//! by highlights over ranges and never by changing the page, the one shown scrolled to.

import { expect, test } from 'bun:test'
import { page, settled } from './fake_page'

// Three paragraphs: the word twice in the first, in two cases; none in the second; once in the
// third. Once more where nobody sees it (a box with no rects) and once in a script.
const TEXT = `
  const para = (text) => {
    const p = document.body.appendChild(document.createElement('p'))
    p.appendChild(document.createTextNode(text))
    return p
  }
  const first = para('A needle and a NEEDLE.')
  para('Nothing here.')
  const third = para('The last needle')
  para('A needle out of sight')._hidden = true
  document.body.appendChild(document.createElement('script')).appendChild(document.createTextNode('needle()'))
`

function finder(text = TEXT) {
  const p = page()
  p.run(text)
  return p
}

// What the toolbar's row asks: a step of its query for tab 3, in the window's accent.
async function find(p: ReturnType<typeof page>, step: string, query = 'needle', accent = '#0a84ff') {
  p.call('find', { tab: 3, step, query, accent })
  await settled()
  expect(p.posted().filter(({ message }) => message.err)).toEqual([])
  const found = p.posted().filter(({ message }) => message.found)
  return found.at(-1)
}

const lit = (p: ReturnType<typeof page>, name: string) =>
  p.run(`CSS.highlights.get(${JSON.stringify(name)})?.ranges.map((r) => [r.startContainer.data, r.startOffset]) ?? null`)

test('typing finds what a person can see, case ignored, and says which is shown of how many', async () => {
  const p = finder()
  const said = await find(p, 'type')
  expect(said!.sealed).toBe(true)
  expect(said!.message.found).toEqual({ tab: 3, query: 'needle', at: 0, of: 3 })
  expect(lit(p, 'llxterm-find')).toEqual([
    ['A needle and a NEEDLE.', 2], ['A needle and a NEEDLE.', 15], ['The last needle', 9],
  ])
  // The one shown is lit apart, over the others.
  expect(lit(p, 'llxterm-find-shown')).toEqual([['A needle and a NEEDLE.', 2]])
  expect(p.run(`CSS.highlights.get('llxterm-find-shown').priority`)).toBeGreaterThan(0)
})

test('next and previous walk the matches, round from the last to the first', async () => {
  const p = finder()
  await find(p, 'type')
  const at = async (step: string) => (await find(p, step))!.message.found.at
  expect(await at('next')).toBe(1)
  expect(await at('next')).toBe(2)
  expect(await at('next')).toBe(0)
  expect(await at('previous')).toBe(2)
  expect(lit(p, 'llxterm-find-shown')).toEqual([['The last needle', 9]])
})

test('the match shown is scrolled to only when it is out of sight', async () => {
  const p = finder()
  p.run(`third._rect = { left: 0, top: 2400, width: 300, height: 16 }`)
  await find(p, 'type')
  expect(p.run('scrolled.length')).toBe(0)
  await find(p, 'next')
  await find(p, 'next')
  expect(p.run('scrolled.map(([element, how]) => [element === third, how.block])')).toEqual([[true, 'center']])
})

test('the page is never changed: the marks are highlights, their look an adopted sheet', async () => {
  const p = finder()
  const before = p.run('document.body.childNodes.map((n) => n.textContent).join("|")')
  const nodes = p.run('document.body.childNodes.length')
  await find(p, 'type')
  expect(p.run('document.body.childNodes.map((n) => n.textContent).join("|")')).toBe(before)
  expect(p.run('document.body.childNodes.length')).toBe(nodes)
  expect(p.run('first.childNodes.length')).toBe(1)
  const sheet = p.run('document.adoptedStyleSheets.map((s) => s.text).join("")') as string
  expect(sheet).toContain('::highlight(llxterm-find){background-color:rgb(10 132 255 / 25%)}')
  expect(sheet).toContain('::highlight(llxterm-find-shown){background-color:rgb(10 132 255 / 60%)}')
  // Closed, nothing of it is left.
  p.call('find', { tab: 3, step: 'clear', query: '' })
  await settled()
  expect(lit(p, 'llxterm-find')).toBe(null)
  expect(lit(p, 'llxterm-find-shown')).toBe(null)
  expect(p.run('document.adoptedStyleSheets.length')).toBe(0)
})

test("a page's own sheets stay adopted beside the marks' and after them", async () => {
  const p = finder()
  p.run('document.adoptedStyleSheets = [Object.assign(new CSSStyleSheet(), { text: "own" })]')
  await find(p, 'type')
  await find(p, 'next')
  expect(p.run('document.adoptedStyleSheets.length')).toBe(2)
  p.call('find', { tab: 3, step: 'clear', query: '' })
  await settled()
  expect(p.run('document.adoptedStyleSheets.map((s) => s.text)')).toEqual(['own'])
})

test('none found says so, and an empty query lights nothing', async () => {
  const p = finder()
  expect((await find(p, 'type', 'haystack'))!.message.found).toEqual({ tab: 3, query: 'haystack', at: -1, of: 0 })
  await find(p, 'type')
  expect((await find(p, 'type', ''))!.message.found).toEqual({ tab: 3, query: '', at: -1, of: 0 })
  expect(lit(p, 'llxterm-find')).toBe(null)
})

test('the query is words, never a pattern', async () => {
  const p = finder(`document.body.appendChild(document.createElement('p')).appendChild(document.createTextNode('a.b axb (a.b'))`)
  expect((await find(p, 'type', 'a.b'))!.message.found.of).toBe(2)
  expect((await find(p, 'type', '(a'))!.message.found.of).toBe(1)
})

test('found again (a new document, the tab shown again) keeps the match shown and moves nothing', async () => {
  const p = finder()
  p.run(`third._rect = { left: 0, top: 2400, width: 300, height: 16 }`)
  await find(p, 'type')
  await find(p, 'next')
  expect((await find(p, 'again'))!.message.found).toEqual({ tab: 3, query: 'needle', at: 1, of: 3 })
  expect(p.run('scrolled.length')).toBe(0)
  // A document that has not searched yet finds from its top, and scrolls to nothing.
  const fresh = finder()
  fresh.run(`first._rect = { left: 0, top: 2400, width: 300, height: 16 }`)
  expect((await find(fresh, 'again'))!.message.found.at).toBe(0)
  expect(fresh.run('scrolled.length')).toBe(0)
})

test('past the most it lights, the count says so', async () => {
  const p = finder(`document.body.appendChild(document.createElement('p')).appendChild(document.createTextNode('needle '.repeat(1200)))`)
  expect((await find(p, 'type'))!.message.found.of).toBe(1000)
})

test('an accent that is not a colour is the default one, never style', async () => {
  const p = finder()
  await find(p, 'type', 'needle', 'red}body{display:none')
  const sheet = p.run('document.adoptedStyleSheets.map((s) => s.text).join("")') as string
  expect(sheet).not.toContain('display')
  expect(sheet).toContain('rgb(10 132 255 / 25%)')
})

test("a page's own call to find does nothing, and finds out nothing", async () => {
  const p = finder()
  const said = p.run(`[
    window.__dloop_call(0, 'find', { tab: 3, step: 'type', query: 'needle' }),
    window.__dloop_call('guess', '1', 'w', 'find', { tab: 3, step: 'type', query: 'needle' }),
  ]`)
  await settled()
  expect([...said]).toEqual([false, false])
  expect(p.posted()).toEqual([])
  expect(lit(p, 'llxterm-find')).toBe(null)
})

test('a call nobody waits for (id 0) is not answered, so what the page told waits for a real reply', async () => {
  const p = finder()
  p.run(`window.__dloop_call('k3y', '0', '', 'find', { tab: 3, step: 'type', query: 'needle' })`)
  await settled()
  expect(p.posted().map(({ message }) => Object.keys(message))).toEqual([['found']])
})
