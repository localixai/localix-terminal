//! Hook regressions against a small host mock; no installed Claude session or desk is needed.

import { expect, mock, test } from 'bun:test'

mock.module('claude-code', () => ({
  atom: (_key: unknown, value: unknown) => ({ value }),
  read: async (_api: unknown, atom: { value: unknown }) => atom.value,
  update: async (_api: unknown, atom: { value: unknown }, change: (value: unknown) => unknown) => {
    atom.value = change(atom.value)
  },
}))

const { register } = await import('./register')

test('turn notifications stay in the session project, including taking the ask back', async () => {
  const calls: string[][] = []
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
      run: async (argv: string[]) => {
        calls.push(argv)
        if (argv[0] === 'git') return { exitCode: 0, stdout: '/work/project\n' }
        if (argv.includes('status')) return { exitCode: 0, stdout: 'running\nwindow: shown\nterminals: 0\n' }
        return { exitCode: 0, stdout: 'No tabs open.' }
      },
      spawn: () => ({
        async *[Symbol.asyncIterator]() {
          followed()
        },
      }),
    },
  }
  register(on as never, {} as never)
  const next = async (event: unknown) => event
  await hooks.get('session.start')!(api, {}, next)
  await following
  const before = calls.length
  const completed = { durationMs: 31_000, answer: 'The cart is fixed' }
  await hooks.get('turn.complete')!(api, completed, next)
  await hooks.get('turn.start')!(api, {}, next)
  const asks = calls.slice(before)
  expect(asks).toHaveLength(2)
  for (const argv of asks) {
    expect(argv.slice(1, 4)).toEqual(['window', '--project', '/work/project/nested'])
  }
  expect(JSON.parse(asks[0].at(-1)!)).toEqual({ action: 'interact', ask: 'Claude finished: The cart is fixed' })
  expect(JSON.parse(asks[1].at(-1)!)).toEqual({ action: 'interact', ask: '' })

  const after = calls.length
  await hooks.get('turn.complete')!(api, { ...completed, durationMs: 100 }, next)
  await hooks.get('turn.complete')!(api, { ...completed, isAborted: true }, next)
  expect(calls).toHaveLength(after)
  await hooks.get('session.end')!(api, {}, next)
})
