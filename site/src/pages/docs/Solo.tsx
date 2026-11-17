import type { SoloPage } from '@/entities/doc';
import { Footer } from '@/widgets/footer';
import { Nav } from '@/widgets/nav';

/** A page of content/pages: the bar, the text alone in the docs' column, the footer. No docs
 *  menu, no sections beside it, no turn to a previous or next page: it stands by itself. */
export function SoloLayout({ page }: { page: SoloPage }) {
  return (
    <>
      <Nav at="elsewhere" />
      <div className="docs docs-solo wrap">
        <main id="main" className="docs-main">
          <article className="docs-article">
            <h1 className="docs-title" id={page.titleId}>
              {page.title}
            </h1>
            <div dangerouslySetInnerHTML={{ __html: page.html }} />
          </article>
        </main>
      </div>
      <Footer />
    </>
  );
}
