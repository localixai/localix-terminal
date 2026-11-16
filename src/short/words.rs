//! Words and steps: a step written as a string split as a shell splits it (quotes, a backslash
//! outside single quotes, no substitution), and a command line cut into steps at each `+`.

/// `text` as a shell's words: single quotes keep all, double quotes keep all but `\"` and `\\`,
/// a backslash outside them keeps the next character. Nothing is substituted: `$HOME` stays.
pub fn words(text: &str) -> Result<Vec<String>, String> {
    let mut words = vec![];
    let mut word = String::new();
    // A word was begun, even an empty one (`""`).
    let mut begun = false;
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        match c {
            '\'' => {
                begun = true;
                loop {
                    match chars.next() {
                        Some('\'') => break,
                        Some(c) => word.push(c),
                        None => return Err(unclosed(text)),
                    }
                }
            }
            '"' => {
                begun = true;
                loop {
                    match chars.next() {
                        Some('"') => break,
                        Some('\\') => match chars.next() {
                            Some(c @ ('"' | '\\')) => word.push(c),
                            Some(c) => {
                                word.push('\\');
                                word.push(c);
                            }
                            None => return Err(unclosed(text)),
                        },
                        Some(c) => word.push(c),
                        None => return Err(unclosed(text)),
                    }
                }
            }
            '\\' => {
                begun = true;
                word.extend(chars.next());
            }
            c if c.is_whitespace() => {
                if begun {
                    words.push(std::mem::take(&mut word));
                    begun = false;
                }
            }
            c => {
                begun = true;
                word.push(c);
            }
        }
    }
    if begun {
        words.push(word);
    }
    Ok(words)
}

fn unclosed(text: &str) -> String {
    format!("a quote is not closed in `{text}`")
}

/// `argv` cut into steps at each lone `+`; after a `--`, the rest is that step's values.
pub(super) fn split(argv: &[String]) -> Result<Vec<&[String]>, String> {
    if argv.is_empty() {
        return Err("no command".into());
    }
    let mut steps = vec![];
    let mut start = 0;
    for (at, word) in argv.iter().enumerate() {
        if word == "--" {
            break;
        }
        if word == "+" {
            if at == start {
                return Err("nothing before `+`: a step is a command and its words".into());
            }
            steps.push(argv.get(start..at).unwrap_or_default());
            start = at + 1;
        }
    }
    match argv.get(start..) {
        Some(rest) if !rest.is_empty() => steps.push(rest),
        _ => return Err("nothing after `+`: a step is a command and its words".into()),
    }
    Ok(steps)
}

/// How many characters must change to make `a` into `b`: how near a mistyped command is.
pub(super) fn distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut diagonal = row.first().copied().unwrap_or_default();
        if let Some(first) = row.first_mut() {
            *first = i + 1;
        }
        for (j, cb) in b.iter().enumerate() {
            let above = row.get(j + 1).copied().unwrap_or_default();
            let left = row.get(j).copied().unwrap_or_default();
            let next = (diagonal + usize::from(ca != *cb))
                .min(above + 1)
                .min(left + 1);
            diagonal = above;
            if let Some(cell) = row.get_mut(j + 1) {
                *cell = next;
            }
        }
    }
    row.last().copied().unwrap_or_default()
}
