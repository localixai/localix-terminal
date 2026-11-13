//! Tests of `editor::language`: a file's language is told from its name, then from the program
//! its first line names, and only Markdown is drawn to read.

use super::*;

fn id(name: &str, text: &str) -> Option<&'static str> {
    language(name, text).map(|language| language.id)
}

#[test]
fn a_files_language_is_told_from_its_name() {
    assert_eq!(id("main.rs", ""), Some("rust"));
    assert_eq!(id("App.TSX", ""), Some("tsx"));
    assert_eq!(id("Cargo.lock", ""), Some("toml"));
    assert_eq!(id("Dockerfile", ""), Some("dockerfile"));
    assert_eq!(id("Dockerfile.dev", ""), Some("dockerfile"));
    assert_eq!(id(".env.local", ""), Some("properties"));
    assert_eq!(id("build.gradle.kts", ""), Some("kotlin"));
    assert_eq!(id("LICENSE", ""), None);
    assert_eq!(id("photo.png", ""), None);
    assert!(wraps("README.md") && wraps("notes.TXT") && !wraps("main.rs"));
}

#[test]
fn a_script_with_no_ending_is_told_from_the_program_its_first_line_names() {
    assert_eq!(id("cf", "#!/bin/sh\nexec true\n"), Some("shell"));
    assert_eq!(id("bootstrap", "#!/usr/bin/env bash\n"), Some("shell"));
    assert_eq!(id("lab-ci", "#! /bin/zsh -f\n"), Some("shell"));
    assert_eq!(id("serve", "#!/usr/bin/env python3\n"), Some("python"));
    assert_eq!(id("serve", "#!/usr/local/bin/python3.12\n"), Some("python"));
    assert_eq!(
        id("shot", "#!/usr/bin/env -S node --no-warnings\n"),
        Some("javascript")
    );
    assert_eq!(
        id("build", "#!/usr/bin/env FOO=1 bun\n"),
        Some("typescript")
    );
    assert_eq!(id("rake-it", "#!/usr/bin/ruby\r\nputs 1\r\n"), Some("ruby"));
    // A script's language is held in a script the page can run.
    let shell = language("cf", "#!/bin/sh\n").unwrap();
    assert_eq!(shell.scripts, ["modes"]);
}

#[test]
fn the_first_line_decides_only_where_the_name_says_nothing() {
    assert_eq!(id("main.rs", "#!/bin/sh\n"), Some("rust"));
    assert_eq!(id("notes.txt", "#!/bin/sh\n"), Some("shell"));
    assert_eq!(id("LICENSE", "MIT License\n#!/bin/sh\n"), None);
    assert_eq!(id("tool", "#!/usr/bin/perl\n"), None);
    assert_eq!(id("tool", "#!/usr/bin/env\n"), None);
    assert_eq!(id("tool", "#!"), None);
    assert_eq!(id("tool", ""), None);
}

#[test]
fn only_markdown_is_drawn_to_read() {
    assert!(reads("README.md") && reads("notes.MARKDOWN"));
    // An MDX file is code as much as prose: it stays text.
    assert!(!reads("page.mdx") && !reads("notes.txt") && !reads("main.rs") && !reads("md"));
}

#[test]
fn every_language_told_is_held_in_scripts_that_exist() {
    for (_, id) in BY_END.iter().chain(BY_NAME).chain(BY_PROGRAM) {
        let held = (HELD_IN.iter())
            .find_map(|(ids, scripts)| ids.contains(id).then_some(*scripts))
            .unwrap_or_else(|| std::slice::from_ref(id));
        for script in held {
            let there = SCRIPTS.iter().any(|known| known.name == *script);
            assert!(there, "{id}: no script {script}");
        }
    }
}
