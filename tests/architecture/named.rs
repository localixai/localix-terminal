//! The top-level modules a source file's paths name: `crate::x`, `$crate::x`, a group
//! (`crate::{x, y::{z, w}}`), and `super::…` that climbs as many modules as the file is deep, to
//! the crate's root. Paths in comments and strings are not code, and the tokens have none.

use std::collections::BTreeSet;

use super::crate_map::SourceFile;
use super::lex::Token;

impl SourceFile {
    /// The top-level modules among `modules` this file's paths name.
    pub(crate) fn named(&self, modules: &BTreeSet<String>) -> BTreeSet<String> {
        let mut named = BTreeSet::new();
        // The brace depth at which each inline module (`mod native { … }`) opened.
        let mut inline = Vec::<usize>::new();
        let mut braces = 0usize;
        for (at, token) in self.tokens.iter().enumerate() {
            let rest = self.tokens.get(at..).unwrap_or_default();
            if token.is_punct("{") {
                braces += 1;
            } else if token.is_punct("}") {
                braces = braces.saturating_sub(1);
                if inline.last() == Some(&braces) {
                    inline.pop();
                }
            } else if token.is("mod") && rest.get(2).is_some_and(|next| next.is_punct("{")) {
                inline.push(braces);
            }
            let Some(after) = climb(rest, self.depth + inline.len()) else {
                continue;
            };
            let heads = heads(rest.get(after..).unwrap_or_default());
            named.extend(heads.into_iter().filter(|head| modules.contains(head)));
        }
        named
    }
}

/// Where the path starting at `tokens` reaches the crate's root: past `crate::` (also as
/// `$crate::`), or past as many `super::` as the module is deep; `None` for any other path.
fn climb(tokens: &[Token], depth: usize) -> Option<usize> {
    let first = tokens.first()?;
    if first.is("crate") {
        return tokens.get(1).filter(|next| next.is_punct("::")).map(|_| 2);
    }
    if !first.is("super") {
        return None;
    }
    let supers = (tokens.chunks(2))
        .take_while(
            |pair| matches!(pair, [word, punct] if word.is("super") && punct.is_punct("::")),
        )
        .count();
    (supers == depth).then_some(supers * 2)
}

/// The first segment of a path after its root, or of each element of its group (`{x, y::z}`).
fn heads(tokens: &[Token]) -> Vec<String> {
    match tokens.first() {
        Some(Token::Word(word)) => vec![word.clone()],
        Some(first) if first.is_punct("{") => {
            let mut heads = vec![];
            let mut depth = 0usize;
            let mut starts = true;
            for token in tokens {
                if token.is_punct("{") {
                    depth += 1;
                    starts = depth == 1;
                    continue;
                } else if token.is_punct("}") {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        break;
                    }
                } else if depth == 1 && token.is_punct(",") {
                    starts = true;
                    continue;
                } else if depth == 1 && starts {
                    heads.extend(token.word().map(str::to_string));
                }
                starts = false;
            }
            heads
        }
        Some(_) | None => vec![],
    }
}
