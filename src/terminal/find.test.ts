//! Find in a terminal's output (find.js), in the stand-in page (stand_in.ts): the bar the
//! window's Find opens, what typing, Return, Shift+Return and Esc ask of the search addon, the
//! count it says, its colours, and where the keys are.

import { expect, test } from 'bun:test'
import { page } from './stand_in'

// The page with its bar open and `query` typed into the field, as a person types it.
function typed(query: string) {
  const p = page()
  p.findText('open')
  const field = p.find('find-q')
  field.value = query
  field.dispatch('input')
  return p
}

test('Find opens the bar under the screen with the keys in its field; Esc gives them back', () => {
  const p = page()
  const bar = p.find('find')
  expect(bar.hidden).toBe(true)
  p.findText('open')
  expect(bar.hidden).toBe(false)
  expect(p.root.attributes.has('data-find')).toBe(true)
  expect(p.at.active).toBe(p.find('find-q'))
  expect(p.find('find-q').attributes.has('selected')).toBe(true)
  // The window in front again: the keys stay in the field while the bar has them.
  p.tell('focus')
  expect(p.at.active).toBe(p.find('find-q'))
  const esc = p.find('find-q').dispatch('keydown', { key: 'Escape' })
  expect(esc.defaultPrevented).toBe(true)
  expect(bar.hidden).toBe(true)
  expect(p.root.attributes.has('data-find')).toBe(false)
  expect(p.at.active).toBe(p.term.textarea)
  expect(p.searched.at(-1)).toEqual(['clear'])
  // Nothing typed in the bar went to the program.
  expect(p.posted).toEqual([])
})

test('typing searches as it goes, case ignored, every match lit; an empty field clears', () => {
  const p = typed('needle')
  const [step, query, options] = p.searched.at(-1) as [string, string, Record<string, unknown>]
  expect([step, query]).toEqual(['next', 'needle'])
  expect(options.incremental).toBe(true)
  expect(options.caseSensitive).toBe(false)
  expect(options.decorations).toBeDefined()
  p.find('find-q').value = ''
  p.find('find-q').dispatch('input')
  expect(p.searched.at(-1)).toEqual(['clear'])
  expect(p.find('find-n').textContent).toBe('')
})

test('Return and the window\'s Find Next go on, Shift+Return and Find Previous go back', () => {
  const p = typed('needle')
  const steps = () => p.searched.map(said => said[0])
  p.searched.length = 0
  const enter = p.find('find-q').dispatch('keydown', { key: 'Enter' })
  expect(enter.defaultPrevented).toBe(true)
  p.find('find-q').dispatch('keydown', { key: 'Enter', shiftKey: true })
  p.findText('next')
  p.findText('previous')
  expect(steps()).toEqual(['next', 'previous', 'next', 'previous'])
  // A step goes from the match shown, never anew from it.
  expect((p.searched[0][2] as Record<string, unknown>).incremental).toBeFalsy()
  // Find again over an open bar selects what it holds, to type over.
  p.find('find-q').attributes.delete('selected')
  p.findText('open')
  expect(p.find('find-q').attributes.has('selected')).toBe(true)
  expect(p.posted).toEqual([])
})

test('Find Next with the bar shut opens it and goes to the next match of what it held', () => {
  const p = typed('needle')
  p.find('find-q').dispatch('keydown', { key: 'Escape' })
  p.searched.length = 0
  p.findText('next')
  expect(p.find('find').hidden).toBe(false)
  expect(p.searched).toHaveLength(1)
  expect(p.searched[0].slice(0, 2)).toEqual(['next', 'needle'])
})

test('the count says which match is shown of how many, or that there is none', () => {
  const p = typed('needle')
  const count = () => p.find('find-n').textContent
  p.results(2, 12)
  expect(count()).toBe('3 of 12')
  p.results(-1, 0)
  expect(count()).toBe('No results')
  // Past the most the addon lights (1,000), the count says so.
  p.results(4, 1000)
  expect(count()).toBe('5 of 1,000+')
  p.results(-1, 1000)
  expect(count()).toBe('1,000+')
  p.results(-1, 7)
  expect(count()).toBe('7 found')
})

test('the matches are the accent over the ground, as #rrggbb, the current one firmer', () => {
  const p = typed('needle')
  const options = p.searched.at(-1)![2] as Record<string, Record<string, string>>
  const lit = options.decorations
  for (const name of ['matchBackground', 'activeMatchBackground', 'activeMatchBorder',
    'matchOverviewRuler', 'activeMatchColorOverviewRuler']) {
    expect(lit[name]).toMatch(/^#[0-9a-f]{6}$/)
  }
  // The accent (10,132,255) at 25% and 60% over the dark ground (30,30,30); its edge solid.
  expect(lit.matchBackground).toBe('#193856')
  expect(lit.activeMatchBackground).toBe('#125ba5')
  expect(lit.activeMatchBorder).toBe('#0a84ff')
})

test('a full-screen program\'s screen is searched as it comes, and the shell\'s again after', () => {
  const p = typed('needle')
  p.searched.length = 0
  p.full(true)
  expect(p.searched.map(said => said.slice(0, 2))).toEqual([['clear'], ['next', 'needle']])
  p.full(false)
  expect(p.searched.at(-1)!.slice(0, 2)).toEqual(['next', 'needle'])
  // With the bar shut, a screen changed searches nothing.
  p.find('find-q').dispatch('keydown', { key: 'Escape' })
  p.searched.length = 0
  p.full(true)
  expect(p.searched).toEqual([])
})
