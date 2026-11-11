//! Keys known by their shape (a GitHub token, a cloud key, a signed token, a private key), shown
//! to an agent as what they are, not as their value: what a program printed or a page shows is
//! read by a model, and a live key is not for it to read. The screen itself is not changed.

use std::borrow::Cow;

/// A kind of key: how it starts, what its body is made of, and its least length.
struct Shape {
    name: &'static str,
    starts: &'static [&'static str],
    body: fn(char) -> bool,
    least: usize,
}

fn word(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '-'
}

fn upper(c: char) -> bool {
    c.is_ascii_uppercase() || c.is_ascii_digit()
}

/// The more exact start first: the first shape that fits is the one named.
const SHAPES: [Shape; 9] = [
    Shape {
        name: "Stripe key",
        starts: &["sk_live_", "sk_test_", "rk_live_", "rk_test_"],
        body: |c| c.is_ascii_alphanumeric(),
        least: 20,
    },
    Shape {
        name: "API key",
        starts: &["sk-"],
        body: word,
        least: 20,
    },
    Shape {
        name: "Google API key",
        starts: &["AIza"],
        body: word,
        least: 35,
    },
    Shape {
        name: "AWS access key",
        starts: &["AKIA", "ASIA"],
        body: upper,
        least: 16,
    },
    Shape {
        name: "GitHub token",
        starts: &["ghp_", "gho_", "ghu_", "ghs_", "ghr_"],
        body: word,
        least: 36,
    },
    Shape {
        name: "GitHub token",
        starts: &["github_pat_"],
        body: word,
        least: 50,
    },
    Shape {
        name: "Slack token",
        starts: &["xoxb-", "xoxp-", "xoxa-", "xoxr-", "xoxs-", "xapp-"],
        body: word,
        least: 20,
    },
    Shape {
        name: "GitLab token",
        starts: &["glpat-"],
        body: word,
        least: 20,
    },
    Shape {
        name: "npm token",
        starts: &["npm_"],
        body: |c| c.is_ascii_alphanumeric(),
        least: 36,
    },
];

/// How a signed token starts (a JSON object, encoded), and the least length of each of its
/// three parts.
const SIGNED: &str = "eyJ";
const SIGNED_PART: usize = 10;
/// A private key's first and last lines begin and end so.
const KEY_BEGINS: &str = "-----BEGIN ";
const KEY_NAMED: &str = "PRIVATE KEY-----";
const KEY_ENDS: &str = "-----END ";

/// `text` with every key in it shown as `[hidden: its kind]`.
pub(super) fn hidden(text: &str) -> Cow<'_, str> {
    let may = SHAPES
        .iter()
        .flat_map(|shape| shape.starts)
        .chain(&[SIGNED, KEY_BEGINS]);
    if !may.into_iter().any(|start| text.contains(start)) {
        return Cow::Borrowed(text);
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    // A key starts where a word does: `task-sk-…` is a word that goes on.
    let mut inside = false;
    while let Some(c) = rest.chars().next() {
        if let Some((name, length)) = (!inside).then(|| key_at(rest)).flatten() {
            out.push_str("[hidden: ");
            out.push_str(name);
            out.push(']');
            rest = rest.get(length..).unwrap_or_default();
            inside = false;
            continue;
        }
        out.push(c);
        inside = word(c);
        rest = rest.get(c.len_utf8()..).unwrap_or_default();
    }
    Cow::Owned(out)
}

/// The key `rest` starts with: its kind and how many bytes it takes.
fn key_at(rest: &str) -> Option<(&'static str, usize)> {
    if rest.starts_with(KEY_BEGINS) {
        return private_key(rest);
    }
    if rest.starts_with(SIGNED) {
        let length = run(rest, |c| word(c) || c == '.');
        let token = rest.get(..length)?.trim_end_matches('.');
        let parts: Vec<&str> = token.split('.').collect();
        let signed = parts.len() == 3 && parts.iter().all(|part| part.len() >= SIGNED_PART);
        return signed.then_some(("signed token", token.len()));
    }
    SHAPES.iter().find_map(|shape| {
        let start = shape
            .starts
            .iter()
            .find(|start| rest.starts_with(**start))?;
        let body = rest.get(start.len()..)?;
        let length = run(body, shape.body);
        // A key has digits in it; a long name with dashes has none.
        let digits = body.get(..length)?.bytes().any(|b| b.is_ascii_digit());
        (length >= shape.least && digits).then_some((shape.name, start.len() + length))
    })
}

/// How many bytes at the start of `text` are all `of`.
fn run(text: &str, of: fn(char) -> bool) -> usize {
    text.find(|c| !of(c)).unwrap_or(text.len())
}

/// A private key that `rest` starts with, through its last line, or through the end of a text
/// that is cut before it.
fn private_key(rest: &str) -> Option<(&'static str, usize)> {
    let first = rest.lines().next()?;
    if !first.ends_with(KEY_NAMED) {
        return None;
    }
    let length = rest.find(KEY_ENDS).map_or(rest.len(), |end| {
        let last = rest.get(end..).and_then(|from| from.lines().next());
        end + last.map_or(0, str::len)
    });
    Some(("private key", length))
}

#[cfg(test)]
#[path = "shapes_tests.rs"]
mod tests;
