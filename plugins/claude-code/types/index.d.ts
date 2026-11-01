export type Desk = {
  daemon: string
  window: string
  terminals: string
  tabs: string[]
  comments: { at: string; text: string }[]
}

declare module 'claude-code' {
  interface PluginState {
    'llx-desk': { desk: Shaped<Desk> }
  }
}
