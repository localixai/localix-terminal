//! The board as the plugin reads it: its cards from the project's `.localix/board.json`, the line
//! above the prompt's summary of them, the names its workers are known by, and the words a worker
//! that leaves its card half done is sent.

import type { Events } from 'claude-code'

import type { BoardCard, BoardSummary } from '../types'

/** What every hook is handed as `$`. */
export type Api = Parameters<Events['session.start']>[0]

/** A worker's name as the board takes it: letters, digits and `-`, a few words at most. */
export const slug = (text: string) =>
  text
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .split('-')
    .slice(0, 3)
    .join('-')
    .slice(0, 24)

/** The worker a board note from the person is about: `returned c4 "Cart" (tests) to Todo: …`. */
export const NOTE = /^The person on the board \S+ (?:returned|commented on|edited|moved|blocked|unblocked) c\d+ "(?:[^"\\]|\\.)*" \(([\w.:-]+)\)(?: |:|\n|$)/

/** The board's cards in `file`, or none when it has no file yet. */
export async function cards($: Api, file: string | undefined): Promise<BoardCard[] | undefined> {
  if (!file) return undefined
  try {
    const kept = JSON.parse(await $.fs.read(file)) as { cards?: BoardCard[] }
    return Array.isArray(kept.cards) ? kept.cards : undefined
  } catch {
    return undefined
  }
}

/** A card's id as the board makes it: the file may carry anything, and ids go into what the agent is told. */
const CARD_ID = /^c\d+$/

export function summary(list: BoardCard[]): BoardSummary {
  const count = (column: BoardCard['column']) => list.filter(card => card.column === column).length
  return {
    backlog: count('backlog'),
    todo: count('todo'),
    doing: count('doing'),
    review: count('review'),
    done: count('done'),
    blocked: list.filter(card => card.blocked && card.column !== 'done').length,
    working: list.filter(card => card.column === 'doing' && CARD_ID.test(card.id)).map(card => (card.who ? `${card.id} ${card.who}` : card.id)),
  }
}

/** The cards `worker` (none: the main agent) left in Doing on the board in `file`, named by id: `c3, c4`. A blocked card was put down on purpose, so it is not left half done. A title is data a file may carry, so none is put into what the agent is told. */
export async function leftOver($: Api, file: string | undefined, worker: string | undefined): Promise<string> {
  const left = ((await cards($, file)) ?? []).filter(
    card => card.column === 'doing' && !card.blocked && CARD_ID.test(card.id) && (card.who ?? undefined) === worker,
  )
  return left.map(card => card.id).join(', ')
}

/** The plugin's worker, as a spawn names it. */
export const WORKER = 'llxterm:worker'
/** A worker's name on the board as its task gives it: `your name on the board: cart-button`. */
export const NAMED = /your name on the board:\s*`?([A-Za-z0-9._-]+)/i
/** A call of the desk on the board's tab, in a shell command: `llxterm window '{… "tab": "b1" …}'`. */
export const ON_BOARD = /\bllxterm\s+window\b[\s\S]*"tab"\s*:\s*"b\d+"/
export const WHO = /"who"\s*:\s*"([^"]*)"/

/** What a worker that stops with `named` in Doing is asked to do first. */
export const finish = (named: string) =>
  `Your card ${named} is still in Doing on the board. Hand it in with \`review\` and a note of what was done and how it was checked, give it back to Todo with \`release\` and where it stands, or \`block\` it with what it waits for; then end.`
