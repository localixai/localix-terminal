// Every event a site sends beyond Google's own (page views, scrolls, outbound clicks, downloads),
// with what each may carry. A value is a member of a closed set or a boolean, never free text:
// nothing a visitor typed, no search query, no address but the page's own.
import { SITE } from '@/shared/config';

export type AnalyticsEvents = {
  /** The landing's install command copied; `method` is the tab's label from `SITE.install`. */
  install_copied: { method: string };
  /** A docs code block copied. */
  code_copied: Record<string, never>;
  /** The docs' search closed after a query; `found` is whether its last query had results. */
  docs_searched: { found: boolean };
};

export type AnalyticsEvent = keyof AnalyticsEvents;

/** Each string parameter's closed set; a value outside it drops the event. */
const SETS: { readonly [P in string]?: () => readonly string[] } = {
  // A site may have no install command: its config can leave it out.
  method: () => (SITE.install ?? []).map(([label]) => label),
};

/** The event's parameters as GA4 takes them, or undefined when one is outside its set. */
export function wireParams(params: Record<string, string | boolean>): Record<string, string> | undefined {
  const wire: Record<string, string> = {};
  for (const [key, value] of Object.entries(params)) {
    if (typeof value === 'boolean') {
      wire[key] = String(value);
      continue;
    }
    if (!SETS[key]?.().includes(value)) return undefined;
    wire[key] = value;
  }
  return wire;
}
