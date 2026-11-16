//! The plugin's hooks: the line above the prompt and `/tabs`, the person's comments as turns,
//! the board's workers held to their cards, and, in a terminal of the desk, its window as the
//! browser the person means (a row the session starts with) and the session's id, so the
//! terminal resumes this session after the app opens again. The board's reading is `board.ts`,
//! the watcher of the app `watch.ts`.

import { atom, read, update } from 'claude-code'
import type { Register } from 'claude-code'

import type { BoardSummary, Desk } from '../types'
import { type Api, cards, finish, leftOver, NAMED, NOTE, ON_BOARD, slug, summary, WHO, WORKER } from './board'
import { follow, type Watched } from './watch'

/** What a session in a terminal of the desk is told: the window the person watches is their
 * browser, ahead of any other browser's tools the session has. */
const BROWSER = [
  'This session runs in a terminal of Localix Terminal, the window the person is watching.',
  'When they ask to open, show or look at a web page or a site ("open google"), open it as a tab of',
  'that window with `llxterm window` (the llxterm window skill), not with another browser\'s tools',
  '(Claude in Chrome, a built-in browser pane, `open <url>`). Use another browser only when the',
  'person names it. What you open lands beside this terminal, in their sight: no `show` is needed.',
].join(' ')

const EMPTY: Desk = { app: 'not running', window: '-', terminals: '0', tabs: [], comments: [] }
// Change the tag when Desk's fields change: it declines what an earlier version stored.
const desk = atom({ plugin: 'llxterm', key: 'desk' } as const, EMPTY, { shape: 'desk-2' })
const board = atom({ plugin: 'llxterm', key: 'board' } as const, null as BoardSummary | null, { shape: 'board-1' })

const time = () => new Date().toTimeString().slice(0, 5)

/** The first line of a comment block: `The person commented (#1) on URL: words`. */
const words = (text: string) =>
  (text.split('\n').find(line => line.startsWith('The person commented')) ?? text.split('\n')[0] ?? '')
    .replace(/^The person commented \(#\d+\) on /, '')

// Which subagent is which worker on the board: its loop's id → the name the board shows.
const workers = new Map<string, string>()
// Where this project's board is kept, once known: `<project>/.localix/board.json`.
let boardFile: string | undefined
// `--project <the session's folder>`, once known: every call names the session's project.
let project: string[] = []

/** How the session reaches the desk, once it has started: a `llxterm` call. */
let deskRun: ((argv: string[], timeoutMs?: number) => Promise<{ exitCode: number; stdout: string } | undefined>) | undefined
/** How long a call of the desk may take before it is given up. */
const CALL_MS = 5000
/** The call that closes the tabs as the session ends: short, so a hung app never holds the exit. */
const END_MS = 1500
/** The desk runs, as its watcher last said: telling it something then starts nothing. */
let deskUp = false
/** The session told the Event Center it waits for the person. */
let waiting = false
/** The terminal reads what Claude Code says of its state (OSC 7501, `LLXTERM_PROGRAM_STATUS`):
 * that it waits for the person is the terminal's line there, so the plugin says only what the
 * report carries no words for (a long turn's last line, an error). */
let statusRead = false
/** A turn of the main agent at least this long is worth telling the person it ended. */
const LONG_TURN_MS = 30_000

/** The longest line of the agent's question that the person is shown. */
const SHOWN_MOST = 120

/** The session ended: the watcher is not started again. */
let ended = false
/** The session's id its terminal was last told, and whether it runs in a terminal of the app. */
let toldSession: string | undefined
let inTerminal = false

/** The terminal is told the session's id when it is not the one it was told last (the session
 * started, or `/clear` and `/resume` gave it another): once each, not each turn. Only from a
 * terminal of the app, which runs: the call never starts it. */
async function tellSession($: Api) {
  if (!inTerminal || !deskRun) return
  const id = await $.session.id().catch(() => undefined)
  if (typeof id !== 'string' || !id || id === toldSession) return
  toldSession = id
  void deskRun(['window', ...project, JSON.stringify({ action: 'interact', session: id })])
}

/** The workers sent word once to finish their card. */
const nudged = new Set<string>()
/** The main agent's cards it was last asked to finish, named. */
let heldMain = ''

export const register: Register = (on, options) => {
  // A session begun waits for nobody yet, and its terminal knows none of its ids.
  waiting = false
  toldSession = undefined
  on('session.start', async ($, e, next) => {
    const started = await next(e)
    // In a terminal of the desk the person watches its window: a page they ask for opens there,
    // so they see it beside the session, rather than in a browser the session also has the
    // tools of. Said as a row of the conversation, ahead of the first prompt: a section of the
    // system prompt is passed over where an organization seats its policy.
    if ((await $.env.get('TERM_PROGRAM')) === 'llxterm') {
      await $.session.append({ message: { type: 'user', content: [{ type: 'text', text: BROWSER }] } }).catch(() => undefined)
    }
    // The program: the app's own, which its terminals put first on the PATH (`LLXTERM_BIN` names
    // its folder): the plugin is loaded only in them. Never installed from here.
    // Inside tmux or screen, started in a terminal of the app, the variable is the app's but the
    // terminal Claude Code reports to is theirs, which tells the app nothing.
    const muxed = Boolean((await $.env.get('TMUX')) || (await $.env.get('STY')))
    statusRead = (await $.env.get('LLXTERM_PROGRAM_STATUS')) === '1' && !muxed
    const bin = await $.env.get('LLXTERM_BIN')
    const program = bin ? `${bin}/llxterm` : 'llxterm'
    const run = (argv: string[], timeoutMs = CALL_MS) =>
      $.process.run([program, ...argv], { timeoutMs }).catch(() => undefined)
    deskRun = run
    // The session's folder names its project: every call says it (`--project`), so the desk
    // takes the same project the session works in, wherever the hook's process runs. The
    // project is the nearest directory with `.git` around it (the desk finds it the same way);
    // its board, when it has one, is in `.localix/board.json` there.
    const cwd = await $.session.cwd()
    project = ['--project', cwd]
    inTerminal = Boolean(await $.env.get('LLXTERM_PANE'))
    await tellSession($)
    const top = await $.process.run(['git', '-C', cwd, 'rev-parse', '--show-toplevel'], { timeoutMs: 3000 }).catch(() => undefined)
    const root = top && top.exitCode === 0 && top.stdout.trim() ? top.stdout.trim() : cwd
    boardFile = `${root}/.localix/board.json`

    await $.command.register({ name: 'tabs', description: "This project's tabs and the person's latest comments" })

    // The board as its file holds it, once: it shows with the app not running. The app tells
    // each change after, through the watcher.
    const showBoard = async (list: BoardCard[] | undefined) => {
      const brief = list && list.length ? summary(list) : null
      await update($, board, () => brief)
    }
    await showBoard(await cards($, boardFile))

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

    // What the window, the board and the person say, as they come: kept for the pane, and each
    // comment a turn of its own, so Claude wakes without anyone typing.
    const heard = async (line: Watched) => {
      if ('app' in line) {
        deskUp = line.app !== 'not running'
        if (!deskUp) waiting = false
        const held = deskUp
          ? { app: line.app, window: line.window ?? '-', terminals: line.terminals ?? '0', tabs: line.tabs ?? [] }
          : { app: 'not running', window: '-', terminals: '0', tabs: [] }
        await update($, desk, d => ({ ...d, ...held }))
      } else if ('board' in line) {
        await showBoard(Array.isArray(line.board?.cards) ? line.board.cards : undefined)
      } else if (typeof line.said === 'string' && line.said.trim()) {
        const text = line.said.trim()
        await update($, desk, d => ({ ...d, comments: [...d.comments, { at: time(), text: words(text) }].slice(-20) }))
        $.ui.toast(`Localix Terminal: ${words(text).slice(0, 80)}`)
        await tell(text)
      }
    }

    // One watcher for the session (watch.ts), until the session ends.
    ended = false
    void follow($, program, project, heard, () => ended)

    return started
  })

  // A turn ended. A worker whose run ends with its card still in Doing left it half done: it is
  // sent word once to hand it in, release it or block it (a message resumes it); if it ends so
  // again, the main agent hears which card, so it is not forgotten. The main agent is held to its
  // own cards the same way when the person turned that on. A long turn of the main agent leaves
  // one line in the Event Center that the session waits for the person, while the desk runs; the
  // next turn takes it back, so it is never more than one, never stale.
  /** The session waits for the person with `words` in the Event Center, while the desk runs. */
  const waits = (words: string) => {
    if (!deskRun || !deskUp) return
    waiting = true
    void deskRun(['window', ...project, JSON.stringify({ action: 'interact', ask: words })])
  }
  /** It goes on: its line, if one is up, is taken back. */
  const goesOn = () => {
    if (!waiting || !deskRun || !deskUp) return
    waiting = false
    void deskRun(['window', ...project, JSON.stringify({ action: 'interact', ask: '' })])
  }

  on('turn.complete', async ($, e, next) => {
    const done = await next(e)
    if (e.isAborted || 'refusal' in e) return done
    if (e.agentId) {
      const worker = workers.get(e.agentId)
      const named = worker ? await leftOver($, boardFile, worker) : ''
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
      const named = await leftOver($, boardFile, undefined)
      // Asked once for the same cards: a second turn that leaves them is the agent's answer.
      if (named && named !== heldMain) {
        heldMain = named
        void $.prompt.submit({ text: finish(named) })
        return done
      }
      if (!named) heldMain = ''
    }
    // An error ended it: said however short the turn, as the session stopped without an answer.
    if (e.reason === 'error') {
      waits('Claude stopped on an error')
      return done
    }
    // A short turn says nothing, and what waited within it goes.
    if (e.durationMs < LONG_TURN_MS) {
      goesOn()
      return done
    }
    const said = (e.answer.split('\n').find(line => line.trim()) ?? '').trim().slice(0, 140)
    waits(said ? `Claude finished: ${said}` : 'Claude finished and waits for you')
    return done
  })

  on('turn.start', async ($, e, next) => {
    if (!e.agentId) {
      goesOn()
      await tellSession($)
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
    return { deny: `llxterm: you are ${name} on the board: pass "who": "${name}" in every call on it, then run it again.` }
  }).catch(($, e, next) => next(e))

  // A question the agent asks the person waits for them in the Event Center till it is answered:
  // one line on the terminal the session runs in, gone as soon as the session goes on. Where the
  // terminal reads Claude Code's own report that it waits, that line is the terminal's.
  on('tool.call', { tool: 'AskUserQuestion' }, async ($, e, next) => {
    if (statusRead) return next(e)
    const first = e.questions[0]?.question.replace(/\s+/g, ' ').trim().slice(0, SHOWN_MOST)
    waits(first ? `Claude asks: ${first}` : 'Claude asks you a question')
    try {
      return await next(e)
    } finally {
      goesOn()
    }
  }).catch(($, e, next) => next(e))

  // The session ends: the tabs the project opened close (never a server started for it: only
  // the app's program is run, and `--end` starts nothing).
  on('session.end', async ($, e, next) => {
    ended = true
    // Its line in the Event Center is its terminal's, which stays open: it goes with the session.
    goesOn()
    if (deskRun && deskUp) await deskRun(['window', ...project, '--end'], END_MS).catch(() => undefined)
    return next(e)
  })

  // A short answer in the chat, not a pane: three facts do not need a column of their own.
  on('command.run', { command: 'tabs' }, async $ => {
    const d = await read($, desk)
    const b = await read($, board)
    const ideas = b && b.backlog ? ` · ${b.backlog} in backlog` : ''
    const work = b ? `\nBoard: ${b.todo} todo · ${b.doing} doing · ${b.review} in review · ${b.done} done${ideas}` : ''
    if (d.app === 'not running') return { text: `Localix Terminal is not running.${work}` }
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
    // One line of text, its parts spans of it: a narrow pane cuts its end off, where a row of
    // boxes would be squeezed into columns that wrap into each other.
    return (
      <Box>
        <Text wrap="truncate-end">
          <Text color="cyan">◐ Localix Terminal</Text>
          <Text dimColor>
            {running ? ` · ${tabs}` : ''}
            {working}
          </Text>
          {b && b.review ? <Text color="yellow"> · {b.review} to review</Text> : null}
          {b && b.blocked ? <Text color="red"> · {b.blocked} blocked</Text> : null}
          <Text dimColor>
            {last ? ` · last comment ${last.at}` : ''} · /tabs
          </Text>
        </Text>
      </Box>
    )
  })
}
