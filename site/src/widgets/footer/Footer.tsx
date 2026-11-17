import { OPEN } from '@/features/consent';
import { measurementId } from '@/shared/analytics';
import { SITE } from '@/shared/config';
import './Footer.css';

/** Under every page, one strip: who makes the product, the site's one line, then where else to
 *  go. The maker's name is the way to the page of everything they make. With usage statistics on,
 *  the last link opens their card again, so a visitor can change their answer from any page. */
export function Footer() {
  const { maker } = SITE;
  return (
    <footer className="foot wrap">
      <p>
        {maker && (
          <a className="foot-maker" href={maker.href}>
            {maker.label}
          </a>
        )}
        {SITE.footer.note}
      </p>
      <nav aria-label="Footer">
        {SITE.footer.links.map((link) => (
          <a key={link.href} className="label" href={link.href}>
            {link.label}
          </a>
        ))}
        {measurementId() && (
          <button type="button" className="label foot-consent" {...{ [OPEN]: '' }}>
            Cookie settings
          </button>
        )}
      </nav>
    </footer>
  );
}
