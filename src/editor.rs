//! A file edited in a tab. The page is an editor core and one language's grammar, both vendored
//! and run by their SHA-384s (`vendored.rs`), with the page's own script (`page.js`): a page
//! runs the core and the language of the file it shows (`language.rs`), never the others. A
//! Markdown file's page also runs what draws it as a document to read, and opens showing
//! that. The text is the page's while the person edits it; the desk opens the file into it,
//! tells it what the disk did, and takes what it saves and what is left unsaved.

mod language;
mod vendored;

use serde_json::{Value, json};

use language::wraps;
pub use language::{Language, language, reads};
use vendored::{SCRIPTS, Script};

/// The page's own script, and its SHA-384: the page's policy runs it by that hash.
const GLUE: &str = include_str!("editor/page.js");
const GLUE_SHA384: &str = "4ARKJHoi3RfCenlBibuEJ4l0Z+NTQbQ4aVm8mPRj3tZGq9EDfoD1XGBAs35eLZB1";

fn vendored(name: &str) -> Option<&'static Script> {
    SCRIPTS.iter().find(|script| script.name == name)
}

/// The vendored script that draws a file as a document to read.
const PREVIEW: &str = "preview";

/// The page that edits a file titled `title`, with the grammar of `language`; `read`, it also
/// runs what draws the file as a document.
pub fn page(title: &str, language: Option<Language>, read: bool) -> String {
    let title = crate::page::escaped(title);
    let held = language.map_or(&[][..], |language| language.scripts);
    let run: Vec<&Script> = (std::iter::once("core").chain(held.iter().copied()))
        .chain(read.then_some(PREVIEW))
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
        "lang": language(file.name, file.text).map(|language| language.id),
        "wrap": wraps(file.name),
        "read": reads(file.name),
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
    /// A link of the document, followed: the address it names, as the document wrote it.
    Follow(String),
}

/// What `posted`, a message of the page, says the person did; none when it says nothing known.
pub fn did(posted: &str) -> Option<Did> {
    let mut said: Value = serde_json::from_str(posted).ok()?;
    let text = |said: &mut Value, named: &str| match said[named].take() {
        Value::String(text) => Some(text),
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::Array(_) | Value::Object(_) => {
            None
        }
    };
    match said["t"].as_str()? {
        "save" => text(&mut said, "text").map(Did::Save),
        "draft" => text(&mut said, "text").map(Did::Draft),
        "clean" => Some(Did::Clean),
        "follow" => text(&mut said, "href").map(Did::Follow),
        _ => None,
    }
}

#[cfg(test)]
#[path = "editor_tests.rs"]
mod tests;
