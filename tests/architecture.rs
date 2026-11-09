//! Executable source-layout rules: small documented modules and independent domain models.

use std::path::{Path, PathBuf};

fn rust_files(directory: &Path, files: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(directory)? {
        let path = entry?.path();
        if path.is_dir() {
            rust_files(&path, files)?;
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
    Ok(())
}

#[test]
fn source_modules_are_small_documented_and_named_for_their_responsibility() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = vec![];
    rust_files(&source, &mut files).unwrap();
    files.sort();
    assert!(!files.is_empty());
    let mut broken = vec![];
    for path in files {
        let name = path.strip_prefix(&source).unwrap().display().to_string();
        let text = std::fs::read_to_string(&path).unwrap();
        if !text.starts_with("//!") {
            broken.push(format!("{name}: start with a //! responsibility"));
        }
        if path.file_name().is_some_and(|name| name == "mod.rs") {
            broken.push(format!("{name}: name the module for its responsibility"));
        }
        // Blank lines and whole-line comments are not working code. String literals, including
        // SQL migrations and embedded scripts, count: their size also makes a module harder to read.
        let working = text
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with("//"))
            .count();
        if working > 300 {
            broken.push(format!("{name}: {working} working lines, at most 300"));
        }
    }
    assert!(broken.is_empty(), "{}", broken.join("\n"));
}

#[test]
fn domain_models_do_not_depend_on_application_or_ui_adapters() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    for module in [
        "board.rs",
        "board/model.rs",
        "board/cards.rs",
        "layout/tree.rs",
    ] {
        let text = std::fs::read_to_string(root.join(module)).unwrap();
        for adapter in ["browser", "desk", "server", "cli", "mcp"] {
            assert!(
                !text.contains(&format!("crate::{adapter}::")),
                "{module} must not depend on the {adapter} adapter"
            );
        }
    }
}

#[test]
fn every_web_view_takes_the_first_click_of_an_inactive_window() {
    // A window that is not the key one (a panel, a window behind another app) would
    // spend its first click on waking up, and the person would click twice. Every page the app
    // builds says it takes that click.
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = vec![];
    rust_files(&source, &mut files).unwrap();
    let mut missing = vec![];
    for path in files {
        let text = std::fs::read_to_string(&path).unwrap();
        if text.contains("WebViewBuilder::new()") && !text.contains("with_accept_first_mouse(true)")
        {
            missing.push(path.strip_prefix(&source).unwrap().display().to_string());
        }
    }
    assert!(missing.is_empty(), "{missing:?}");
}

#[test]
fn the_window_is_told_the_ledgers_and_the_projects_only_by_its_feed_consumer() {
    // A write of a project's history, or of the projects the desk knows, records a notice of the
    // feed; the window's consumer alone tells the window. A write that told it by hand could be
    // forgotten at the next write, and leave the history menu or the switcher stale.
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = vec![];
    rust_files(&source, &mut files).unwrap();
    let consumer = source.join("desk").join("layout_drawn.rs");
    let mut telling = vec![];
    for path in files.iter().filter(|path| **path != consumer) {
        let text = std::fs::read_to_string(path).unwrap();
        for (at, line) in text.lines().enumerate() {
            let code = line.trim_start();
            if code.starts_with("//") {
                continue;
            }
            if [".tell_ledger(", ".tell_places("]
                .iter()
                .any(|call| code.contains(call))
            {
                let name = path.strip_prefix(&source).unwrap().display();
                telling.push(format!("{name}:{}", at + 1));
            }
        }
    }
    assert!(telling.is_empty(), "told by hand: {telling:?}");
}
