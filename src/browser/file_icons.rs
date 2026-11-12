//! The pictures of a tree's entries: one for a folder, shut and open, and one for each kind of
//! file, told by its name and then by its extension. They are drawings of their own colours,
//! given to the page as data it draws, never as script.

/// Each picture by its name: an SVG, whole.
const ICONS: &[(&str, &str)] = &[
    ("astro", include_str!("file_icons/astro.svg")),
    ("audio", include_str!("file_icons/audio.svg")),
    ("binary", include_str!("file_icons/binary.svg")),
    ("bun", include_str!("file_icons/bun.svg")),
    ("c", include_str!("file_icons/c.svg")),
    ("cmake", include_str!("file_icons/cmake.svg")),
    ("config", include_str!("file_icons/config.svg")),
    ("cpp", include_str!("file_icons/cpp.svg")),
    ("csharp", include_str!("file_icons/csharp.svg")),
    ("css", include_str!("file_icons/css.svg")),
    ("csv", include_str!("file_icons/csv.svg")),
    ("db", include_str!("file_icons/db.svg")),
    ("deno", include_str!("file_icons/deno.svg")),
    ("diff", include_str!("file_icons/diff.svg")),
    ("docker", include_str!("file_icons/docker.svg")),
    ("dotenv", include_str!("file_icons/dotenv.svg")),
    ("elixir", include_str!("file_icons/elixir.svg")),
    ("eslint", include_str!("file_icons/eslint.svg")),
    ("file", include_str!("file_icons/file.svg")),
    ("folder", include_str!("file_icons/folder.svg")),
    ("folder-open", include_str!("file_icons/folder-open.svg")),
    ("font", include_str!("file_icons/font.svg")),
    ("git", include_str!("file_icons/git.svg")),
    ("go", include_str!("file_icons/go.svg")),
    ("gradle", include_str!("file_icons/gradle.svg")),
    ("graphql", include_str!("file_icons/graphql.svg")),
    ("haskell", include_str!("file_icons/haskell.svg")),
    ("html", include_str!("file_icons/html.svg")),
    ("image", include_str!("file_icons/image.svg")),
    ("java", include_str!("file_icons/java.svg")),
    ("js", include_str!("file_icons/js.svg")),
    ("json", include_str!("file_icons/json.svg")),
    ("jsx", include_str!("file_icons/jsx.svg")),
    ("jupyter", include_str!("file_icons/jupyter.svg")),
    ("kotlin", include_str!("file_icons/kotlin.svg")),
    ("less", include_str!("file_icons/less.svg")),
    ("log", include_str!("file_icons/log.svg")),
    ("lua", include_str!("file_icons/lua.svg")),
    ("markdown", include_str!("file_icons/markdown.svg")),
    ("nix", include_str!("file_icons/nix.svg")),
    ("node", include_str!("file_icons/node.svg")),
    ("npm", include_str!("file_icons/npm.svg")),
    ("objectivec", include_str!("file_icons/objectivec.svg")),
    ("pdf", include_str!("file_icons/pdf.svg")),
    ("php", include_str!("file_icons/php.svg")),
    ("pnpm", include_str!("file_icons/pnpm.svg")),
    ("powershell", include_str!("file_icons/powershell.svg")),
    ("prettier", include_str!("file_icons/prettier.svg")),
    ("protobuf", include_str!("file_icons/protobuf.svg")),
    ("python", include_str!("file_icons/python.svg")),
    ("ruby", include_str!("file_icons/ruby.svg")),
    ("rust", include_str!("file_icons/rust.svg")),
    ("scala", include_str!("file_icons/scala.svg")),
    ("scss", include_str!("file_icons/scss.svg")),
    ("shell", include_str!("file_icons/shell.svg")),
    ("sql", include_str!("file_icons/sql.svg")),
    ("svelte", include_str!("file_icons/svelte.svg")),
    ("svg", include_str!("file_icons/svg.svg")),
    ("swift", include_str!("file_icons/swift.svg")),
    ("tailwind", include_str!("file_icons/tailwind.svg")),
    ("terraform", include_str!("file_icons/terraform.svg")),
    ("text", include_str!("file_icons/text.svg")),
    ("toml", include_str!("file_icons/toml.svg")),
    ("ts", include_str!("file_icons/ts.svg")),
    ("tsconfig", include_str!("file_icons/tsconfig.svg")),
    ("tsx", include_str!("file_icons/tsx.svg")),
    ("video", include_str!("file_icons/video.svg")),
    ("vite", include_str!("file_icons/vite.svg")),
    ("vue", include_str!("file_icons/vue.svg")),
    ("wasm", include_str!("file_icons/wasm.svg")),
    ("xml", include_str!("file_icons/xml.svg")),
    ("yaml", include_str!("file_icons/yaml.svg")),
    ("yarn", include_str!("file_icons/yarn.svg")),
    ("zig", include_str!("file_icons/zig.svg")),
    ("zip", include_str!("file_icons/zip.svg")),
];

/// The picture of a file whose kind nothing tells.
const PLAIN: &str = "file";

/// Files told by their whole name, in lower case.
const NAMES: &[(&str, &str)] = &[
    ("dockerfile", "docker"),
    ("containerfile", "docker"),
    (".dockerignore", "docker"),
    ("docker-compose.yml", "docker"),
    ("docker-compose.yaml", "docker"),
    ("compose.yml", "docker"),
    ("compose.yaml", "docker"),
    ("package.json", "npm"),
    ("package-lock.json", "npm"),
    (".npmrc", "npm"),
    (".npmignore", "npm"),
    ("pnpm-lock.yaml", "pnpm"),
    ("pnpm-workspace.yaml", "pnpm"),
    ("yarn.lock", "yarn"),
    (".yarnrc", "yarn"),
    (".yarnrc.yml", "yarn"),
    ("bun.lock", "bun"),
    ("bun.lockb", "bun"),
    ("bunfig.toml", "bun"),
    ("deno.json", "deno"),
    ("deno.jsonc", "deno"),
    ("deno.lock", "deno"),
    (".gitignore", "git"),
    (".gitattributes", "git"),
    (".gitmodules", "git"),
    (".gitkeep", "git"),
    ("jsconfig.json", "tsconfig"),
    (".nvmrc", "node"),
    (".node-version", "node"),
    ("cmakelists.txt", "cmake"),
    ("go.mod", "go"),
    ("go.sum", "go"),
    ("gradlew", "gradle"),
    ("makefile", "config"),
    ("gnumakefile", "config"),
    ("justfile", "config"),
    (".editorconfig", "config"),
    ("license", "text"),
    ("licence", "text"),
    ("copying", "text"),
    ("notice", "text"),
];

/// Files told by how their name begins, in lower case: a tool's own configuration.
const STARTS: &[(&str, &str)] = &[
    (".env", "dotenv"),
    ("tsconfig.", "tsconfig"),
    (".eslintrc", "eslint"),
    ("eslint.config.", "eslint"),
    (".prettierrc", "prettier"),
    (".prettierignore", "prettier"),
    ("prettier.config.", "prettier"),
    ("vite.config.", "vite"),
    ("tailwind.config.", "tailwind"),
    ("dockerfile.", "docker"),
];

/// Files told by their extension, in lower case.
const KINDS: &[(&[&str], &str)] = &[
    (&["rs"], "rust"),
    (&["ts", "mts", "cts"], "ts"),
    (&["tsx"], "tsx"),
    (&["js", "mjs", "cjs"], "js"),
    (&["jsx"], "jsx"),
    (&["json", "jsonc", "json5", "map"], "json"),
    (&["md", "mdx", "markdown"], "markdown"),
    (&["html", "htm", "xhtml"], "html"),
    (&["css"], "css"),
    (&["scss", "sass"], "scss"),
    (&["less"], "less"),
    (&["py", "pyi", "pyw"], "python"),
    (&["ipynb"], "jupyter"),
    (&["rb", "erb", "gemspec"], "ruby"),
    (&["go"], "go"),
    (&["java", "jar"], "java"),
    (&["kt", "kts"], "kotlin"),
    (&["scala", "sc"], "scala"),
    (&["swift"], "swift"),
    (&["c", "h"], "c"),
    (&["cpp", "cc", "cxx", "hpp", "hh", "hxx"], "cpp"),
    (&["cs"], "csharp"),
    (&["m", "mm"], "objectivec"),
    (&["php"], "php"),
    (&["lua"], "lua"),
    (&["zig"], "zig"),
    (&["hs"], "haskell"),
    (&["ex", "exs"], "elixir"),
    (&["nix"], "nix"),
    (&["tf", "tfvars"], "terraform"),
    (&["proto"], "protobuf"),
    (&["graphql", "gql"], "graphql"),
    (&["sql"], "sql"),
    (&["db", "sqlite", "sqlite3"], "db"),
    (&["sh", "bash", "zsh", "fish"], "shell"),
    (&["ps1", "psm1"], "powershell"),
    (&["yml", "yaml"], "yaml"),
    (&["toml"], "toml"),
    (&["xml", "plist", "xsd"], "xml"),
    (&["svg"], "svg"),
    (
        &[
            "png", "jpg", "jpeg", "gif", "webp", "avif", "ico", "icns", "bmp", "tiff", "heic",
        ],
        "image",
    ),
    (&["mp4", "mov", "webm", "mkv", "avi"], "video"),
    (&["mp3", "wav", "ogg", "flac", "m4a", "aac"], "audio"),
    (&["ttf", "otf", "woff", "woff2"], "font"),
    (&["pdf"], "pdf"),
    (
        &[
            "zip", "tar", "gz", "tgz", "bz2", "xz", "zst", "7z", "rar", "dmg",
        ],
        "zip",
    ),
    (&["txt", "rtf"], "text"),
    (&["log"], "log"),
    (&["csv", "tsv"], "csv"),
    (&["diff", "patch"], "diff"),
    (&["vue"], "vue"),
    (&["svelte"], "svelte"),
    (&["astro"], "astro"),
    (&["wasm", "wat"], "wasm"),
    (&["gradle"], "gradle"),
    (&["cmake"], "cmake"),
    (&["dockerfile"], "docker"),
    (&["env"], "dotenv"),
    (&["lock", "ini", "conf", "cfg", "properties"], "config"),
    (
        &["exe", "dll", "so", "dylib", "a", "o", "bin", "class"],
        "binary",
    ),
];

/// The name of the picture of the file called `name`.
pub(super) fn of(name: &str) -> &'static str {
    let lower = name.to_lowercase();
    if let Some((_, icon)) = NAMES.iter().find(|(whole, _)| *whole == lower) {
        return icon;
    }
    if let Some((_, icon)) = STARTS.iter().find(|(start, _)| lower.starts_with(start)) {
        return icon;
    }
    // A name that is all extension (`.zshrc`) has none.
    let kind =
        (lower.rsplit_once('.')).map_or("", |(stem, kind)| if stem.is_empty() { "" } else { kind });
    (KINDS.iter())
        .find(|(kinds, _)| kinds.contains(&kind))
        .map_or(PLAIN, |(_, icon)| icon)
}

/// The call that gives the side every picture, each drawing whole as text: the page reads one
/// as a drawing the first time a row names it, and none of them as script.
pub(super) fn script() -> &'static str {
    static SCRIPT: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    SCRIPT.get_or_init(|| {
        let pictures: serde_json::Map<String, serde_json::Value> = (ICONS.iter())
            .map(|(name, svg)| ((*name).to_string(), svg.trim().into()))
            .collect();
        format!("FI({})", serde_json::Value::Object(pictures))
    })
}

#[cfg(test)]
#[path = "file_icons_tests.rs"]
mod tests;
