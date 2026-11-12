// The site's content, typed: what content/ says, for every layer to read.
import docs from '@content/docs/nav';
import landing from '@content/landing';
import site from '@content/site';

export type { Block, Cell, Line, Tone } from '@/shared/ui';
export type { Build, Card, DocsConfig, LandingContent, Media, NavLink, SiteConfig, Tour } from './types';
export { DOC_TITLES, RELEASES } from 'virtual:site-data';

export const SITE = site;
export const LANDING = landing;
export const DOCS_NAV = docs;
/** A file of the site in the repository its sources are in: `kind` is GitHub's `blob` (read it)
 *  or `edit`, and `path` is under the site's root, which may be a folder of that repository. */
export function sourceUrl(kind: 'blob' | 'edit', path: string): string {
  const dir = site.sourceDir ? `${site.sourceDir.replace(/^\/+|\/+$/g, '')}/` : '';
  return `${site.source ?? site.github}/${kind}/main/${dir}${path}`;
}
/** Where the docs live on the site. */
export const DOCS = '/docs';
