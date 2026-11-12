# The site

The landing page and the docs of Localix Terminal, as a folder of the product's repository. It was
made from the [llx-sitekit](https://github.com/rsaryev/llx-sitekit) template (its commit
`8e8c891`) and keeps the template's layout, so [AGENTS.md](AGENTS.md) here holds for every change
in this folder.

```sh
cd site
npm install
npm run dev          # http://localhost:5173
npm run check        # the layers, the types, the build, the docs' standard
```

## What is whose

| Path | Owner | |
| --- | --- | --- |
| `content/` | the site | `site.ts` (name, address, the builds to download), `landing.ts` (the landing's words and the window's stills), `docs/` (the pages and `nav.ts`), `theme.css` |
| `public/` | the site | the icons, the link-preview picture, the screenshots in `shots/` |
| `CHANGELOG.md` | the product | a link to the repository's own changelog, which the landing's releases are read from |
| `src/`, `scripts/`, the configs | the template | change one only where the site cannot say it in `content/` |

## What differs from a site cloned from the template

- **It has no repository of its own.** `content/site.ts` says so with `sourceDir: 'site'`, which
  `sourceUrl` in `src/shared/config/index.ts` puts into a page's "Edit on GitHub" and into a
  docs link to a repository file. The template does not have that field yet.
- **Its workflow is the repository's**, `.github/workflows/site.yml`: a workflow in a folder
  never runs, so the template's three are not here.
- **`npm run sync` does not apply.** It merges the template's branch into a repository's root.
  Until the template can sync into a folder, an update of the template is carried over by hand:
  its `CHANGELOG.md` lists what changed and what a site has to change.

## Publishing

A push to `main` that touches `site/` or the changelog runs `site.yml`: the check, then
`dist/` to Cloudflare Pages when the repository has the two secrets the workflow names.
