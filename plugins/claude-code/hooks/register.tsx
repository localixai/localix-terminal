//! The plugin's hooks: the line above the prompt and `/tabs`, the person's comments as turns,
//! and the board's workers held to their cards.

import { atom, read, update } from 'claude-code'
import type { Events, Register } from 'claude-code'

import type { BoardCard, BoardSummary, Desk } from '../types'

const EMPTY: Desk = { app: 'not running', window: '-', terminals: '0', tabs: [], comments: [] }
// Change the tag when Desk's fields change: it declines what an earlier version stored.
const desk = atom({ plugin: 'llx-desk', key: 'desk' } as const, EMPTY, { shape: 'desk-2' })
const board = atom({ plugin: 'llx-desk', key: 'board' } as const, null as BoardSummary | null, { shape: 'board-1' })

/** What every hook is handed as `$`. */
type Api = Parameters<Events['session.start']>[0]

const time = () => new Date().toTimeString().slice(0, 5)

/** The first line of a comment block: `The person commented (#1) on URL: words`. */
const words = (text: string) =>
  (text.split('\n').find(line => line.startsWith('The person commented')) ?? text.split('\n')[0] ?? '')
    .replace(/^The person commented \(#\d+\) on /, '')

/** A worker's name as the board takes it: letters, digits and `-`, a few words at most. */
const slug = (text: string) =>
  text
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .split('-')
    .slice(0, 3)
    .join('-')
    .slice(0, 24)

/** The worker a board note from the person is about: `returned c4 "Cart" (tests) to Todo: …`. */
const NOTE = /^The person on the board \S+ (?:returned|commented on|edited|moved|blocked|unblocked) c\d+ "(?:[^"\\]|\\.)*" \(([\w.:-]+)\)(?: |:|\n|$)/

// Which subagent is which worker on the board: its loop's id → the name the board shows.
const workers = new Map<string, string>()
// Where this project's board is kept, once known: `<project>/.localix/board.json`.
let boardFile: string | undefined
// `--project <the session's folder>`, once known: every call names the session's project.
let project: string[] = []

/** The board's cards, or none when it has no file yet. */
async function cards($: Api): Promise<BoardCard[] | undefined> {
  if (!boardFile) return undefined
  try {
    const kept = JSON.parse(await $.fs.read(boardFile)) as { cards?: BoardCard[] }
    return Array.isArray(kept.cards) ? kept.cards : undefined
  } catch {
    return undefined
  }
}

/** A card's id as the board makes it: the file may carry anything, and ids go into what the agent is told. */
const CARD_ID = /^c\d+$/

function summary(list: BoardCard[]): BoardSummary {
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

/** The cards `worker` (none: the main agent) left in Doing, named by id: `c3, c4`. A blocked card was put down on purpose, so it is not left half done. A title is data a file may carry, so none is put into what the agent is told. */
async function leftOver($: Api, worker: string | undefined): Promise<string> {
  const left = ((await cards($)) ?? []).filter(
    card => card.column === 'doing' && !card.blocked && CARD_ID.test(card.id) && (card.who ?? undefined) === worker,
  )
  return left.map(card => card.id).join(', ')
}

/** How the session reaches the desk, once it has started: a `llx-desk` call. */
let deskRun: ((argv: string[], timeoutMs?: number) => Promise<{ exitCode: number; stdout: string } | undefined>) | undefined
/** How long a call of the desk may take before it is given up. */
const CALL_MS = 5000
/** The call that closes the tabs as the session ends: short, so a hung app never holds the exit. */
const END_MS = 1500
/** The desk runs: telling it something then starts nothing. */
let deskUp = false
/** The session told the Event Center it waits for the person. */
let waiting = false
/** A turn of the main agent at least this long is worth telling the person it ended. */
const LONG_TURN_MS = 30_000

/** The plugin's worker, as a spawn names it. */
const WORKER = 'llx-desk:worker'
/** A worker's name on the board as its task gives it: `your name on the board: cart-button`. */
const NAMED = /your name on the board:\s*`?([A-Za-z0-9._-]+)/i
/** How long a follower that ended waits before it is started again. */
const FOLLOW_RETRY_MS = 5000
/** The session ended: the comment follower is not started again. */
let ended = false
/** The workers sent word once to finish their card. */
const nudged = new Set<string>()
/** The main agent's cards it was last asked to finish, named. */
let heldMain = ''
/** A call of the desk on the board's tab, in a shell command: `llx-desk window '{… "tab": "b1" …}'`. */
const ON_BOARD = /\bllx-desk\s+window\b[\s\S]*"tab"\s*:\s*"b\d+"/
const WHO = /"who"\s*:\s*"([^"]*)"/

/** What a worker that stops with `named` in Doing is asked to do first. */
const finish = (named: string) =>
  `Your card ${named} is still in Doing on the board. Hand it in with \`review\` and a note of what was done and how it was checked, give it back to Todo with \`release\` and where it stands, or \`block\` it with what it waits for; then end.`

export const register: Register = (on, options) => {
  on('session.start', async ($, e, next) => {
    const started = await next(e)
    // The program: the one on PATH, else LLX Desk.app's (its link in ~/.local/bin, or the app
    // where macOS keeps apps). Never installed from here: the pane only reads.
    const home = await $.env.get('HOME')
    const candidates = [
      'llx-desk',
      `${home ?? ''}/.local/bin/llx-desk`,
      `${home ?? ''}/Applications/LLX Desk.app/Contents/MacOS/llx-desk`,
      '/Applications/LLX Desk.app/Contents/MacOS/llx-desk',
    ]
    // Resolved at each call, from the one that last ran (`found`), then the rest in order: a
    // program that could not start once (not installed yet) is tried again, and one missing from
    // the PATH costs no failed start on every look. Only a program that cannot start is passed
    // over: one that started and outlasted its time is the app, slow or hung, and every other
    // candidate is the same app, so it is not asked again.
    let found = 0
    const run = async (argv: string[], timeoutMs = CALL_MS) => {
      const from = found
      const order = [...candidates.slice(from), ...candidates.slice(0, from)]
      for (const [step, program] of order.entries()) {
        const started = Date.now()
        const ran = await $.process.run([program, ...argv], { timeoutMs }).catch(() => undefined)
        if (ran) {
          found = (from + step) % candidates.length
          return ran
        }
        if (Date.now() - started >= timeoutMs) return undefined
      }
      return undefined
    }
    deskRun = run
    // The session's folder names its project: every call says it (`--project`), so the desk
    // takes the same project the session works in, wherever the hook's process runs. The
    // project is the nearest directory with `.git` around it (the desk finds it the same way);
    // its board, when it has one, is in `.localix/board.json` there.
    const cwd = await $.session.cwd()
    project = ['--project', cwd]
    const top = await $.process.run(['git', '-C', cwd, 'rev-parse', '--show-toplevel'], { timeoutMs: 3000 }).catch(() => undefined)
    const root = top && top.exitCode === 0 && top.stdout.trim() ? top.stdout.trim() : cwd
    boardFile = `${root}/.localix/board.json`

    // The status line an earlier version pinned: its notice read like a warning.
    $.ui.status(undefined)
    await $.command.register({ name: 'tabs', description: "This project's tabs and the person's latest comments" })

    // What the app holds, every few seconds. `status` never starts the app, and the tabs are
    // listed only while one runs, so the pane never opens a window by itself. The board is read
    // from its file, so it shows with the app not running.
    // Each look writes only what changed since the last: a quiet desk redraws nothing.
    const wrote = new Map<string, string>()
    const changed = (key: string, value: unknown) => {
      const text = JSON.stringify(value)
      if (wrote.get(key) === text) return false
      wrote.set(key, text)
      return true
    }
    const look = async () => {
      const list = await cards($)
      const brief = list && list.length ? summary(list) : null
      if (changed('board', brief)) await update($, board, () => brief)
      const status = await run(['status'])
      if (!status || status.exitCode !== 0) {
        deskUp = false
        waiting = false
        const gone = { app: 'not running', window: '-', terminals: '0', tabs: [] }
        if (changed('desk', gone)) await update($, desk, d => ({ ...d, ...gone }))
        return
      }
      const lines = status.stdout.split('\n')
      const field = (name: string) => lines.find(l => l.startsWith(`${name}: `))?.slice(name.length + 2) ?? '-'
      deskUp = true
      // A look for the pane: `--peek` leaves the person's notes for Claude's own next call.
      const listed = await run(['window', ...project, '--peek', '{"action":"list"}'])
      const tabs = (listed?.stdout ?? '').split('\n').filter(l => /^[wtb]\d+ /.test(l))
      const held = { app: lines[0] ?? 'running', window: field('window'), terminals: field('terminals'), tabs }
      if (changed('desk', held)) await update($, desk, d => ({ ...d, ...held }))
    }
    // One look at a time: a slow call does not pile ticks up behind it.
    let looking = false
    const refresh = async () => {
      if (looking) return
      looking = true
      try {
        await look()
      } finally {
        looking = false
      }
    }
    $.clock.every(3000, () => void refresh().catch(() => undefined))

    // A note the person left for a subagent's card goes to that subagent while it runs; the main
    // agent hears it too, and that it went there.
    const tell = async (text: string) => {
      const worker = text.match(NOTE)?.[1]
      const agentId = worker ? [...workers].find(([, name]) => name === worker)?.[0] : undefined
      if (agentId) {
        const agents = await $.agent.list().catch(() => [])
        const live = agents.find(agent => agent.id === agentId && agent.status !== 'completed' && agent.status !== 'failed')
        if (live) {
          const sent = await $.session.send({ to: { agentId }, text }).catch(() => undefined)
          if (sent?.isDelivered) {
            void $.prompt.submit({ text: `${text}\n(Sent to the subagent ${worker} too, which has the card: let it work on it.)` })
            return
          }
        }
      }
      void $.prompt.submit({ text })
    }

    // The person's comments and what they do on the board, as they come: kept for the pane, and
    // each one a turn of its own, so Claude wakes without anyone typing. Each round looks first,
    // which settles which program; a follower that cannot start or ends is started again a few
    // seconds later, until the session ends.
    ended = false
    void (async () => {
      while (!ended) {
        try {
          await refresh()
          // Each note is one JSON string on a line of its own (`--json`): what the pipe hands over
          // comes in pieces of any size, so the lines are put together again before one is read, and
          // a note's own lines (its picture's path last) never come as a turn of their own.
          const follow = $.process.spawn({ argv: [candidates[found] ?? 'llx-desk', 'window', ...project, '--said', '--follow', '--json'] })
          let held = ''
          for await (const piece of follow) {
            if (piece.stream !== 'stdout') continue
            held += piece.text
            const lines = held.split('\n')
            held = lines.pop() ?? ''
            for (const line of lines) {
              let text: string
              try {
                text = String(JSON.parse(line)).trim()
              } catch {
                continue
              }
              if (!text) continue
              await update($, desk, d => ({ ...d, comments: [...d.comments, { at: time(), text: words(text) }].slice(-20) }))
              $.ui.toast(`llx-desk: ${words(text).slice(0, 80)}`)
              await tell(text)
            }
          }
        } catch {
          // The follower could not start or broke: the next round starts it again.
        }
        // A wait that is refused means the module is gone: the loop ends with it.
        if (!(await $.clock.sleep(FOLLOW_RETRY_MS).then(() => true, () => false))) break
      }
    })()

    return started
  })

  // A turn ended. A worker whose run ends with its card still in Doing left it half done: it is
  // sent word once to hand it in, release it or block it (a message resumes it); if it ends so
  // again, the main agent hears which card, so it is not forgotten. The main agent is held to its
  // own cards the same way when the person turned that on. A long turn of the main agent leaves
  // one line in the Event Center that the session waits for the person, while the desk runs; the
  // next turn takes it back, so it is never more than one, never stale.
  on('turn.complete', async ($, e, next) => {
    const done = await next(e)
    if (e.isAborted || 'refusal' in e) return done
    if (e.agentId) {
      const worker = workers.get(e.agentId)
      const named = worker ? await leftOver($, worker) : ''
      if (!worker || !named) return done
      if (!nudged.has(e.agentId)) {
        nudged.add(e.agentId)
        const sent = await $.session.send({ to: { agentId: e.agentId }, text: finish(named) }).catch(() => undefined)
        if (sent?.isDelivered) return done
      }
      void $.prompt.submit({
        text: `The subagent ${worker} finished, and its card ${named} is still in Doing on the board. Hand it in (\`review\`), give it back to Todo (\`release\`), or \`block\` it with what it waits for.`,
      })
      return done
    }
    if (options.finish_cards === true) {
      const named = await leftOver($, undefined)
      // Asked once for the same cards: a second turn that leaves them is the agent's answer.
      if (named && named !== heldMain) {
        heldMain = named
        void $.prompt.submit({ text: finish(named) })
        return done
      }
      if (!named) heldMain = ''
    }
    if (e.durationMs < LONG_TURN_MS || !deskRun || !deskUp) return done
    const said = (e.answer.split('\n').find(line => line.trim()) ?? '').trim().slice(0, 140)
    const ask = said ? `Claude finished: ${said}` : 'Claude finished and waits for you'
    waiting = true
    void deskRun(['window', ...project, JSON.stringify({ action: 'interact', ask })])
    return done
  })

  on('turn.start', async ($, e, next) => {
    if (!e.agentId && waiting && deskUp && deskRun) {
      waiting = false
      void deskRun(['window', ...project, JSON.stringify({ action: 'interact', ask: '' })])
    }
    return next(e)
  })

  // A worker is told its name on the board in its task, and passes it as `who` itself. The main
  // agent gives it ("your name on the board: cart-button"): kept for the subagent's id as it
  // starts. A task that gives none gets one from its description, added to its words.
  on('agent.spawn', async ($, e, next) => {
    if (e.subagentType !== WORKER) return next(e)
    const raw = e.prompt.match(NAMED)?.[1]
    const given = slug(raw ?? '')
    const taken = [...workers.values()]
    let name = given && !taken.includes(given) && given !== 'agent' && given !== 'person' ? given : ''
    if (!name) {
      name = slug(e.description) || 'worker'
      if (taken.includes(name) || name === 'agent' || name === 'person') name = `${name}-${taken.length + 1}`
    }
    // The name the task gives is kept as written when it is the one taken; else its line says the
    // chosen one, so the prompt holds one name only.
    let prompt = e.prompt
    if (raw !== name) {
      const tail = raw?.match(/\.+$/)?.[0] ?? ''
      prompt = raw
        ? prompt.replace(NAMED, said => `${said.slice(0, said.length - raw.length)}${name}${tail}`)
        : `${prompt.trim()}\n\nYour name on the board: ${name}.`
    }
    const started = await next(prompt === e.prompt ? e : { ...e, prompt })
    if (started.agentId) workers.set(started.agentId, name)
    return started
  }).catch(($, e, next) => next(e))

  // The board knows who works on a card from who calls: a worker's call on the board carries its
  // name as `who`, so one worker never acts on another's cards. A worker passes it itself, as its
  // task says; a call on the board without it, or with another's, is refused here with the name to
  // pass, never rewritten (a command changed under it is one auto mode reviews again). The main
  // agent passes none, and may act on any card.
  on('tool.call', { tool: 'Bash' }, async ($, e, next) => {
    const name = e.agentId ? workers.get(e.agentId) : undefined
    if (!name || !ON_BOARD.test(e.command)) return next(e)
    const said = e.command.match(WHO)?.[1]
    if (said === name) return next(e)
    return { deny: `llx-desk: you are ${name} on the board: pass "who": "${name}" in every call on it, then run it again.` }
  }).catch(($, e, next) => next(e))

  // The session ends: the tabs the project opened close (never a server started for it: only
  // the app's program is run, and `--end` starts nothing).
  on('session.end', async ($, e, next) => {
    ended = true
    if (deskRun && deskUp) await deskRun(['window', ...project, '--end'], END_MS).catch(() => undefined)
    return next(e)
  })

  // A short answer in the chat, not a pane: three facts do not need a column of their own.
  on('command.run', { command: 'tabs' }, async $ => {
    const d = await read($, desk)
    const b = await read($, board)
    const ideas = b && b.backlog ? ` · ${b.backlog} in backlog` : ''
    const work = b ? `\nBoard: ${b.todo} todo · ${b.doing} doing · ${b.review} in review · ${b.done} done${ideas}` : ''
    if (d.app === 'not running') return { text: `llx-desk is not running.${work}` }
    const tabs = d.tabs.length ? d.tabs.map(tab => `  ${tab}`).join('\n') : '  none open'
    const last = d.comments.slice(-3).map(c => `  ${c.at} ${c.text}`).join('\n')
    const said = last ? `\nLast comments:\n${last}` : ''
    return { text: `${d.app} · window ${d.window}\nTabs of this project:\n${tabs}${work}${said}` }
  })

  // A quiet line above the prompt while the app runs or the board has cards: the tabs, who
  // works on what, what waits for the person, and the last comment.
  on('ui.render', { component: 'AbovePrompt' }, async ($, e, next) => {
    const d = await read($, desk)
    const b = await read($, board)
    const running = d.app !== 'not running'
    if (e.props.hasSurvey || (!running && !b)) return next(e)
    const { Box, Text } = $.ui.resolve(e)
    const tabs = `${d.tabs.length} tab${d.tabs.length === 1 ? '' : 's'}`
    const last = d.comments[d.comments.length - 1]
    const working = b && b.working.length ? ` · Doing: ${b.working.slice(0, 3).join(', ')}${b.working.length > 3 ? '…' : ''}` : ''
    return (
      <Box>
        <Text color="cyan">◐ desk</Text>
        <Text dimColor>
          {running ? ` · ${tabs}` : ''}
          {running && d.terminals !== '0' ? ` · ${d.terminals} terminal${d.terminals === '1' ? '' : 's'}` : ''}
          {working}
        </Text>
        {b && b.review ? <Text color="yellow"> · {b.review} to review</Text> : null}
        {b && b.blocked ? <Text color="red"> · {b.blocked} blocked</Text> : null}
        <Text dimColor>
          {last ? ` · last comment ${last.at}` : ''} · /tabs
        </Text>
      </Box>
    )
  })
}
