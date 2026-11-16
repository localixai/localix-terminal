//! How a word of an agent's command line is held before it is carried or typed: a value to the
//! form its option takes, never a secret's look, never a character a terminal or the shell's
//! line would read as anything but itself (a control: ^U erases the line, ^O runs it in some; a
//! mark that turns text around or hides it), and quoted for the shell where it must be.

use super::Check;

impl Check {
    pub(super) fn holds(self, value: &str) -> bool {
        match self {
            Self::Flag => false,
            Self::Word => {
                let allowed = |c: char| c.is_ascii_alphanumeric() || "._:/@[]-".contains(c);
                (1..=100).contains(&value.len())
                    && value.chars().all(allowed)
                    && !secret_like(value)
            }
            Self::Choice(choices) => choices.contains(&value),
            Self::Path => {
                let path = std::path::Path::new(value);
                (1..=1024).contains(&value.len())
                    && path.is_absolute()
                    && printable(value)
                    && (path.components()).all(|part| {
                        matches!(
                            part,
                            std::path::Component::RootDir | std::path::Component::Normal(_)
                        )
                    })
            }
        }
    }
}

/// Whether `value` reads as a key or a token rather than a name: never carried.
fn secret_like(value: &str) -> bool {
    const PREFIXES: [&str; 8] = [
        "sk-",
        "ghp_",
        "gho_",
        "github_pat_",
        "xox",
        "AKIA",
        "AIza",
        "glpat-",
    ];
    PREFIXES.iter().any(|prefix| value.starts_with(prefix))
}

/// Whether every character of `text` shows as itself: no control, no mark that changes the
/// direction of text or hides itself, no line or paragraph break of Unicode's.
pub(super) fn printable(text: &str) -> bool {
    let hidden = |c: char| {
        matches!(c,
            '\u{00ad}' | '\u{061c}' | '\u{180e}' | '\u{200b}'..='\u{200f}'
            | '\u{2028}'..='\u{202e}' | '\u{2060}'..='\u{206f}' | '\u{feff}'
            | '\u{fff9}'..='\u{fffb}')
    };
    text.chars().all(|c| !c.is_control() && !hidden(c))
}

/// `word` as the shell reads it back the same: as it is when it holds nothing the shell would
/// read otherwise, else in single quotes; none where it holds a character that does not print,
/// which no quoting keeps from the terminal's line.
pub(super) fn quoted(word: &str) -> Option<String> {
    if !printable(word) {
        return None;
    }
    let plain = |c: char| c.is_ascii_alphanumeric() || "-_./:=@%+,".contains(c);
    Some(match !word.is_empty() && word.chars().all(plain) {
        true => word.to_string(),
        false => format!("'{}'", word.replace('\'', r"'\''")),
    })
}
