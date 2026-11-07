//! Resize gesture state and frame scheduling, independent of a DOM or a native window.

import { expect, test } from 'bun:test'
import { readFileSync } from 'node:fs'
import { runInNewContext } from 'node:vm'

const source = readFileSync(new URL('./resize.js', import.meta.url), 'utf8')

function fixture() {
  type Pointer = { pointerId: number; button: number; buttons: number; clientX: number; screenX: number; preventDefault: () => void }
  type Handler = (event: Pointer & { key?: string }) => void
  class Target {
    handlers = new Map<string, Handler[]>()
    captured: number | null = null
    addEventListener(kind: string, handler: Handler) {
      this.handlers.set(kind, [...(this.handlers.get(kind) ?? []), handler])
    }
    emit(kind: string, values = {}) {
      const event = { pointerId: 1, button: 0, buttons: 1, clientX: 0, screenX: 0, preventDefault() {}, ...values }
      for (const handler of this.handlers.get(kind) ?? []) handler(event)
    }
    setPointerCapture(id: number) { this.captured = id }
    hasPointerCapture(id: number) { return this.captured === id }
    releasePointerCapture(id: number) {
      this.captured = null
      this.emit('lostpointercapture', { pointerId: id })
    }
  }
  const window = new Target(), handle = new Target()
  const frames = new Map<number, () => void>()
  let serial = 0
  const starts: number[] = [], moves: number[] = [], ends: [number, boolean][] = []
  const context = {
    addEventListener: window.addEventListener.bind(window),
    requestAnimationFrame: (run: () => void) => { frames.set(++serial, run); return serial },
    cancelAnimationFrame: (id: number) => frames.delete(id),
    handle,
    callbacks: {
      start: (event: Pointer) => { starts.push(event.clientX) },
      move: (event: Pointer) => { moves.push(event.clientX) },
      end: (event: Pointer, cancelled: boolean) => { ends.push([event.clientX, cancelled]) },
    },
  }
  runInNewContext(source + '\nresizeHandle(handle, callbacks)', context)
  const tick = () => {
    const waiting = [...frames.values()]
    frames.clear()
    waiting.forEach(run => run())
  }
  return { window, handle, frames, starts, moves, ends, tick }
}

test('moves coalesce, idle drags schedule nothing, and release uses its own coordinates', () => {
  const f = fixture()
  f.handle.emit('pointerdown', { clientX: 10 })
  expect(f.frames.size).toBe(0)
  f.window.emit('pointermove', { clientX: 20 })
  f.window.emit('pointermove', { clientX: 30 })
  expect(f.frames.size).toBe(1)
  f.tick()
  expect(f.moves).toEqual([30])
  expect(f.frames.size).toBe(0)
  f.window.emit('pointermove', { clientX: 40 })
  f.window.emit('pointerup', { clientX: 50, buttons: 0 })
  f.tick()
  expect(f.moves).toEqual([30])
  expect(f.ends).toEqual([[50, false]])
  expect(f.handle.captured).toBeNull()
})

test('other pointers cannot move or finish the gesture', () => {
  const f = fixture()
  f.handle.emit('pointerdown', { clientX: 10 })
  f.window.emit('pointermove', { pointerId: 2, clientX: 20 })
  f.window.emit('pointerup', { pointerId: 2 })
  f.tick()
  expect(f.moves).toEqual([])
  expect(f.ends).toEqual([])
  f.window.emit('pointerup', { clientX: 30 })
  expect(f.ends).toEqual([[30, false]])
})

test('releasing another button does not end the primary drag', () => {
  const f = fixture()
  f.handle.emit('pointerdown')
  f.window.emit('pointerup', { button: 2 })
  expect(f.ends).toEqual([])
  expect(f.handle.captured).toBe(1)
  f.window.emit('pointerup', { clientX: 30 })
  expect(f.ends).toEqual([[30, false]])
})

for (const kind of ['pointercancel', 'blur', 'keydown', 'lostpointercapture', 'pointermove']) {
  test(`${kind} cancels pending work without leaving a held handle`, () => {
    const f = fixture()
    f.handle.emit('pointerdown', { clientX: 10 })
    f.window.emit('pointermove', { clientX: 20 })
    const target = kind === 'lostpointercapture' ? f.handle : f.window
    target.emit(kind, { key: 'Escape', buttons: 0 })
    f.tick()
    expect(f.ends).toHaveLength(1)
    expect(f.ends[0][1]).toBe(true)
    expect(f.moves).toEqual([])
    expect(f.frames.size).toBe(0)
    expect(f.handle.captured).toBeNull()
  })
}
