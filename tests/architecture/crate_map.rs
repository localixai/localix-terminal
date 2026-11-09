//! The crate's module tree as the compiler finds it, from `src/lib.rs` through every `mod x;`
//! (with its `#[path]`): each file is in the top-level module whose tree declares it, whatever
//! its name. What a file's paths name is `named.rs`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use super::lex::{Token, tokens};

/// One source file of the library, where the module tree puts it.
pub(crate) struct SourceFile {
    /// Its path under `src/`, as a failure names it.
    pub(crate) name: String,
    /// The top-level module it belongs to (`browser` for `src/browser/host_keys.rs`).
    pub(crate) top: String,
    /// It is compiled for tests only (declared under `#[cfg(test)]`, or within such a module).
    pub(crate) test_only: bool,
    /// The inline test modules it holds (`mod tests { … }`), by name.
    pub(crate) inline_tests: Vec<String>,
    pub(crate) tokens: Vec<Token>,
    /// How many modules deep it is: 1 for `src/board.rs`, 2 for `src/board/model.rs`.
    pub(crate) depth: usize,
}

/// The library's files and its top-level modules (those compiled outside tests).
pub(crate) struct CrateMap {
    pub(crate) files: Vec<SourceFile>,
    pub(crate) modules: BTreeSet<String>,
}

/// How a file is reached: under `#[cfg(test)]` or not, and whether the compiler reads it as a
/// directory's own (the crate's root, a file reached by `#[path]`), whose `mod x;` is `x.rs`
/// beside it, where another file's is in the directory of its name (`board/x.rs`).
#[derive(Clone, Copy)]
struct Place {
    test_only: bool,
    beside: bool,
}

/// A `mod x;` (or `mod x {`) a file declares.
struct Declared {
    name: String,
    path: Option<String>,
    test_only: bool,
    /// The inline modules it sits in, outermost first.
    within: Vec<String>,
    inline: bool,
}

impl CrateMap {
    /// The tree from `src/lib.rs`; an error names a declared module whose file is not there.
    pub(crate) fn read(source: &Path) -> Result<Self, String> {
        let mut map = Self {
            files: vec![],
            modules: BTreeSet::new(),
        };
        let tokens = tokens(&read(&source.join("lib.rs"))?);
        for declared in declarations(&tokens) {
            if declared.inline || !declared.within.is_empty() {
                continue;
            }
            let file = resolve(source, source, &declared)?;
            if !declared.test_only {
                map.modules.insert(declared.name.clone());
            }
            let place = Place {
                test_only: declared.test_only,
                beside: declared.path.is_some(),
            };
            map.walk(source, &file, (&declared.name, 1), place)?;
        }
        Ok(map)
    }

    /// `file` and the modules it declares, in the top-level module `top`, `depth` modules deep.
    fn walk(
        &mut self,
        source: &Path,
        file: &Path,
        (top, depth): (&str, usize),
        place: Place,
    ) -> Result<(), String> {
        let tokens = tokens(&read(file)?);
        let directory = file.parent().unwrap_or(source);
        let stem = (file.file_stem())
            .and_then(|stem| stem.to_str())
            .unwrap_or_default();
        let owned = if place.beside {
            directory.to_path_buf()
        } else {
            directory.join(stem)
        };
        let mut inline_tests = vec![];
        for declared in declarations(&tokens) {
            if declared.inline {
                if declared.test_only || declared.name == "tests" {
                    inline_tests.push(declared.name);
                }
                continue;
            }
            let base = match declared.path {
                Some(_) => directory.to_path_buf(),
                None => owned.clone(),
            };
            let base = (declared.within.iter()).fold(base, |base, name| base.join(name));
            let child = resolve(source, &base, &declared)?;
            let child_place = Place {
                test_only: place.test_only || declared.test_only,
                beside: declared.path.is_some(),
            };
            let deeper = depth + 1 + declared.within.len();
            self.walk(source, &child, (top, deeper), child_place)?;
        }
        self.files.push(SourceFile {
            name: file
                .strip_prefix(source)
                .unwrap_or(file)
                .display()
                .to_string(),
            top: top.to_string(),
            test_only: place.test_only,
            inline_tests,
            tokens,
            depth,
        });
        Ok(())
    }

    /// Each top-level module's working (not test-only) dependencies on the others, with the
    /// first file that names each.
    pub(crate) fn graph(&self) -> BTreeMap<(String, String), String> {
        let mut edges = BTreeMap::new();
        for file in self.files.iter().filter(|file| !file.test_only) {
            for named in file.named(&self.modules) {
                if named != file.top {
                    let edge = (file.top.clone(), named);
                    edges.entry(edge).or_insert_with(|| file.name.clone());
                }
            }
        }
        edges
    }
}

/// Every `mod` a file declares, with its `#[path]` and whether it is under `#[cfg(test)]`.
fn declarations(tokens: &[Token]) -> Vec<Declared> {
    let mut found = vec![];
    let mut attributes: Vec<&[Token]> = vec![];
    let mut within: Vec<(String, usize)> = vec![];
    let mut braces = 0usize;
    let mut at = 0;
    while let Some(token) = tokens.get(at) {
        let rest = tokens.get(at..).unwrap_or_default();
        if token.is_punct("#") && rest.get(1).is_some_and(|next| next.is_punct("[")) {
            let length = group_length(rest.get(1..).unwrap_or_default());
            attributes.push(rest.get(2..length).unwrap_or_default());
            at += 1 + length;
            continue;
        }
        if token.is("pub") {
            let scoped = rest.get(1).is_some_and(|next| next.is_punct("("));
            at += 1 + if scoped {
                group_length(rest.get(1..).unwrap_or_default())
            } else {
                0
            };
            continue;
        }
        if let (true, Some(Token::Word(name)), Some(after)) =
            (token.is("mod"), rest.get(1), rest.get(2))
        {
            let inline = after.is_punct("{");
            found.push(Declared {
                name: name.clone(),
                path: path_of(&attributes),
                test_only: attributes.iter().any(|attribute| for_tests(attribute)),
                within: within.iter().map(|(name, _)| name.clone()).collect(),
                inline,
            });
            if inline {
                within.push((name.clone(), braces));
            }
        }
        if token.is_punct("{") {
            braces += 1;
        } else if token.is_punct("}") {
            braces = braces.saturating_sub(1);
            if within.last().is_some_and(|(_, opened)| *opened == braces) {
                within.pop();
            }
        }
        attributes.clear();
        at += 1;
    }
    found
}

/// How many tokens the bracketed group starting at `tokens` spans, both brackets included.
fn group_length(tokens: &[Token]) -> usize {
    let mut depth = 0usize;
    for (at, token) in tokens.iter().enumerate() {
        if token.is_punct("[") || token.is_punct("(") || token.is_punct("{") {
            depth += 1;
        } else if token.is_punct("]") || token.is_punct(")") || token.is_punct("}") {
            depth = depth.saturating_sub(1);
            if depth == 0 {
                return at + 1;
            }
        }
    }
    tokens.len()
}

fn path_of(attributes: &[&[Token]]) -> Option<String> {
    attributes.iter().find_map(|attribute| match attribute {
        [path, equals, Token::Str(file)] if path.is("path") && equals.is_punct("=") => {
            Some(file.clone())
        }
        _ => None,
    })
}

/// `#[cfg(test)]`, or a `cfg` that needs `test` (never one that excludes it).
fn for_tests(attribute: &[Token]) -> bool {
    attribute.first().is_some_and(|cfg| cfg.is("cfg"))
        && attribute.iter().any(|token| token.is("test"))
        && !attribute.iter().any(|token| token.is("not"))
}

fn resolve(source: &Path, base: &Path, declared: &Declared) -> Result<PathBuf, String> {
    let file = match &declared.path {
        Some(path) => base.join(path),
        None => base.join(format!("{}.rs", declared.name)),
    };
    if file.is_file() {
        Ok(file)
    } else {
        let at = file.strip_prefix(source).unwrap_or(&file).display();
        Err(format!(
            "{at}: declared as `mod {}`, not found",
            declared.name
        ))
    }
}

fn read(file: &Path) -> Result<String, String> {
    std::fs::read_to_string(file).map_err(|error| format!("{}: {error}", file.display()))
}
