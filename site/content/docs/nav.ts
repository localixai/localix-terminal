import type { DocsConfig } from '@/shared/config';

// The docs' table of contents. Every markdown file under content/docs is a page listed here, in
// a group, or is named under `unlisted` with why: a file in neither fails the build.
const docs: DocsConfig = {
  index: {
    title: 'Localix Terminal docs',
    description:
      'How to install Localix Terminal, connect your coding agents, and work in the window: terminal sessions, the browser, the board, the Store, and the one command agents use.',
  },
  groups: [
    {
      title: 'Start',
      blurb: 'From nothing to an agent working in a project.',
      pages: ['getting-started', 'agents'],
    },
    {
      title: 'In the window',
      blurb: 'What a project has open, and how each part behaves.',
      pages: ['terminals', 'browser', 'board', 'store', 'files', 'projects', 'shortcuts'],
    },
    {
      title: 'For agents',
      blurb: 'The one command an agent or a script works in the window with.',
      pages: ['window-tool'],
    },
    {
      title: 'Reference',
      blurb: 'The switches, the protocol, and what the app does and does not allow.',
      pages: ['configuration', 'protocol', 'security'],
    },
  ],
  unlisted: {},
};

export default docs;
