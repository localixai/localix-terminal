//! The frontmatter of a Markdown file (a skill's `SKILL.md`), read as far as a listing needs:
//! the `key: value` lines between two `---`, a value plain or quoted, or a block that follows
//! its key folded (`>`) or kept line by line (`|`). Anything else in it is passed over.

use std::collections::BTreeMap;

/// The top-level keys of `text`'s frontmatter and their values; none when it has none.
pub(super) fn read(text: &str) -> BTreeMap<String, String> {
    let mut keys = BTreeMap::new();
    let mut lines = text.trim_start_matches('\u{feff}').lines();
    if lines.next().map(str::trim_end) != Some("---") {
        return keys;
    }
    let lines: Vec<&str> = lines
        .take_while(|line| !matches!(line.trim_end(), "---" | "..."))
        .collect();
    let mut at = 0;
    while let Some(line) = lines.get(at) {
        at += 1;
        let Some((key, value)) = top_level(line) else {
            continue;
        };
        let more = lines.get(at..).unwrap_or_default();
        let held = more
            .iter()
            .take_while(|line| line.trim().is_empty() || line.starts_with([' ', '\t']))
            .count();
        let block = more.get(..held).unwrap_or_default();
        at += held;
        keys.insert(key.to_string(), value_of(value, block));
    }
    keys
}

/// A line's key and the rest after its colon, when it is a key of the top level.
fn top_level(line: &str) -> Option<(&str, &str)> {
    if line.starts_with([' ', '\t', '#', '-']) {
        return None;
    }
    let (key, value) = line.split_once(':')?;
    let key = key.trim();
    let named = !key.is_empty()
        && (key.chars()).all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
    named.then_some((key, value.trim()))
}

/// The value written as `first` after its key, with the indented `block` that follows it.
fn value_of(first: &str, block: &[&str]) -> String {
    let lines = block.iter().map(|line| line.trim());
    match first.chars().next() {
        Some('|') => {
            let kept: Vec<&str> = lines.collect();
            kept.join("\n").trim_end().to_string()
        }
        Some('>') => folded(lines),
        Some(quote @ ('"' | '\'')) => {
            let whole = folded(std::iter::once(first).chain(lines));
            unquoted(&whole, quote)
        }
        Some(_) | None => folded(std::iter::once(first).chain(lines)),
    }
}

/// Lines folded into one: each joined to the next by a space, an empty one a new line.
fn folded<'a>(lines: impl Iterator<Item = &'a str>) -> String {
    let mut out = String::new();
    for line in lines {
        if line.is_empty() {
            out.push('\n');
        } else {
            if !out.is_empty() && !out.ends_with('\n') {
                out.push(' ');
            }
            out.push_str(line);
        }
    }
    out.trim().to_string()
}

/// `text` without the `quote` it opens with and the one that closes it, its escapes read: in
/// double quotes a backslash's, in single quotes a doubled quote.
fn unquoted(text: &str, quote: char) -> String {
    let inner = text.strip_prefix(quote).unwrap_or(text);
    let mut out = String::new();
    let mut chars = inner.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' if quote == '"' => match chars.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some(other) => out.push(other),
                None => {}
            },
            c if c == quote && quote == '\'' && chars.peek() == Some(&'\'') => {
                out.push('\'');
                chars.next();
            }
            c if c == quote => break,
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
#[path = "frontmatter_tests.rs"]
mod tests;
