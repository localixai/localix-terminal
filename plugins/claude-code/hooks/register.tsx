import { atom, read, update } from 'claude-code'
import type { Register } from 'claude-code'

import type { Desk } from '../types'

const EMPTY: Desk = { daemon: 'not running', window: '-', terminals: '0', tabs: [], comments: [] }
// Change the tag when Desk's fields change: it declines what an earlier version stored.
const desk = atom({ plugin: 'llx-desk', key: 'desk' } as const, EMPTY, { shape: 'desk-1' })

const time = () => new Date().toTimeString().slice(0, 5)

/** The first line of a comment block: `The person commented (#1) on URL: words`. */
const words = (text: string) =>
  (text.split('\n').find(line => line.startsWith('The person commented')) ?? text.split('\n')[0] ?? '')
    .replace(/^The person commented \(#\d+\) on /, '')

export const register: Register = on => {
  on('session.start', async ($, e, next) => {
    const started = await next(e)
    // The program: the one on PATH, else the release build the plugin's launcher fetched (kept
    // under ~/.local/share/llx-desk/current). Never fetched from here: the pane only reads.
    const home = await $.env.get('HOME')
    const kept = `${home ?? ''}/.local/share/llx-desk/current/llx-desk`
    let program = 'llx-desk'
    const run = async (argv: string[]) => {
      try {
        return await $.process.run([program, ...argv], { timeoutMs: 5000 })
      } catch {
        if (program === kept || !home) return undefined
        program = kept
        return await $.process.run([program, ...argv], { timeoutMs: 5000 }).catch(() => undefined)
      }
    }
    // The status line an earlier version pinned: its notice read like a warning.
    $.ui.status(undefined)
    await $.command.register({ name: 'tabs', description: "This project's tabs and the person's latest comments" })

    // What the daemon holds, every few seconds. `status` never starts a daemon, and the tabs are
    // listed only while one runs, so the pane never opens a window by itself.
    const refresh = async () => {
      const status = await run(['status'])
      if (!status || status.exitCode !== 0) {
        await update($, desk, d => ({ ...d, daemon: 'not running', window: '-', terminals: '0', tabs: [] }))
        return
      }
      const lines = status.stdout.split('\n')
      const field = (name: string) => lines.find(l => l.startsWith(`${name}: `))?.slice(name.length + 2) ?? '-'
      const list = await run(['window', '{"action":"list"}'])
      const tabs = (list?.stdout ?? '').split('\n').filter(l => /^[wt]\d+ /.test(l))
      await update($, desk, d => ({
        ...d,
        daemon: lines[0] ?? 'running',
        window: field('window'),
        terminals: field('terminals'),
        tabs,
      }))
    }
    $.clock.every(3000, () => void refresh())

    // The person's comments, as they come: kept for the pane, and each one a turn of its own,
    // so Claude wakes on a comment without anyone typing. The first look settles which program.
    void (async () => {
      await refresh()
      const follow = $.process.spawn({ argv: [program, 'window', '--said', '--follow'] })
      for await (const piece of follow) {
        if (piece.stream !== 'stdout' || !piece.text.trim()) continue
        const text = piece.text.trim()
        await update($, desk, d => ({ ...d, comments: [...d.comments, { at: time(), text: words(text) }].slice(-20) }))
        $.ui.toast(`llx-desk: ${words(text).slice(0, 80)}`)
        void $.prompt.submit({ text })
      }
    })()

    return started
  })

  // A short answer in the chat, not a pane: three facts do not need a column of their own.
  on('command.run', { command: 'tabs' }, async $ => {
    const d = await read($, desk)
    if (d.daemon === 'not running') return { text: 'llx-desk is not running.' }
    const tabs = d.tabs.length ? d.tabs.map(tab => `  ${tab}`).join('\n') : '  none open'
    const last = d.comments.slice(-3).map(c => `  ${c.at} ${c.text}`).join('\n')
    const said = last ? `\nLast comments:\n${last}` : ''
    return { text: `${d.daemon} · window ${d.window}\nTabs of this project:\n${tabs}${said}` }
  })

  // A quiet line above the prompt while the daemon runs: the tabs, and the last comment.
  on('ui.render', { component: 'AbovePrompt' }, async ($, e, next) => {
    const d = await read($, desk)
    if (e.props.hasSurvey || d.daemon === 'not running') return next(e)
    const { Box, Text } = $.ui.resolve(e)
    const tabs = `${d.tabs.length} tab${d.tabs.length === 1 ? '' : 's'}`
    const last = d.comments[d.comments.length - 1]
    return (
      <Box>
        <Text color="cyan">◐ desk</Text>
        <Text dimColor>
          {' '}· {tabs}
          {d.terminals !== '0' ? ` · ${d.terminals} terminal${d.terminals === '1' ? '' : 's'}` : ''}
          {last ? ` · last comment ${last.at}` : ''} · /tabs
        </Text>
      </Box>
    )
  })

}
