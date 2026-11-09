//! Executable source-layout rules: small documented modules, the module map's dependencies
//! between top-level modules (ARCHITECTURE.md), no cycle among them, tests beside the code, and
//! no async runtime.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

#[path = "architecture/crate_map.rs"]
mod crate_map;
#[path = "architecture/lex.rs"]
mod lex;
#[path = "architecture/module_map.rs"]
mod module_map;
#[path = "architecture/named.rs"]
mod named;

use crate_map::{CrateMap, SourceFile};
use module_map::{ALLOWED, EXCEPTIONS};

fn source() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

fn crate_map() -> Result<CrateMap, String> {
    CrateMap::read(&source()).map_err(|error| format!("the module tree: {error}"))
}

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
    let source = source();
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
fn every_source_file_is_in_the_module_tree() {
    // The rules below read the tree from `lib.rs`; a file it does not reach would go unchecked.
    let source = source();
    let reached: BTreeSet<String> = crate_map()
        .unwrap()
        .files
        .into_iter()
        .map(|file| file.name)
        .collect();
    let mut files = vec![];
    rust_files(&source, &mut files).unwrap();
    let mut missing = vec![];
    for path in files {
        let name = path.strip_prefix(&source).unwrap().display().to_string();
        if !reached.contains(&name) && name != "lib.rs" && name != "main.rs" {
            missing.push(format!("{name}: no module of the crate declares it"));
        }
    }
    assert!(missing.is_empty(), "{}", missing.join("\n"));
}

#[test]
fn top_level_modules_depend_only_as_the_module_map_allows() {
    let map = crate_map().unwrap();
    let allowed: BTreeMap<&str, &[&str]> = ALLOWED.iter().copied().collect();
    let mut broken = vec![];
    for module in &map.modules {
        if !allowed.contains_key(module.as_str()) {
            broken.push(format!(
                "{module}: a top-level module with no row in ALLOWED"
            ));
        }
    }
    let graph = map.graph();
    for ((from, to), file) in &graph {
        let excepted = EXCEPTIONS.iter().any(|&(f, t, _)| f == from && t == to);
        let drawn = allowed
            .get(from.as_str())
            .is_some_and(|uses| uses.contains(&to.as_str()));
        if !drawn && !excepted {
            broken.push(format!(
                "{file}: `{from}` must not depend on `{to}` (the module map, ALLOWED)"
            ));
        }
    }
    for &(from, to, reason) in EXCEPTIONS {
        if !graph.contains_key(&(from.to_string(), to.to_string())) {
            broken.push(format!(
                "EXCEPTIONS: `{from}` no longer uses `{to}` ({reason}): drop it"
            ));
        }
    }
    assert!(broken.is_empty(), "{}", broken.join("\n"));
}

#[test]
fn top_level_modules_form_no_cycle() {
    let graph = crate_map().unwrap().graph();
    let mut uses: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for (from, to) in graph.keys() {
        uses.entry(from).or_default().push(to);
    }
    // A depth-first walk: a module met again while still on the path closes a cycle.
    fn walk<'a>(
        at: &'a str,
        uses: &BTreeMap<&'a str, Vec<&'a str>>,
        path: &mut Vec<&'a str>,
        done: &mut BTreeSet<&'a str>,
    ) -> Option<Vec<&'a str>> {
        if let Some(start) = path.iter().position(|&on| on == at) {
            let mut cycle = path[start..].to_vec();
            cycle.push(at);
            return Some(cycle);
        }
        if !done.insert(at) {
            return None;
        }
        path.push(at);
        for &next in uses.get(at).into_iter().flatten() {
            if let Some(cycle) = walk(next, uses, path, done) {
                return Some(cycle);
            }
        }
        path.pop();
        None
    }
    let mut done = BTreeSet::new();
    for &module in uses.keys() {
        if let Some(cycle) = walk(module, &uses, &mut vec![], &mut done) {
            let files: Vec<String> = (cycle.windows(2))
                .map(|pair| graph[&(pair[0].to_string(), pair[1].to_string())].clone())
                .collect();
            panic!(
                "{}: the top-level modules form a cycle, {}",
                files.join(", "),
                cycle.join(" -> ")
            );
        }
    }
}

#[test]
fn tests_sit_beside_the_code_never_inline() {
    let mut broken = vec![];
    for file in crate_map()
        .unwrap()
        .files
        .iter()
        .filter(|file| !file.test_only)
    {
        for name in &file.inline_tests {
            broken.push(format!(
                "{}: `mod {name} {{ … }}` inline; tests sit in a sibling `x_tests.rs` \
                 (`#[cfg(test)] #[path = \"x_tests.rs\"] mod tests;`)",
                file.name
            ));
        }
    }
    assert!(broken.is_empty(), "{}", broken.join("\n"));
}

#[test]
fn no_async_runtime_threads_and_channels_instead() {
    const RUNTIMES: [&str; 6] = [
        "tokio",
        "async-std",
        "smol",
        "futures",
        "async-executor",
        "actix-rt",
    ];
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let manifest = std::fs::read_to_string(manifest).unwrap();
    let mut broken = vec![];
    let mut in_dependencies = false;
    for line in manifest.lines().map(str::trim) {
        if line.starts_with('[') {
            in_dependencies = line.contains("dependencies");
        } else if let Some((name, _)) = line.split_once('=').filter(|_| in_dependencies)
            && RUNTIMES.contains(&name.trim())
        {
            broken.push(format!("Cargo.toml: `{}` is an async runtime", name.trim()));
        }
    }
    for file in &crate_map().unwrap().files {
        let mut words = file.tokens.iter().filter_map(lex::Token::word);
        if let Some(word) = words.find(|&word| word == "async" || word == "await") {
            broken.push(format!(
                "{}: `{word}`, but no async runtime: the design is threads and channels",
                file.name
            ));
        }
    }
    assert!(broken.is_empty(), "{}", broken.join("\n"));
}

#[test]
fn the_reader_sees_grouped_nested_and_climbing_paths_but_not_comments_or_strings() {
    let text = "use crate::{board, layout::{tree, Tab}, self as _};\n\
                fn f() { crate::view::go(); super::super::feed::x(); super::own(); }\n\
                // crate::browser::Browser\n\
                /* crate::desk /* nested */ crate::server */\n\
                const S: &str = \"crate::cli\"; const R: &str = r#\"crate::mcp\"#;\n\
                fn g<'a>(c: char) -> bool { c == '{' }\n\
                mod native { fn h() { super::super::super::store::open(); } }";
    let file = SourceFile {
        name: "x/y.rs".into(),
        top: "x".into(),
        test_only: false,
        inline_tests: vec![],
        tokens: lex::tokens(text),
        depth: 2,
    };
    let modules = [
        "board", "browser", "cli", "desk", "feed", "layout", "mcp", "server", "store",
    ];
    let modules: BTreeSet<String> = modules.into_iter().map(String::from).collect();
    let named: Vec<String> = file.named(&modules).into_iter().collect();
    assert_eq!(named, ["board", "feed", "layout", "store"]);
}

#[test]
fn every_web_view_takes_the_first_click_of_an_inactive_window() {
    // A window that is not the key one (a panel, a window behind another app) would
    // spend its first click on waking up, and the person would click twice. Every page the app
    // builds says it takes that click.
    let source = source();
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
