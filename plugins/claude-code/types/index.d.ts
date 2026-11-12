export type Desk = {
  app: string
  window: string
  terminals: string
  tabs: string[]
  comments: { at: string; text: string }[]
}

/** The project's board as the pane shows it, read from `.localix/board.json`. */
export type BoardSummary = {
  /** Ideas: not work until the person moves one to Todo. */
  backlog: number
  todo: number
  doing: number
  review: number
  done: number
  blocked: number
  /** Who has a card in Doing: `c3 tests`, `c5` for the main agent's. */
  working: string[]
}

/** A card as `.localix/board.json` keeps it: the fields the plugin reads. */
export type BoardCard = {
  id: string
  title: string
  column: 'backlog' | 'todo' | 'doing' | 'review' | 'done' | 'discarded'
  who?: string
  blocked?: string
  result?: string
}

declare module 'claude-code' {
  interface PluginState {
    'llxterm': { desk: Shaped<Desk>; board: Shaped<BoardSummary | null> }
  }
}
