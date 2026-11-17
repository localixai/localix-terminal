// The doc: a page of markdown under content/docs (or content/pages, a page of its own), read, checked and rendered at build time.
// Nothing here runs in a browser.
export { GROUPS, IMAGES, PAGES, REPO_FILES, type DocPage } from './content';
export { renderMarkdown, type Heading } from './markdown';
export { SOLO_PAGES, type SoloPage } from './solo';
