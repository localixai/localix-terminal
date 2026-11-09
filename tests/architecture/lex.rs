//! A small Rust tokenizer for the architecture rules: words, punctuation and string literals,
//! with comments dropped, so that a path in a comment, a doc or a string never counts as code.

/// One token of a source file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Token {
    /// An identifier, a keyword or a number (`crate`, `mod`, `r#type` as `type`, `42`).
    Word(String),
    /// A string literal's contents, raw or not (`#[path = "x.rs"]` keeps its path).
    Str(String),
    /// One punctuation character, or `::` as one token.
    Punct(&'static str),
}

impl Token {
    /// Whether this is the word `word`.
    pub(crate) fn is(&self, word: &str) -> bool {
        matches!(self, Self::Word(own) if own == word)
    }

    /// Whether this is the punctuation `punct`.
    pub(crate) fn is_punct(&self, punct: &str) -> bool {
        matches!(self, Self::Punct(own) if *own == punct)
    }

    /// The word, if this is one.
    pub(crate) fn word(&self) -> Option<&str> {
        match self {
            Self::Word(word) => Some(word),
            Self::Str(_) | Self::Punct(_) => None,
        }
    }
}

/// The punctuation the rules read; any other character is kept as `?`.
const PUNCTS: [&str; 14] = [
    "{", "}", "(", ")", "[", "]", ";", ",", "#", "!", "=", "$", ":", "?",
];

/// The tokens of `text`, with its comments left out.
pub(crate) fn tokens(text: &str) -> Vec<Token> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = vec![];
    let mut at = 0;
    while let Some(&here) = chars.get(at) {
        let next = chars.get(at + 1).copied();
        if here == '/' && next == Some('/') {
            at = skip_line(&chars, at);
        } else if here == '/' && next == Some('*') {
            at = skip_block(&chars, at);
        } else if here == '"' {
            let (literal, end) = quoted(&chars, at + 1);
            out.push(Token::Str(literal));
            at = end;
        } else if here == '\'' {
            at = skip_quote(&chars, at);
        } else if here.is_alphanumeric() || here == '_' {
            at = word(&chars, at, &mut out);
        } else if here == ':' && next == Some(':') {
            out.push(Token::Punct("::"));
            at += 2;
        } else {
            if !here.is_whitespace() {
                let punct = PUNCTS.iter().find(|punct| punct.starts_with(here));
                out.push(Token::Punct(punct.copied().unwrap_or("?")));
            }
            at += 1;
        }
    }
    out
}

fn skip_line(chars: &[char], from: usize) -> usize {
    let rest = chars.get(from..).unwrap_or_default();
    from + rest.iter().position(|&c| c == '\n').unwrap_or(rest.len())
}

/// Past a block comment, which may nest.
fn skip_block(chars: &[char], from: usize) -> usize {
    let (mut depth, mut at) = (0usize, from);
    while let Some(&here) = chars.get(at) {
        let next = chars.get(at + 1).copied();
        if here == '/' && next == Some('*') {
            depth += 1;
            at += 2;
        } else if here == '*' && next == Some('/') {
            depth -= 1;
            at += 2;
            if depth == 0 {
                return at;
            }
        } else {
            at += 1;
        }
    }
    at
}

/// A string's contents from just past its opening quote, and where it ends.
fn quoted(chars: &[char], from: usize) -> (String, usize) {
    let mut literal = String::new();
    let mut at = from;
    while let Some(&here) = chars.get(at) {
        match here {
            '\\' => {
                literal.extend(chars.get(at + 1));
                at += 2;
            }
            '"' => return (literal, at + 1),
            _ => {
                literal.push(here);
                at += 1;
            }
        }
    }
    (literal, at)
}

/// A raw string's contents from just past its `r`, and where it ends: `r"…"`, `r#"…"#`.
fn raw(chars: &[char], from: usize) -> (String, usize) {
    let hashes = chars.get(from..).unwrap_or_default();
    let hashes = hashes.iter().take_while(|&&c| c == '#').count();
    let start = from + hashes + 1;
    let mut at = start;
    while let Some(&here) = chars.get(at) {
        let closes = (1..=hashes).all(|offset| chars.get(at + offset) == Some(&'#'));
        if here == '"' && closes {
            let literal = chars.get(start..at).unwrap_or_default().iter().collect();
            return (literal, at + 1 + hashes);
        }
        at += 1;
    }
    (String::new(), at)
}

/// Past a character literal (`'x'`, `'\n'`, `'\u{1F600}'`) or a lifetime's quote (`'a`).
fn skip_quote(chars: &[char], from: usize) -> usize {
    match chars.get(from + 1) {
        Some('\\') => {
            let rest = chars.get(from + 2..).unwrap_or_default();
            from + 3 + rest.iter().position(|&c| c == '\'').unwrap_or(rest.len())
        }
        Some(_) if chars.get(from + 2) == Some(&'\'') => from + 3,
        _ => from + 1,
    }
}

/// A word from `from`, or the literal it prefixes (`b"…"`, `r#"…"#`, `b'x'`, `r#ident`).
fn word(chars: &[char], from: usize, out: &mut Vec<Token>) -> usize {
    let rest = chars.get(from..).unwrap_or_default();
    let length = rest
        .iter()
        .take_while(|&&c| c.is_alphanumeric() || c == '_')
        .count();
    let end = from + length;
    let text: String = chars.get(from..end).unwrap_or_default().iter().collect();
    let next = chars.get(end).copied();
    let raw_prefix = matches!(text.as_str(), "r" | "br" | "cr");
    let after_hash = chars.get(end + 1).copied();
    if raw_prefix
        && (next == Some('"') || (next == Some('#') && matches!(after_hash, Some('"' | '#'))))
    {
        let (literal, past) = raw(chars, end);
        out.push(Token::Str(literal));
        return past;
    }
    if text == "r" && next == Some('#') {
        return word(chars, end + 1, out);
    }
    if matches!(text.as_str(), "b" | "c") && next == Some('"') {
        let (literal, past) = quoted(chars, end + 1);
        out.push(Token::Str(literal));
        return past;
    }
    if text == "b" && next == Some('\'') {
        return skip_quote(chars, end);
    }
    out.push(Token::Word(text));
    end
}
