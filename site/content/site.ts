import type { SiteConfig } from '@/shared/config';

// What the site knows about itself: the product's name, where the site is published, the
// repository, the builds a reader downloads.
const GITHUB = 'https://github.com/localixai/localix-terminal';
const RELEASE = `${GITHUB}/releases/latest/download`;
const DESCRIPTION =
  'Terminal sessions that belong to a project and come back as you left them. Claude Code, Codex or any CLI agent runs in them as it ships, and uses the same browser, board and Store you do.';

const site: SiteConfig = {
  name: 'Localix Terminal',
  title: 'Localix Terminal: the terminal for you and your coding agents',
  description: DESCRIPTION,
  url: 'https://terminal.localix.ai',
  github: GITHUB,
  // The site is a folder of the product's repository, not a repository of its own.
  sourceDir: 'site',
  // The app's builds: the landing offers the one for the reader's system, macOS's otherwise.
  downloads: [
    { os: 'macos', label: 'Download for macOS', href: `${RELEASE}/Localix-Terminal-macos.dmg`, note: 'Apple Silicon, macOS 12 or later', also: 'Also for macOS' },
    {
      os: 'linux',
      label: 'Download for Linux',
      href: `${RELEASE}/llxterm-x86_64-unknown-linux-gnu.tar.gz`,
      note: 'x86_64, with WebKitGTK 4.1',
      also: 'Also for Linux',
    },
  ],
  image: {
    path: '/og.jpg',
    width: 2400,
    height: 1260,
    alt: 'Localix Terminal, the terminal for you and your coding agents: the app’s window on a project, with Claude Code in a terminal and the page it works on beside it',
  },
  bar: {
    links: [
      { label: 'Docs', href: '/docs' },
      { label: 'Changelog', href: `${GITHUB}/blob/main/CHANGELOG.md` },
      { label: 'GitHub', href: GITHUB },
    ],
    action: { label: 'Download', href: '/#start' },
  },
  footer: {
    note: 'Localix Terminal is MIT-licensed and open source. The app and its command carry the name Localix Terminal, llxterm.',
    links: [
      { label: 'Docs', href: '/docs' },
      { label: 'GitHub', href: GITHUB },
      { label: 'Releases', href: `${GITHUB}/releases` },
      { label: 'Changelog', href: `${GITHUB}/blob/main/CHANGELOG.md` },
    ],
  },
  jsonLd: {
    '@type': 'SoftwareApplication',
    applicationCategory: 'DeveloperApplication',
    operatingSystem: 'macOS, Linux',
    license: 'https://opensource.org/licenses/MIT',
    offers: { '@type': 'Offer', price: '0', priceCurrency: 'USD' },
  },
  llms: '> Localix Terminal is an open source terminal for a person and their coding agents: its sessions belong to a project and come back as they were left, and Claude Code, Codex or any CLI agent runs in them as it ships and uses the same browser, board and Store the person does.',
};

export default site;
