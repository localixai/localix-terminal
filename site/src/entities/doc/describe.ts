// A page's description, for its meta tag, the docs' cards and llms.txt.

/**
 * A page's description from its first paragraph, short enough that a search result shows it whole
 * (about 160 characters): the whole sentences that fit, or, when the first alone is longer, its
 * words up to the limit and an ellipsis.
 */
export function cut(text: string, most = 160): string {
  if (text.length <= most) return text;
  let end = -1;
  for (const m of text.matchAll(/[.!?](?=\s)/g)) {
    if (m.index >= most) break;
    end = m.index + 1;
  }
  if (end >= 60) return text.slice(0, end);
  const head = text.slice(0, most - 1);
  return `${head.slice(0, head.lastIndexOf(' ')).replace(/[,:;.]$/, '')}…`;
}
