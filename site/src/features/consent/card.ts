// The usage statistics card: one sentence and two equal buttons, over the page's lower corner. It
// is plain markup and a few listeners, not React, so a docs page (no React in the browser) shows
// the same card as the landing. It is a named region, not a dialog: it takes no focus at a page's
// load, blocks nothing, and is fixed, so its arrival moves nothing on the page.
import { SITE } from '@/shared/config';
import { escapeHtml } from '@/shared/lib/html';
import { answer, measurementId, privacySignal, resumeAnalytics, storedAnswer, type Answer } from '@/shared/analytics';

const CARD = 'consent';
/** The attribute of anything that opens the card again: the footer's "Cookie settings". */
export const OPEN = 'data-consent-open';

/** The card's markup. `specimen` draws it in place, for the kit page. */
export function consentMarkup(copy: { notice: string; privacy?: string | undefined }, specimen = false): string {
  const privacy = copy.privacy ? ` <a href="${escapeHtml(copy.privacy)}">Privacy</a>` : '';
  const button = (value: Answer, label: string) => `<button type="button" class="action" data-consent="${value}">${label}</button>`;
  return [
    `<section class="consent${specimen ? ' consent-specimen' : ''}"${specimen ? '' : ` id="${CARD}"`} aria-label="Usage statistics">`,
    `<p>${escapeHtml(copy.notice)}${privacy}</p>`,
    `<div class="consent-actions">${button('denied', 'No thanks')}${button('granted', 'Allow')}</div>`,
    '</section>',
  ].join('');
}

/** At a page's load: measure if this device said yes; ask if it has not answered and sent no
 *  privacy signal; open the card again from anything marked `data-consent-open`. */
export function startConsent(): void {
  const copy = SITE.analytics;
  if (!copy || !measurementId()) return;
  resumeAnalytics();
  if (!storedAnswer() && !privacySignal()) show(copy, false);
  document.addEventListener('click', (event) => {
    if (!(event.target instanceof Element) || !event.target.closest(`[${OPEN}]`)) return;
    event.preventDefault();
    show(copy, true);
  });
}

function show(copy: { notice: string; privacy?: string | undefined }, asked: boolean): void {
  let card = document.getElementById(CARD);
  if (!card) {
    document.body.insertAdjacentHTML('beforeend', consentMarkup(copy));
    card = document.getElementById(CARD);
    card?.addEventListener('click', (event) => {
      const button = event.target instanceof Element ? event.target.closest<HTMLButtonElement>('[data-consent]') : null;
      if (!button) return;
      answer(button.dataset.consent === 'granted' ? 'granted' : 'denied');
      card?.remove();
    });
  }
  // Asked for from the footer, the card is where the visitor's attention is: focus its first choice.
  if (asked) card?.querySelector('button')?.focus();
}
