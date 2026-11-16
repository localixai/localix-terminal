//! How a coding agent's session goes on in its terminal when the app opens again: which session
//! its command line names, what of that line is carried (a list of what is allowed, per agent:
//! never a prompt, a run that prints and ends, a value that may hold a secret, a sandbox
//! bypass), and the command that resumes it, built again from those words alone. The
//! permission skip a person chose is carried, and said in the line the terminal shows.

use std::ffi::OsStr;
use std::fmt::Write as _;

use serde::{Deserialize, Serialize};

use super::AGENTS;

/// Whether agents go on in their terminals as the app opens (Settings, Terminal).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Resuming {
    #[default]
    On,
    Off,
}

/// An agent's command line as it is read for resuming.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Launch {
    /// The agent's id in the table (`claude`).
    pub agent: &'static str,
    /// The session the line names, in the agent's own form; none where it names none.
    pub session: Option<String>,
    /// The words carried, each option with its value.
    pub kept: Vec<String>,
    /// A permission skip is among them.
    pub skips: bool,
    /// A sandbox bypass was dropped.
    pub bypass: bool,
}

/// How an option's value is held to what it may be.
#[derive(Clone, Copy)]
enum Check {
    /// No value: a flag.
    Flag,
    /// A name: letters, digits and `._:/@[]-`, no secret's look.
    Word,
    Choice(&'static [&'static str]),
    /// A folder or a file by its whole path, held to its form alone: the disk is never asked
    /// (one under Documents would make macOS ask the person as the app starts).
    Path,
}

/// The form of an agent's session ids.
#[derive(Clone, Copy)]
enum Form {
    Uuid,
    /// `ses_` and letters and digits (OpenCode's).
    Prefixed(&'static str),
}

/// What of an agent's command line is read, carried and dropped. Each list is of options as the
/// agent's own `--help` names them, apart by spaces; anything not in one is dropped.
struct Policy {
    agent: &'static str,
    /// The words that resume a session, `{}` its id; none: it cannot be named.
    resume: &'static [&'static str],
    /// The options whose value is the session's id.
    ids: &'static str,
    /// A first word whose next word is the session's id (`codex resume <id>`).
    id_after: Option<&'static str>,
    form: Form,
    kept: &'static [(&'static str, Check)],
    /// Options (or `option=value`) that skip the agent's questions: carried, and said.
    skips: &'static str,
    /// Options (or `option=value`) that take its sandbox away: never carried, and said.
    bypass: &'static str,
    /// Options of a run that prints and ends, or runs elsewhere: nothing to resume.
    once: &'static str,
    /// Options that make the session another than the one the line names.
    forks: &'static str,
    /// Options dropped with the value after them.
    valued: &'static str,
    /// First words that are a command of the agent's own, not a session.
    commands: &'static str,
}

/// Whether `list`, words apart by spaces, has `word`.
fn has(list: &str, word: &str) -> bool {
    list.split_whitespace().any(|listed| listed == word)
}

#[path = "agents_policies.rs"]
mod policies;
#[path = "agents_words.rs"]
mod words;
use policies::POLICIES;
use words::quoted;

/// `argv` (a program's arguments, its own first) read as a coding agent's session: none where
/// it is no agent's, or a run of one that is not a session to go on with.
pub fn launch(argv: &[String]) -> Option<Launch> {
    let file = argv.first()?.rsplit('/').next()?;
    let agent = AGENTS.iter().find(|agent| agent.commands.contains(&file))?;
    let policy = POLICIES.iter().find(|policy| policy.agent == agent.id)?;
    policy.read(argv.get(1..)?)
}

impl Policy {
    fn read(&self, words: &[String]) -> Option<Launch> {
        let mut read = Launch {
            agent: self.agent,
            session: None,
            kept: Vec::new(),
            skips: false,
            bypass: false,
        };
        let (mut forked, mut first, mut id_next) = (false, true, false);
        let mut at = 0;
        while let Some(word) = words.get(at) {
            at += 1;
            if word == "--" {
                break;
            }
            if !word.starts_with('-') || word.len() == 1 {
                if id_next {
                    id_next = false;
                    read.session = read.session.or_else(|| self.form.holds(word));
                } else if first && self.id_after == Some(word.as_str()) {
                    id_next = true;
                } else if first && has(self.commands, word) {
                    return None;
                }
                // Else a prompt: never carried.
                first = false;
                continue;
            }
            // An option leaves the first word to come the first: `codex -m o3 resume <id>`.
            let (name, inline) = match word.split_once('=') {
                Some((name, value)) if word.starts_with("--") => (name, Some(value.to_string())),
                _ => (word.as_str(), None),
            };
            // The value an option takes: written after `=`, else the next word, if it is no option.
            let value = |at: &mut usize| {
                inline.clone().or_else(|| {
                    let next = words.get(*at).filter(|next| !next.starts_with('-'))?;
                    *at += 1;
                    Some(next.clone())
                })
            };
            if has(self.once, name) {
                return None;
            }
            if has(self.forks, name) {
                forked = true;
            } else if has(self.ids, name) {
                if let Some(id) = value(&mut at).and_then(|id| self.form.holds(&id)) {
                    read.session = Some(id);
                }
            } else if has(self.bypass, name) {
                read.bypass = true;
            } else if has(self.skips, name) {
                read.kept.push(name.to_string());
                read.skips = true;
            } else if let Some((_, check)) = self.kept.iter().find(|(kept, _)| *kept == name) {
                if matches!(check, Check::Flag) {
                    read.kept.push(name.to_string());
                    continue;
                }
                let Some(value) = value(&mut at) else {
                    continue;
                };
                let both = format!("{name}={value}");
                if has(self.bypass, &both) {
                    read.bypass = true;
                } else if check.holds(&value) {
                    read.skips |= has(self.skips, &both);
                    read.kept.extend([name.to_string(), value]);
                }
            } else if has(self.valued, name) {
                drop(value(&mut at));
            }
        }
        if forked {
            read.session = None;
        }
        Some(read)
    }
}

impl Form {
    /// `word` as an id of this form, if it is one.
    fn holds(self, word: &str) -> Option<String> {
        let fits = match self {
            Self::Uuid => is_uuid(word),
            Self::Prefixed(prefix) => word.strip_prefix(prefix).is_some_and(|rest| {
                (8..=64).contains(&rest.len()) && rest.chars().all(|c| c.is_ascii_alphanumeric())
            }),
        };
        fits.then(|| word.to_string())
    }
}

/// Whether `word` is a UUID as agents write one: 8-4-4-4-12 hexadecimal digits.
pub(super) fn is_uuid(word: &str) -> bool {
    let parts: Vec<&str> = word.split('-').collect();
    parts.len() == 5
        && (parts.iter().zip([8, 4, 4, 4, 12]))
            .all(|(part, len)| part.len() == len && part.chars().all(|c| c.is_ascii_hexdigit()))
}

/// Whether `session` is an id of agent `agent`'s own form, one its sessions resume by.
pub fn names_session(agent: &str, session: &str) -> bool {
    (POLICIES.iter().find(|policy| policy.agent == agent))
        .is_some_and(|policy| !policy.resume.is_empty() && policy.form.holds(session).is_some())
}

/// A session's command, as it is typed into the terminal's shell again.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resumed {
    /// The agent's name (`Claude Code`).
    pub name: &'static str,
    /// The command line, each word quoted for the shell where it must be.
    pub line: String,
    /// A permission skip is carried in it.
    pub skips: bool,
}

impl Resumed {
    /// The line the terminal shows above it, the agent and what the person should know of how
    /// it runs (`bypass`: a sandbox bypass its line had was dropped): `Resumed Claude Code
    /// session · permissions skipped`.
    pub fn said(&self, bypass: bool) -> String {
        let mut said = format!("Resumed {} session", self.name);
        if self.skips {
            said.push_str(" · permissions skipped");
        }
        if bypass {
            said.push_str(" · sandbox bypass not carried");
        }
        said
    }
}

/// The command that resumes session `session` of agent `agent`, with the words `kept` of its
/// line read through the agent's list again (a row of the store is never taken as it is), on
/// `path`. Why not, where it cannot be.
pub fn resumed(
    agent: &str,
    session: &str,
    kept: &[String],
    path: &OsStr,
) -> Result<Resumed, String> {
    let known = AGENTS.iter().find(|known| known.id == agent);
    let known = known.ok_or_else(|| format!("there is no agent `{agent}`"))?;
    let policy = POLICIES.iter().find(|policy| policy.agent == agent);
    let policy = policy.filter(|policy| !policy.resume.is_empty());
    let policy =
        policy.ok_or_else(|| format!("{} cannot resume a session by its id", known.name))?;
    let session = (policy.form.holds(session))
        .ok_or_else(|| format!("`{session}` is no session of {}", known.name))?;
    let command =
        (known.command_on(path)).ok_or_else(|| format!("{} is not installed", known.name))?;
    let read = policy.read(kept).unwrap_or_else(|| Launch {
        agent: policy.agent,
        session: None,
        kept: Vec::new(),
        skips: false,
        bypass: false,
    });
    let mut words = vec![command.to_string()];
    words.extend(
        policy
            .resume
            .iter()
            .map(|word| word.replace("{}", &session)),
    );
    words.extend(read.kept.iter().cloned());
    let line: Option<Vec<String>> = words.iter().map(|word| quoted(word)).collect();
    let line = line.ok_or_else(|| format!("a word of {}'s line does not print", known.name))?;
    Ok(Resumed {
        name: known.name,
        line: line.join(" "),
        skips: read.skips,
    })
}

/// `command`, a coding agent's own command as "+" starts it, with an id it chose for the new
/// session (`claude --session-id <uuid>`), so its terminal knows the session from the start:
/// none where the agent takes no such id, or the line is more than its command.
pub fn pinned(command: &str) -> Option<String> {
    let agent = AGENTS
        .iter()
        .find(|agent| agent.commands.contains(&command))?;
    POLICIES
        .iter()
        .find(|policy| policy.agent == agent.id)
        .filter(|policy| has(policy.ids, "--session-id"))?;
    Some(format!("{command} --session-id {}", new_uuid()))
}

/// A random UUID (version 4).
fn new_uuid() -> String {
    let mut bytes = crate::random_bytes::<16>();
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex = bytes.iter().fold(String::new(), |mut hex, byte| {
        let _ = write!(hex, "{byte:02x}");
        hex
    });
    let part = |from: usize, to: usize| hex.get(from..to).unwrap_or_default();
    format!(
        "{}-{}-{}-{}-{}",
        part(0, 8),
        part(8, 12),
        part(12, 16),
        part(16, 20),
        part(20, 32)
    )
}

#[cfg(test)]
#[path = "agents_resume_tests.rs"]
mod tests;
