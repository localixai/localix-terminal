//! Hook regressions against a small host mock; no installed Claude session or desk is needed.

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

/** What the desk answers, wherever its program is. */
const answer = (argv: string[]): Ran => {
  if (argv.includes('status')) return { exitCode: 0, stdout: 'running\nwindow: shown\nterminals: 0\n' }
  return { exitCode: 0, stdout: 'No tabs open.' }
}

/** A session on a mock host whose programs answer as `runner` says; its hooks, once started. */
async function session(runner: Runner) {
  const calls: { argv: string[]; timeoutMs: number }[] = []
  const spawned: string[][] = []
  type Handler = (api: unknown, event: unknown, next: (event: unknown) => Promise<unknown>) => Promise<unknown>
  const hooks = new Map<string, Handler>()
  const on = (event: string, filterOrHandler: unknown, handler?: Handler) => {
    hooks.set(event, handler ?? filterOrHandler as Handler)
    return { catch: () => undefined }
  }
  let followed!: () => void
  const following = new Promise<void>(resolve => { followed = resolve })
  const api = {
    env: { get: async () => '/home/test' },
    session: { cwd: async () => '/work/project/nested' },
    fs: { read: async () => '{"cards":[]}' },
    ui: { status: () => undefined },
    command: { register: async () => undefined },
    clock: {
      every: () => undefined,
      sleep: async () => { throw new Error('the mock host ended') },
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
          followed()
        },
      }),
    },
  }
  register(on as never, {} as never)
  const next = async (event: unknown) => event
  await hooks.get('session.start')!(api, {}, next)
  await following
  const hook = (name: string, event: unknown) => hooks.get(name)!(api, event, next)
  return { calls, spawned, hook }
}

/** A program that cannot start, as a missing one fails: at once. */
const missing = async (): Promise<Ran> => {
  throw new Error('ENOENT: no such file or directory')
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

test('a program missing from the PATH is tried once, then the one that ran is asked first', async () => {
  const local = '/home/test/.local/bin/llx-desk'
  const { calls, spawned, hook } = await session(async argv => (argv[0] === local ? answer(argv) : missing()))
  // The first look finds the program: the PATH's fails, then ~/.local/bin's runs.
  expect(calls.slice(0, 2).map(call => call.argv[0])).toEqual(['llx-desk', local])
  const later = calls.slice(1).map(call => call.argv[0])
  expect(later.every(program => program === local)).toBe(true)
  expect(spawned[0]?.[0]).toBe(local)
  const before = calls.length
  await hook('turn.complete', { durationMs: 31_000, answer: 'Done' })
  expect(calls.slice(before).map(call => call.argv[0])).toEqual([local])
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
