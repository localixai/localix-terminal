//! The language a file is written in, as the editor's page colours it: told from the file's
//! name, and where the name says nothing from the program its first line names (`#!/bin/sh`).
//! Also which files wrap their long lines, and which the page draws as a document to read.

use super::vendored::SCRIPTS;

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

/// Languages by the program a script's first line asks for (`#!/usr/bin/env python3`), its
/// version left off.
const BY_PROGRAM: &[(&[&str], &str)] = &[
    (
        &["sh", "bash", "zsh", "fish", "dash", "ksh", "ash"],
        "shell",
    ),
    (&["python", "pypy"], "python"),
    (&["node", "nodejs"], "javascript"),
    (&["bun", "deno", "tsx", "ts-node"], "typescript"),
    (&["ruby"], "ruby"),
    (&["php"], "php"),
    (&["lua", "luajit"], "lua"),
    (&["swift"], "swift"),
];

fn known(table: &[(&[&str], &'static str)], word: &str) -> Option<&'static str> {
    (table.iter()).find_map(|(words, id)| words.contains(&word).then_some(*id))
}

/// The language the name `name`, in lower case, tells.
fn named(name: &str) -> Option<&'static str> {
    known(BY_NAME, name).or_else(|| {
        // `.env.local` is an `.env`, `Dockerfile.dev` a Dockerfile.
        let (start, end) = name.rsplit_once('.')?;
        known(BY_END, end).or_else(|| known(BY_NAME, start))
    })
}

/// The language of the program that runs `text`, a script: the one its first line names after
/// `#!`, straight or through `env` (whose own flags and variables are passed over).
fn run_by(text: &str) -> Option<&'static str> {
    let line = text.strip_prefix("#!")?.lines().next()?;
    let mut words = line.split_whitespace().map(|word| {
        let program = word.rsplit('/').next().unwrap_or(word);
        program.trim_end_matches(|c: char| c.is_ascii_digit() || c == '.')
    });
    let program = match words.next()? {
        "env" => words.find(|word| !word.starts_with('-') && !word.contains('='))?,
        program => program,
    };
    known(BY_PROGRAM, &program.to_ascii_lowercase())
}

/// The language of the file named `name` that holds `text`; none where its words are not
/// coloured. The name decides; a name that says nothing leaves it to the text's first line.
pub fn language(name: &str, text: &str) -> Option<Language> {
    let id = named(&name.to_ascii_lowercase()).or_else(|| run_by(text))?;
    let scripts = (HELD_IN.iter())
        .find_map(|(ids, scripts)| ids.contains(&id).then_some(*scripts))
        .or_else(|| {
            let own = SCRIPTS.iter().find(|script| script.name == id)?;
            Some(std::slice::from_ref(&own.name))
        })?;
    Some(Language { id, scripts })
}

/// Whether the file named `name` is prose: its long lines wrap at the pane's edge.
pub(super) fn wraps(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    (name.rsplit_once('.')).is_some_and(|(_, end)| ["md", "markdown", "mdx", "txt"].contains(&end))
}

/// Whether the file named `name` is one the page draws as a document to read (Markdown), with
/// its text a click away.
pub fn reads(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    (name.rsplit_once('.')).is_some_and(|(_, end)| ["md", "markdown"].contains(&end))
}

#[cfg(test)]
#[path = "language_tests.rs"]
mod tests;
