//! The terminal page's own script (the glue terminal.rs runs after xterm.js), in the stand-in
//! page (stand_in.ts): what the person's keys send, what ⌘K does to a full-screen program, and
//! what a program's copy (OSC 52) asks of the window.

import { expect, test } from 'bun:test'
import { page } from './stand_in'

test('Shift+Enter sends LF, a newline to a coding agent and Enter to a shell; Enter alone is left to xterm', () => {
  const p = page()
  expect(p.press({ key: 'Enter', code: 'Enter', shiftKey: true })).toBe(false)
  expect(p.posted).toEqual(['k\n'])
  p.posted.length = 0
  expect(p.press({ key: 'Enter', code: 'Enter' })).toBe(true)
  // An input method still composing takes its own Return.
  expect(p.press({ key: 'Enter', code: 'Enter', shiftKey: true, isComposing: true })).toBe(true)
  // Shift with another modifier is not this key.
  expect(p.press({ key: 'Enter', code: 'Enter', shiftKey: true, metaKey: true })).toBe(true)
  expect(p.posted).toEqual([])
})

test('Option+Enter sends ESC CR only from an Option that is Meta; another is Enter', () => {
  for (const [option, side, sent] of [
    ['left', 'AltLeft', '\x1b\r'], ['left', 'AltRight', '\r'], ['right', 'AltRight', '\x1b\r'],
    ['both', 'AltRight', '\x1b\r'], ['off', 'AltLeft', '\r'],
  ]) {
    const p = page({ option })
    p.press({ key: 'Alt', code: side, altKey: true })
    expect(p.press({ key: 'Enter', code: 'Enter', altKey: true })).toBe(false)
    expect([option, side, p.posted]).toEqual([option, side, ['k' + sent]])
  }
})

test('neither Option is Meta until the person chooses one, so a layout keeps its @ [ ] { } | \\ ~', () => {
  const meta = (p: ReturnType<typeof page>, side: string) => {
    p.press({ key: 'Alt', code: side, altKey: true })
    expect(p.press({ key: '∫', code: 'KeyB', altKey: true })).toBe(true)
    const was = p.term.options.macOptionIsMeta
    p.press({ type: 'keyup', key: 'Alt', code: side })
    return was
  }
  const p = page()
  expect([meta(p, 'AltLeft'), meta(p, 'AltRight')]).toEqual([false, false])
  for (const [option, left, right] of [['left', true, false], ['right', false, true], ['both', true, true], ['off', false, false]]) {
    // Settings changed: the open terminals are told at once.
    p.tell('desk-look', { detail: { mono: 'Menlo', size: 13, option } })
    expect([meta(p, 'AltLeft'), meta(p, 'AltRight')]).toEqual([left, right])
  }
  // A terminal opened after the change reads it from its start.
  const later = page({ option: 'right' })
  expect([meta(later, 'AltLeft'), meta(later, 'AltRight')]).toEqual([false, true])
})

test('Ctrl+Option and a key take that Option\'s own setting', () => {
  const p = page({ option: 'right' })
  p.press({ key: 'Alt', code: 'AltRight', altKey: true })
  expect(p.press({ key: 'b', code: 'KeyB', altKey: true, ctrlKey: true })).toBe(true)
  expect(p.term.options.macOptionIsMeta).toBe(true)
  p.press({ type: 'keyup', key: 'Alt', code: 'AltRight' })
  p.press({ key: 'Alt', code: 'AltLeft', altKey: true })
  expect(p.press({ key: 'b', code: 'KeyB', altKey: true, ctrlKey: true })).toBe(true)
  expect(p.term.options.macOptionIsMeta).toBe(false)
})

test('an Option whose release the page never saw is not taken for held after', () => {
  const p = page({ option: 'right' })
  p.press({ key: 'Alt', code: 'AltRight', altKey: true })
  p.tell('blur')
  // Seen with Option but not which: the left one, the one most pressed.
  p.press({ key: '∫', code: 'KeyB', altKey: true })
  expect(p.term.options.macOptionIsMeta).toBe(false)
  // Its keyup lost with the page keeping the keys: the next key without Option forgets it.
  p.press({ key: 'Alt', code: 'AltRight', altKey: true })
  p.press({ key: 'x', code: 'KeyX' })
  p.press({ key: '∫', code: 'KeyB', altKey: true })
  expect(p.term.options.macOptionIsMeta).toBe(false)
})

test('⌘K clears a shell but leaves a full-screen program as it drew itself', () => {
  const p = page()
  expect(p.press({ key: 'k', code: 'KeyK', metaKey: true })).toBe(false)
  expect([p.counts.cleared, p.posted]).toEqual([1, ['k\f']])
  p.posted.length = 0
  p.full(true)
  expect(p.press({ key: 'k', code: 'KeyK', metaKey: true })).toBe(false)
  expect([p.counts.cleared, p.posted]).toEqual([1, []])
  // Its menu has no Clear then: what does nothing is not offered.
  p.tell('contextmenu', { preventDefault() {}, clientX: 1, clientY: 2 })
  expect(p.posted).toHaveLength(1)
  expect(p.posted[0]).not.toContain('Clear')
})

test("a program's copy goes to the window to decide; a read is never answered", () => {
  const p = page()
  const copy = p.osc.get(52)
  expect(copy).toBeDefined()
  expect(copy!('c;aGk=')).toBe(true)
  expect(copy!(';aGk=')).toBe(true)
  expect(p.posted).toEqual(['c aGk=', 'c aGk='])
  p.posted.length = 0
  for (const asked of ['c;?', '?', '', 'c;']) expect(copy!(asked)).toBe(true)
  expect(p.posted).toEqual([])
  // No more than the window takes (panel.rs `COPIED_MAX`, 1 MiB, in base64) is posted at all.
  const most = 'A'.repeat(1398104)
  expect(copy!('c;' + most)).toBe(true)
  expect(copy!('c;' + most + 'AAAA')).toBe(true)
  expect(p.posted.map(sent => sent.length)).toEqual([2 + most.length])
  expect(p.term.options.macOptionClickForcesSelection).toBe(true)
})
