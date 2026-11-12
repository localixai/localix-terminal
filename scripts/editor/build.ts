//! Builds the editor's vendored scripts into `src/editor/vendor`, and the table of them with
//! their SHA-384s in `src/editor/vendored.rs`: `bun install --frozen-lockfile && bun build.ts`.
//!
//! The core holds the packages every language shares; each language is a script of its own that
//! takes those packages from the core, so a page runs the core and the one language it shows.

import { createHash } from "node:crypto";
import { mkdirSync, readdirSync, rmSync } from "node:fs";
import { join } from "node:path";

const here = import.meta.dir;
const out = join(here, "../../src/editor/vendor");

/// The packages a language's script takes from the core, or from the language script named
/// with it, which the page runs first.
const shared: Record<string, string | null> = {
  "@codemirror/state": null,
  "@codemirror/view": null,
  "@codemirror/language": null,
  "@lezer/common": null,
  "@lezer/lr": null,
  "@lezer/highlight": null,
  "@codemirror/lang-javascript": "javascript",
  "@codemirror/lang-css": "css",
};

/// The packages a language names but never runs here: what completes or nests another language
/// inside it. Each is left out whole, so no second copy of it is carried.
const unused = ["@codemirror/lang-html", "@codemirror/autocomplete"];

const taken = (own: string) => ({
  name: "shared",
  setup(build: any) {
    build.onResolve({ filter: /^@(codemirror|lezer)\// }, (args: any) => {
      if (args.path in shared && shared[args.path] !== own) {
        return { path: args.path, namespace: "shared" };
      }
      if (unused.includes(args.path) && !args.path.endsWith(`/lang-${own}`)) {
        return { path: args.path, namespace: "unused" };
      }
      return undefined;
    });
    build.onLoad({ filter: /.*/, namespace: "shared" }, (args: any) => ({
      contents: `module.exports = globalThis.CM.pkg[${JSON.stringify(args.path)}];`,
      loader: "js",
    }));
    build.onLoad({ filter: /.*/, namespace: "unused" }, () => ({
      contents: "module.exports = {};",
      loader: "js",
    }));
  },
});

async function bundle(name: string): Promise<string> {
  const built = await Bun.build({
    entrypoints: [join(here, "src", `${name}.js`)],
    minify: true,
    format: "iife",
    target: "browser",
    plugins: name === "core" ? [] : [taken(name)],
  });
  if (!built.success) throw new Error(built.logs.join("\n"));
  // A page holds the script inside its own element: nothing in it may read as that element's end
  // or as the start of a comment.
  const text = (await built.outputs[0].text())
    .replaceAll("</script", "<\\/script")
    .replaceAll("<!--", "<\\!--");
  return text.trimEnd() + "\n";
}

const names = readdirSync(join(here, "src"))
  .filter((file) => file.endsWith(".js"))
  .map((file) => file.slice(0, -3))
  .sort((a, b) => (a === "core" ? -1 : b === "core" ? 1 : a.localeCompare(b)));

rmSync(out, { recursive: true, force: true });
mkdirSync(out, { recursive: true });
const rows: string[] = [];
for (const name of names) {
  const text = await bundle(name);
  await Bun.write(join(out, `${name}.js`), text);
  const hash = createHash("sha384").update(text).digest("base64");
  rows.push(
    `    Script {\n        name: "${name}",\n        text: include_str!("vendor/${name}.js"),\n        sha384: "${hash}",\n    },`,
  );
  console.log(`${name}.js ${text.length}`);
}

await Bun.write(
  join(here, "../../src/editor/vendored.rs"),
  `//! The editor's vendored scripts and their SHA-384s: the core every page runs, then a script a
//! language. Written by \`scripts/editor/build.ts\`, never by hand; a page runs each by its hash,
//! so a script that was altered never runs.

/// A vendored script: its name, its text, and the SHA-384 a page's policy runs it by.
pub(super) struct Script {
    pub(super) name: &'static str,
    pub(super) text: &'static str,
    pub(super) sha384: &'static str,
}

pub(super) const SCRIPTS: &[Script] = &[
${rows.join("\n")}
];
`,
);
