//! A file edited in a tab. The page is an editor core and one language's grammar, both vendored
//! and run by their SHA-384s (`vendored.rs`), with the page's own script (`page.js`): a page
//! runs the core and the language of the file it shows, never the others. The text is the
//! page's while the person edits it; the desk opens the file into it, tells it what the disk
//! did, and takes what it saves and what is left unsaved.

mod vendored;

use serde_json::{Value, json};

use vendored::{SCRIPTS, Script};

/// The page's own script, and its SHA-384: the page's policy runs it by that hash.
const GLUE: &str = include_str!("editor/page.js");
const GLUE_SHA384: &str = "PZ36sCcqd99zQDVYR52fbRyfeNZWi6NwmPQQcr0WME3r2Lnw0YdWOYvLtVnLMw36";

/// The language a file is written in, as the page names it, and the vendored scripts that
/// hold it, in the order a page runs them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Language {
    pub id: &'static str,
    pub scripts: &'static [&'static str],
}

/// Languages by the end of a file's name, in lower case.
const BY_END: &[(&[&str], &str)] = &[
    (&["js", "mjs", "cjs"], "javascript"),
    (&["jsx"], "jsx"),
    (&["ts", "mts", "cts"], "typescript"),
    (&["tsx"], "tsx"),
    (&["json", "jsonc", "json5", "webmanifest", "map"], "json"),
    (&["html", "htm", "xhtml", "vue", "svelte"], "html"),
    (&["css", "scss", "less"], "css"),
    (&["md", "markdown", "mdx"], "markdown"),
    (&["rs"], "rust"),
    (&["py", "pyi", "pyw"], "python"),
    (&["go"], "go"),
    (
        &["c", "h", "cc", "cpp", "cxx", "hpp", "hh", "hxx", "m", "mm"],
        "cpp",
    ),
    (&["java"], "java"),
    (&["yml", "yaml"], "yaml"),
    (&["xml", "svg", "plist", "xsd", "xsl", "xslt"], "xml"),
    (&["sql"], "sql"),
    (&["php"], "php"),
    (&["sh", "bash", "zsh", "fish", "command"], "shell"),
    (&["toml", "lock"], "toml"),
    (&["rb", "gemspec", "rake"], "ruby"),
    (&["swift"], "swift"),
    (&["lua"], "lua"),
    (&["diff", "patch"], "diff"),
    (&["ini", "conf", "cfg", "properties", "env"], "properties"),
    (&["kt", "kts"], "kotlin"),
    (&["cs"], "csharp"),
    (&["scala", "sc"], "scala"),
    (&["dart"], "dart"),
];

/// Languages by a file's whole name, where its end says nothing.
const BY_NAME: &[(&[&str], &str)] = &[
    (&["dockerfile", "containerfile"], "dockerfile"),
    (&["gemfile", "rakefile", "podfile", "brewfile"], "ruby"),
    (
        &[
            ".bashrc",
            ".zshrc",
            ".profile",
            ".bash_profile",
            ".zprofile",
            ".zshenv",
        ],
        "shell",
    ),
    (
        &[".env", ".gitconfig", ".editorconfig", ".npmrc"],
        "properties",
    ),
];

/// The scripts that hold a language, where it is not one script named as it is.
const HELD_IN: &[(&[&str], &[&str])] = &[
    (&["javascript", "jsx", "typescript", "tsx"], &["javascript"]),
    (&["html"], &["javascript", "css", "html"]),
    (
        &[
            "shell",
            "toml",
            "dockerfile",
            "ruby",
            "swift",
            "lua",
            "diff",
            "properties",
            "kotlin",
            "csharp",
            "scala",
            "dart",
        ],
        &["modes"],
    ),
];

/// The language of the file named `name`; none where its words are not coloured.
pub fn language(name: &str) -> Option<Language> {
    let name = name.to_ascii_lowercase();
    let known = |table: &[(&[&str], &'static str)], word: &str| {
        (table.iter()).find_map(|(words, id)| words.contains(&word).then_some(*id))
    };
    let id = known(BY_NAME, &name).or_else(|| {
        // `.env.local` is an `.env`, `Dockerfile.dev` a Dockerfile.
        let (start, end) = name.rsplit_once('.')?;
        known(BY_END, end).or_else(|| known(BY_NAME, start))
    })?;
    let scripts = (HELD_IN.iter())
        .find_map(|(ids, scripts)| ids.contains(&id).then_some(*scripts))
        .or_else(|| {
            let own = SCRIPTS.iter().find(|script| script.name == id)?;
            Some(std::slice::from_ref(&own.name))
        })?;
    Some(Language { id, scripts })
}

/// Whether the file named `name` is prose: its long lines wrap at the pane's edge.
pub fn wraps(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    (name.rsplit_once('.')).is_some_and(|(_, end)| ["md", "markdown", "mdx", "txt"].contains(&end))
}

fn vendored(name: &str) -> Option<&'static Script> {
    SCRIPTS.iter().find(|script| script.name == name)
}

/// The page that edits a file titled `title`, with the grammar of `language`.
pub fn page(title: &str, language: Option<Language>) -> String {
    let title = crate::page::escaped(title);
    let held = language.map_or(&[][..], |language| language.scripts);
    let run: Vec<&Script> = (std::iter::once("core").chain(held.iter().copied()))
        .filter_map(vendored)
        .collect();
    let mut hashes: Vec<String> = (run.iter())
        .map(|script| format!("'sha384-{}'", script.sha384))
        .collect();
    hashes.push(format!("'sha384-{GLUE_SHA384}'"));
    let scripts = run.iter().fold(String::new(), |mut page, script| {
        page.push_str("<script>");
        page.push_str(script.text);
        page.push_str("</script>\n");
        page
    });
    // The scripts first: each later place stands before them in the page, so nothing a script
    // holds is read as one.
    include_str!("editor/page.html")
        .replacen("{{glue}}", GLUE, 1)
        .replacen("{{vendored}}", &scripts, 1)
        .replacen("{{scripts}}", &hashes.join(" "), 1)
        .replacen("{{title}}", &title, 1)
}

/// A file as the desk opens it in the page: its name, its text as the disk holds it, the text
/// the person left unsaved and whether the disk changed since, and the line to show.
#[derive(Debug, Default)]
pub struct Opened<'a> {
    pub name: &'a str,
    pub text: &'a str,
    pub draft: Option<&'a str>,
    pub moved: bool,
    pub line: Option<u32>,
}

/// The frame that opens a file in the page.
pub fn opened(file: &Opened<'_>) -> String {
    let told = json!({
        "name": file.name,
        "lang": language(file.name).map(|language| language.id),
        "wrap": wraps(file.name),
        "text": file.text,
        "draft": file.draft,
        "moved": file.moved,
        "line": file.line,
    });
    format!("O({told})")
}

/// The frame that shows line `line` of the file.
pub fn line(line: u32) -> String {
    format!("L({line})")
}

/// The frame that says the disk holds `text` now.
pub fn disk(text: &str) -> String {
    format!("D({})", json!({ "text": text }))
}

/// The frame that says the file went from the disk.
pub fn gone() -> String {
    "G()".to_string()
}

/// How a save ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Saved<'a> {
    Kept,
    /// The disk changed since the page was last told: nothing was written.
    Conflict,
    Failed(&'a str),
}

/// The frame that answers a save.
pub fn saved(how: Saved<'_>) -> String {
    let told = match how {
        Saved::Kept => json!({ "ok": true }),
        Saved::Conflict => json!({ "ok": false, "conflict": true }),
        Saved::Failed(why) => json!({ "ok": false, "why": why }),
    };
    format!("S({told})")
}

/// What the person did in the page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Did {
    /// The text to write.
    Save(String),
    /// The text as it stands, unsaved.
    Draft(String),
    /// The text is the disk's again.
    Clean,
}

/// What `posted`, a message of the page, says the person did; none when it says nothing known.
pub fn did(posted: &str) -> Option<Did> {
    let mut said: Value = serde_json::from_str(posted).ok()?;
    let text = |said: &mut Value| match said["text"].take() {
        Value::String(text) => Some(text),
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::Array(_) | Value::Object(_) => {
            None
        }
    };
    match said["t"].as_str()? {
        "save" => text(&mut said).map(Did::Save),
        "draft" => text(&mut said).map(Did::Draft),
        "clean" => Some(Did::Clean),
        _ => None,
    }
}

#[cfg(test)]
#[path = "editor_tests.rs"]
mod tests;
