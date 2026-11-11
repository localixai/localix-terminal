//! What a program asks its terminal to tell the person (OSC 9, OSC 777 `notify`): an agent
//! that finished, a build that failed. The words are a program's, untrusted as its output is:
//! they are kept as plain text for the person to read, never acted on and never told to an
//! agent. The last one stays until the person types in the terminal.

/// The most characters of a note kept: a line, not a letter.
const MOST: usize = 200;

#[derive(Debug, Default)]
pub(super) struct Notes {
    said: Option<String>,
    count: u32,
}

impl Notes {
    /// Reads a string sequence's words: `9;Build failed`, `777;notify;Build;It failed`. The
    /// same number opens other requests (`9;4;1;50`, a progress bar), told apart by the number
    /// that follows: they say nothing to the person.
    pub(super) fn read(&mut self, said: &str) {
        let words = match said.split_once(';') {
            Some(("9", words)) => {
                let first = words.split(';').next().unwrap_or_default();
                if !first.is_empty() && first.bytes().all(|byte| byte.is_ascii_digit()) {
                    return;
                }
                plain(words)
            }
            Some(("777", request)) => {
                let Some(("notify", words)) = request.split_once(';') else {
                    return;
                };
                let (title, body) = words.split_once(';').unwrap_or((words, ""));
                match (plain(title), plain(body)) {
                    (title, body) if title.is_empty() => body,
                    (title, body) if body.is_empty() => title,
                    (title, body) => format!("{title}: {body}"),
                }
            }
            _ => return,
        };
        if !words.is_empty() {
            self.said = Some(words.chars().take(MOST).collect());
            self.count = self.count.wrapping_add(1);
        }
    }

    /// The note that waits for the person, and its number: another one said is another number.
    pub(super) fn waiting(&self) -> Option<(u32, &str)> {
        Some((self.count, self.said.as_deref()?))
    }

    /// The person is at the terminal: the note is read. Whether there was one.
    pub(super) fn heard(&mut self) -> bool {
        self.said.take().is_some()
    }
}

/// `words` as plain text on one line: no control character, spaces single.
fn plain(words: &str) -> String {
    let words: String = (words.chars())
        .map(|ch| if ch.is_control() { ' ' } else { ch })
        .collect();
    words.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
#[path = "notes_tests.rs"]
mod tests;
