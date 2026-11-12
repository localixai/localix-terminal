import type { Block, Card, LandingContent, Media } from '@/shared/config';

// The landing's words. The design is the template's; this file is the site's. The stills are
// the app's own window; what a terminal shows here was printed by the app, from real calls.
const DOCS = '/docs';
const GITHUB = 'https://github.com/rsaryev/localix-terminal';

const terminal = (blocks: Block[], label: string): Media => ({ kind: 'terminal', blocks, label });
/** The window's stills, all of one size: what each shows, in words. */
const SHOT = { width: 1720, height: 1134 };
const SHOTS = {
  window:
    'A window on a project: the side lists its board and its session, Claude Code, over the tree of the project’s files; Claude Code runs in a terminal, and the page and the file opened in its session are tabs on the terminal’s right',
  board: 'The project’s board: cards in Todo, one an agent is doing, one handed in for review',
  store: 'The Store: plugins, skills and MCP servers for coding agents, in one list with a search field',
  editor: 'A file of the project in the editor, three more open in tabs over it, with the project’s tree in the side',
};
const shot = (name: keyof typeof SHOTS): Media => ({ kind: 'image', src: `/shots/${name}.jpg`, alt: SHOTS[name], ...SHOT });

const CARDS: Card[] = [
  {
    tag: 'Sessions',
    title: 'A real terminal, for you and for agents',
    text: 'Your shell on a PTY, in sessions that belong to the project. Claude Code, Codex, Gemini CLI and the rest run in them as their makers ship them, with their own commands, sign-in and updates. A session shows the mark and title of what runs in it, and the files and pages opened in it are its own tabs.',
    links: [{ href: `${DOCS}/terminals`, label: 'Terminals and sessions' }],
    media: shot('window'),
  },
  {
    tag: 'Board',
    title: 'Work handed out card by card',
    text: 'The project’s kanban, kept in the project as `.localix/board.json` for git. Hand a card to an agent and it starts in a terminal of the project; what it hands in comes back to Review, and only you accept.',
    links: [{ href: `${DOCS}/board`, label: 'The board' }],
    media: shot('board'),
  },
  {
    tag: 'Store',
    title: 'One Store for every agent',
    text: 'Plugins, skills and MCP servers in one searchable list, installed into Claude Code, Codex, Gemini CLI, GitHub Copilot or OpenCode. Before anything is written or run you are shown exactly what it would do.',
    links: [{ href: `${DOCS}/store`, label: 'The Store' }],
    media: shot('store'),
  },
  {
    tag: 'Projects',
    title: 'A project is a folder, and it opens as you left it',
    text: 'Its sessions, tabs, history and board are its own, even after a restart. The project’s tree is live as an agent changes it, and a file opens in an editor tab beside the terminal.',
    links: [
      { href: `${DOCS}/projects`, label: 'Projects' },
      { href: `${DOCS}/files`, label: 'Files and the editor' },
    ],
    media: shot('editor'),
  },
  {
    tag: 'Native',
    title: 'No model, no account, no port',
    text: 'One small Rust program with the system’s own WebView, no Electron. The agents are the ones you already run; the app listens only on a socket of your own user.',
    links: [{ href: `${DOCS}/security`, label: 'Security' }],
    media: terminal(
      [
        {
          grid: [
            [['hi', 'The app'], ['dim', 'about 6 MB on macOS, signed and notarized']],
            [['hi', 'Models'], ['dim', 'none inside: it calls no model and knows no agent']],
            [['hi', 'Account'], ['dim', 'none: nothing to sign in to']],
            [['hi', 'Network'], ['dim', 'no listening port: a unix socket, mode 0600']],
            [['hi', 'License'], ['dim', 'MIT']],
          ],
        },
      ],
      'What the app is made of: about 6 megabytes, no model, no account, no listening port, MIT-licensed',
    ),
  },
];

const landing: LandingContent = {
  hero: {
    title: { rest: 'The terminal for you and your coding agents' },
    lede: 'Terminal sessions that belong to a project and come back as you left them. Claude Code, Codex or any CLI agent runs in them as it ships, and uses the same browser, board and Store you do.',
    under: 'Free and open source, MIT',
    links: [{ label: 'Get started', href: `${DOCS}/getting-started` }],
    tour: {
      label: 'The app’s window on a project, in four stills',
      ...SHOT,
      scenes: [
        { label: 'Sessions', src: '/shots/window.jpg', alt: SHOTS.window },
        { label: 'Board', src: '/shots/board.jpg', alt: SHOTS.board },
        { label: 'Store', src: '/shots/store.jpg', alt: SHOTS.store },
        { label: 'Editor', src: '/shots/editor.jpg', alt: SHOTS.editor },
      ],
    },
  },
  loop: {
    title: 'What a terminal alone lacks',
    aside: 'Five things in the window',
    cards: CARDS,
  },
  docs: { title: 'Documentation' },
  closing: {
    title: 'Start in three steps',
    aside: 'Free · MIT license',
    steps: [
      { title: 'Install', text: `The download, or [one line in a terminal](${DOCS}/getting-started). The app is all there is: it holds the window, the \`llxterm\` command and the MCP server.` },
      { title: 'Open a project', text: '`llxterm open .` in a folder, or Open Folder… (⌘O) in the window.' },
      { title: 'Open a terminal', text: 'Your shell, or an agent you have installed: it starts in a terminal of the project, already connected to the window.' },
    ],
    links: [
      { label: 'Get started', href: `${DOCS}/getting-started` },
      { label: 'Source on GitHub', href: GITHUB },
    ],
  },
};

export default landing;
