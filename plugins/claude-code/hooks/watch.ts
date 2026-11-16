//! The session's one watcher of the app (`llxterm window --watch`): it waits for the app when
//! none runs and ends with the session, so nothing here looks again and again. Each line is one
//! JSON object; what the pipe hands over comes in pieces of any size, so the lines are put
//! together again before one is read. It ends by itself only when it cannot run: started again
//! later, each wait twice the last.

import type { BoardCard } from '../types'
import type { Api } from './board'

/** A line of `llxterm window --watch`: the app and this project's tabs, the board, or a comment. */
export type Watched =
  | { app: string; window?: string; terminals?: string; tabs?: string[] }
  | { board: { cards?: BoardCard[] } | null }
  | { said: string }

/** How long a watcher that ended waits before it is started again, the first time; each time
 * after, twice as long, up to `WATCH_MOST_MS`. */
const WATCH_RETRY_MS = 5000
const WATCH_MOST_MS = 60_000

/** `program`'s watcher of `project`, each line handed to `heard`, until `over` says the session
 * ended or the host refuses a wait (the module is gone). */
export async function follow(
  $: Api,
  program: string,
  project: string[],
  heard: (line: Watched) => Promise<void>,
  over: () => boolean,
) {
  let pause = WATCH_RETRY_MS
  while (!over()) {
    try {
      const watch = $.process.spawn({ argv: [program, 'window', ...project, '--watch'] })
      let held = ''
      for await (const piece of watch) {
        if (piece.stream !== 'stdout') continue
        held += piece.text
        const lines = held.split('\n')
        held = lines.pop() ?? ''
        for (const line of lines) {
          let said: Watched
          try {
            said = JSON.parse(line) as Watched
          } catch {
            continue
          }
          pause = WATCH_RETRY_MS
          await heard(said)
        }
      }
    } catch {
      // The watcher could not start or broke: it is started again after the pause.
    }
    if (over()) break
    if (!(await $.clock.sleep(pause).then(() => true, () => false))) break
    pause = Math.min(pause * 2, WATCH_MOST_MS)
  }
}
