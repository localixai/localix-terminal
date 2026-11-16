//! Hook regressions against a small host mock; no installed Claude session or desk is needed.
//! Run by `bun test`: the file is a `.spec.ts` so that `claude plugin test`, which takes every
//! `.test.ts` for its own kit, leaves it alone.

import { expect, mock, test } from 'bun:test'

mock.module('claude-code', () => ({
  atom: (_key: unknown, value: unknown) => ({ value }),
  read: async (_api: unknown, atom: { value: unknown }) => atom.value,
  update: async (_api: unknown, atom: { value: unknown }, change: (value: unknown) => unknown) => {
    atom.value = change(atom.value)
  },
}))

// Bun compiles the hooks' JSX to React's runtime, which only the host provides: the line is never
// drawn here, so an element is nothing.
const runtime = () => ({ jsx: () => null, jsxs: () => null, jsxDEV: () => null, Fragment: 'fragment' })
mock.module('react/jsx-dev-runtime', runtime)
mock.module('react/jsx-runtime', runtime)

const { register } = await import('./register')

type Ran = { exitCode: number; stdout: string }
/** How the mock host answers one `$.process.run`; a rejection is a program that failed. */
type Runner = (argv: string[], timeoutMs: number) => Promise<Ran>

/** What the desk answers a call. */
const answer = (_argv: string[]): Ran => ({ exitCode: 0, stdout: 'No tabs open.' })

/** What the watcher prints, by default: the app runs. */
const RUNNING = [{ app: 'llxterm 0.2.0 (pid 7)', window: 'open', terminals: '1', tabs: ['t1 zsh'] }]

/** A session on a mock host whose programs answer as `runner` says and whose watcher prints
 * `watched` (one JSON line each, handed over in pieces) before it ends; the host grants the
 * watcher's pauses `sleeps` times, then refuses. Its hooks, once started. */
async function session(
  runner: Runner,
  vars: Record<string, string> = {},
  watched: unknown[] = RUNNING,
  sleeps = 0,
  sessionId: () => string = () => 'sess-1',
) {
  const calls: { argv: string[]; timeoutMs: number }[] = []
  const spawned: string[][] = []
  const timers: number[] = []
  const paused: number[] = []
  const submitted: string[] = []
  const appended: { message: { type: string; content: { type: string; text: string }[] } }[] = []
  type Handler = (api: unknown, event: unknown, next: (event: unknown) => Promise<unknown>) => Promise<unknown>
  const hooks = new Map<string, Handler>()
  const on = (event: string, filterOrHandler: unknown, handler?: Handler) => {
    hooks.set(event, handler ?? filterOrHandler as Handler)
    return { catch: () => undefined }
  }
  let followed!: () => void
  const following = new Promise<void>(resolve => { followed = resolve })
  const api = {
    env: { get: async (name: string) => vars[name] ?? (name === 'HOME' ? '/home/test' : undefined) },
    session: {
      id: async () => sessionId(),
      cwd: async () => '/work/project/nested',
      append: async (row: (typeof appended)[number]) => { appended.push(row) },
    },
    fs: { read: async () => '{"cards":[]}' },
    ui: { status: () => undefined, toast: () => undefined },
    command: { register: async () => undefined },
    prompt: { submit: async ({ text }: { text: string }) => { submitted.push(text) } },
    agent: { list: async () => [] },
    clock: {
      every: (ms: number) => { timers.push(ms) },
      sleep: async (ms: number) => {
        paused.push(ms)
        if (paused.length > sleeps) {
          followed()
          throw new Error('the mock host ended')
        }
      },
    },
    process: {
      run: async (argv: string[], init: { timeoutMs: number }) => {
        if (argv[0] === 'git') return { exitCode: 0, stdout: '/work/project\n' }
        calls.push({ argv, timeoutMs: init.timeoutMs })
        return runner(argv, init.timeoutMs)
      },
      spawn: ({ argv }: { argv: string[] }) => ({
        async *[Symbol.asyncIterator]() {
          spawned.push(argv)
          const text = watched.map(line => `${JSON.stringify(line)}\n`).join('')
          // In pieces that cut lines apart, as a pipe hands them over.
          for (let at = 0; at < text.length; at += 7) yield { stream: 'stdout', text: text.slice(at, at + 7) }
        },
      }),
    },
  }
  register(on as never, {} as never)
  const next = async (event: unknown) => event
  await hooks.get('session.start')!(api, {}, next)
  await following
  const hook = (name: string, event: unknown) => hooks.get(name)!(api, event, next)
  return { calls, spawned, timers, paused, submitted, appended, hook }
}

/** A program that started and hangs: the host kills it once its time is out, and rejects. */
const hung = (timeoutMs: number) =>
  new Promise<Ran>((_, reject) => setTimeout(() => reject(new Error('timed out')), timeoutMs))

test('turn notifications stay in the session project, including taking the ask back', async () => {
  const { calls, hook } = await session(async argv => answer(argv))
  const before = calls.length
  const completed = { durationMs: 31_000, answer: 'The cart is fixed' }
  await hook('turn.complete', completed)
  await hook('turn.start', {})
  const asks = calls.slice(before).map(call => call.argv)
  expect(asks).toHaveLength(2)
  for (const argv of asks) {
    expect(argv.slice(1, 4)).toEqual(['window', '--project', '/work/project/nested'])
  }
  expect(JSON.parse(asks[0]!.at(-1)!)).toEqual({ action: 'interact', ask: 'Claude finished: The cart is fixed' })
  expect(JSON.parse(asks[1]!.at(-1)!)).toEqual({ action: 'interact', ask: '' })

  const after = calls.length
  await hook('turn.complete', { ...completed, durationMs: 100 })
  await hook('turn.complete', { ...completed, isAborted: true })
  expect(calls).toHaveLength(after)
  await hook('session.end', {})
})

/** The asks a session made of the desk since `from`, as the words of each. */
const asked = (calls: { argv: string[] }[], from: number) =>
  calls
    .slice(from)
    .map(call => call.argv.at(-1)!)
    .filter(last => last.startsWith('{'))
    .map(last => (JSON.parse(last) as { ask?: string }).ask)

test('a question the agent asks waits for the person until they answer it', async () => {
  const { calls, hook } = await session(async argv => answer(argv))
  const before = calls.length
  await hook('tool.call', { tool: 'AskUserQuestion', questions: [{ question: 'Which database should the cart use?' }] })
  expect(asked(calls, before)).toEqual(['Claude asks: Which database should the cart use?', ''])
})

test('in a terminal that reads program status, the question is the terminal\'s line, not the plugin\'s', async () => {
  // Claude Code says it waits (OSC 7501) to a terminal that reads it: one line, never two.
  const { calls, hook } = await session(async argv => answer(argv), { LLXTERM_PROGRAM_STATUS: '1' })
  const before = calls.length
  await hook('tool.call', { tool: 'AskUserQuestion', questions: [{ question: 'Which database should the cart use?' }] })
  expect(asked(calls, before)).toEqual([])
  // A long turn's last line is the plugin's still: the program's report of it carries no words.
  await hook('turn.complete', { durationMs: 90_000, answer: 'The cart is fixed.', reason: 'end_turn' })
  expect(asked(calls, before)).toEqual(['Claude finished: The cart is fixed.'])
})

test('inside tmux or screen the terminal never hears the report, so the question is the plugin\'s line', async () => {
  // The variable came from the terminal tmux or screen was started in; their own terminal
  // answers Claude Code's query, not the app's.
  for (const inside of [{ TMUX: '/tmp/tmux-501/default,7,0' }, { STY: '42.ttys001.host' }]) {
    const { calls, hook } = await session(async argv => answer(argv), { LLXTERM_PROGRAM_STATUS: '1', ...inside })
    const before = calls.length
    await hook('tool.call', { tool: 'AskUserQuestion', questions: [{ question: 'Which database?' }] })
    expect(asked(calls, before)).toEqual(['Claude asks: Which database?', ''])
  }
})

test('a turn that an error ended is said however short, and one the person stopped is not', async () => {
  const { calls, hook } = await session(async argv => answer(argv))
  const before = calls.length
  await hook('turn.complete', { durationMs: 900, answer: '', reason: 'error' })
  await hook('turn.complete', { durationMs: 90_000, answer: 'half', reason: 'aborted', isAborted: true })
  // The session ends while its line is up: the line goes with it.
  await hook('session.end', {})
  expect(asked(calls, before)).toEqual(['Claude stopped on an error', ''])
})

test('the app is called by its own program, and no timer runs nor status is asked during a session', async () => {
  const { calls, spawned, timers, hook } = await session(async argv => answer(argv), { LLXTERM_BIN: '/Apps/LT.app/bin' })
  expect(spawned).toEqual([['/Apps/LT.app/bin/llxterm', 'window', '--project', '/work/project/nested', '--watch']])
  await hook('turn.complete', { durationMs: 31_000, answer: 'Done' })
  expect(calls.length).toBeGreaterThan(0)
  expect(calls.every(call => call.argv[0] === '/Apps/LT.app/bin/llxterm')).toBe(true)
  expect(calls.some(call => call.argv.includes('status'))).toBe(false)
  expect(timers).toEqual([])
})

test("the watcher's lines keep the line above the prompt and a comment wakes Claude", async () => {
  const watched = [
    ...RUNNING,
    { board: { cards: [{ id: 'c1', title: 'Cart', column: 'doing', who: 'tests' }, { id: 'c2', title: 'Header', column: 'review' }] } },
    { said: 'The person commented (#1) on https://shop.test: make it red' },
  ]
  const { submitted, hook } = await session(async argv => answer(argv), {}, watched)
  const tabs = (await hook('command.run', {})) as { text: string }
  expect(tabs.text).toContain('llxterm 0.2.0 (pid 7) · window open')
  expect(tabs.text).toContain('  t1 zsh')
  expect(tabs.text).toContain('Board: 0 todo · 1 doing · 1 in review · 0 done')
  expect(tabs.text).toContain('https://shop.test: make it red')
  expect(submitted).toEqual(['The person commented (#1) on https://shop.test: make it red'])
  // The app gone, the pane says so and nothing is asked of it.
  const gone = await session(async argv => answer(argv), {}, [...RUNNING, { app: 'not running' }])
  const before = gone.calls.length
  await gone.hook('turn.complete', { durationMs: 31_000, answer: 'Done' })
  expect(gone.calls).toHaveLength(before)
  expect(((await gone.hook('command.run', {})) as { text: string }).text).toContain('not running')
})

test('a watcher that ends is started again after a pause that doubles, not in a loop', async () => {
  const { spawned, paused } = await session(async argv => answer(argv), {}, [], 4)
  expect(spawned).toHaveLength(5)
  expect(paused).toEqual([5000, 10_000, 20_000, 40_000, 60_000])
})

test('a hung app is not asked again under another name, and the session ends without waiting long', async () => {
  let hanging = false
  const { calls, hook } = await session(async (argv, timeoutMs) => (hanging ? hung(timeoutMs) : answer(argv)))
  hanging = true
  const before = calls.length
  const started = Date.now()
  await hook('session.end', {})
  const ended = calls.slice(before)
  expect(ended).toHaveLength(1)
  expect(ended[0]?.argv.slice(1)).toEqual(['window', '--project', '/work/project/nested', '--end'])
  expect(ended[0]?.timeoutMs).toBeLessThanOrEqual(1500)
  expect(Date.now() - started).toBeLessThan(2500)
})

test('in a terminal of the desk, a page the person asks for opens in its window, not another browser', async () => {
  const told = async (vars: Record<string, string>) => {
    const { appended, hook } = await session(async argv => answer(argv), vars)
    await hook('session.end', {})
    return appended
  }
  // Said once, as a row the model reads ahead of the first prompt: the prompt itself is left
  // alone, since where an organization seats a policy the mod's part of it is passed over.
  const inDesk = await told({ TERM_PROGRAM: 'llxterm' })
  expect(inDesk).toHaveLength(1)
  expect(inDesk[0]!.message.type).toBe('user')
  const text = inDesk[0]!.message.content[0]!.text
  expect(text).toContain('llxterm window')
  expect(text).toContain('Claude in Chrome')
  // Elsewhere (another terminal) nothing is said: the person may mean their own browser.
  expect(await told({ TERM_PROGRAM: 'WarpTerminal' })).toEqual([])
})

/** The session ids the desk was told since `from`. */
const toldSessions = (calls: { argv: string[] }[], from = 0) =>
  calls
    .slice(from)
    .map(call => call.argv.at(-1)!)
    .filter(last => last.startsWith('{'))
    .map(last => JSON.parse(last) as { session?: string })
    .flatMap(said => (said.session === undefined ? [] : [said.session]))

test('the terminal is told its session once, and again only when the session changes', async () => {
  let id = 'sess-1'
  const { calls, hook } = await session(async argv => answer(argv), { LLXTERM_PANE: 't3' }, RUNNING, 0, () => id)
  expect(toldSessions(calls)).toEqual(['sess-1'])
  const call = calls.find(call => call.argv.at(-1)!.includes('"session"'))!
  expect(call.argv.slice(1, 4)).toEqual(['window', '--project', '/work/project/nested'])
  expect(JSON.parse(call.argv.at(-1)!)).toEqual({ action: 'interact', session: 'sess-1' })
  await hook('turn.start', {})
  expect(toldSessions(calls)).toEqual(['sess-1'])
  // `/clear` or `/resume` gave it another: said at the next turn, once.
  id = 'sess-2'
  await hook('turn.start', { agentId: 'a1' })
  await hook('turn.start', {})
  await hook('turn.start', {})
  expect(toldSessions(calls)).toEqual(['sess-1', 'sess-2'])
})

test('outside a terminal of the app the session is told to nobody', async () => {
  const { calls, hook } = await session(async argv => answer(argv))
  await hook('turn.start', {})
  expect(toldSessions(calls)).toEqual([])
})
