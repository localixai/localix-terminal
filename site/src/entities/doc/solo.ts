// A site's pages outside the docs: a markdown file in content/pages is a page at the site's root
// (content/pages/privacy.md is /privacy), with the bar and the footer and no docs menu. They take
// the docs' standard for a page's shape, and are not in the docs' menu, search or llms.txt.
import { DOCS } from '@/shared/config';
import { cut } from './describe';
import { read, render, type Heading } from './markdown';

const ROOT = 'content/pages/';
const NAME = /^[a-z0-9]+(-[a-z0-9]+)*$/;
/** Addresses the site already has. */
const TAKEN = new Set([DOCS.slice(1), 'kit', 'index', '404']);

const FILES = import.meta.glob<string>('../../../content/pages/*.md', {
  query: '?raw',
  import: 'default',
  eager: true,
});

export type SoloPage = {
  route: string;
  source: string;
  title: string;
  titleId: string;
  description: string;
  headings: Heading[];
  html: string;
};

const problems: string[] = [];

/** Every page of content/pages, by name. */
export const SOLO_PAGES: SoloPage[] = Object.entries(FILES)
  .sort(([a], [b]) => a.localeCompare(b))
  .flatMap(([key, text]) => {
    const source = key.replace(/^(\.\.\/)+/, '');
    const slug = source.slice(ROOT.length, -'.md'.length);
    if (!NAME.test(slug)) problems.push(`${source}: a page's name is lower-case words joined by hyphens`);
    if (TAKEN.has(slug)) problems.push(`${source}: /${slug} is the site's own address`);
    try {
      const { summary, body, ...parsed } = read(text);
      // A link is written as it is meant: a page outside the docs links by address, not by file.
      return [{ ...parsed, route: `/${slug}`, source, description: cut(summary), html: render(body, (href) => href) }];
    } catch (error) {
      problems.push(`${source}: ${error instanceof Error ? error.message : String(error)}`);
      return [];
    }
  });

if (problems.length > 0) {
  throw new Error(`The pages' markdown has problems:\n  ${problems.join('\n  ')}`);
}
