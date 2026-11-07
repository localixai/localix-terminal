//! A cancelled FLIP animation must not remove the tracking for its newer replacement.

import { expect, test } from 'bun:test'
import { readFileSync } from 'node:fs'
import { runInNewContext } from 'node:vm'

const page = readFileSync(new URL('./strip.html', import.meta.url), 'utf8')
const motion = page.slice(page.indexOf('const sliding ='), page.indexOf('function side(data)'))

test('a late animation cancellation cannot forget the next animation on that row', () => {
  type Animation = { cancel: () => void; oncancel?: () => void; onfinish?: () => void }
  const runs: Animation[] = [], cancelled: (() => void)[] = []
  let top = 0
  const row = {
    getBoundingClientRect: () => ({ top }),
    animate: () => {
      const run: Animation = { cancel: () => cancelled.push(() => run.oncancel?.()) }
      runs.push(run)
      return run
    },
  }
  const context = {
    listRows: () => [row],
    calm: () => false,
    document: { documentElement: {} },
    getComputedStyle: () => ({ getPropertyValue: (name: string) => name === '--t-move' ? '200ms' : 'ease' }),
    change: () => { top += 10 },
  }
  const ui = runInNewContext(motion + '\n({flip, size: () => sliding.size})', context)
  ui.flip(context.change)
  ui.flip(context.change)
  expect(ui.size()).toBe(1)
  expect(runs).toHaveLength(2)
  cancelled.forEach(run => run())
  expect(ui.size()).toBe(1)
  runs[1].onfinish?.()
  expect(ui.size()).toBe(0)
})
