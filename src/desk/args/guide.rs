//! The window tool's guide: the one text of how an agent uses the window, a topic at a time.
//! The topics are files read in as the program is built (`guide/*.md`), the short commands' table
//! (`crate::short`), and each kind of tab that draws itself (`About::guide`). `llxterm guide
//! [topic]` prints one, and so does the tool's `guide` action; the tool's description, which
//! every call's model reads whole and some hosts cut at 4096 bytes, is built from them: the
//! overview's lead and a line a topic. A fact a field's own description states
//! (args.rs) need not be said in a topic; the Claude Code plugin's skills are stubs of the same
//! calls, which a test holds to these texts.

use std::fmt::Write as _;

use crate::kind::About;

/// A topic of the guide: its name, as `llxterm guide <name>` takes it, and its text, whose first
/// line says in one sentence what it covers.
struct Topic {
    name: &'static str,
    text: &'static str,
}

/// The window's own topics, the overview first.
const TOPICS: [Topic; 7] = [
    Topic {
        name: OVERVIEW,
        text: include_str!("guide/window.md"),
    },
    Topic {
        name: "pages",
        text: include_str!("guide/pages.md"),
    },
    Topic {
        name: "terminals",
        text: include_str!("guide/terminals.md"),
    },
    Topic {
        name: "panes",
        text: include_str!("guide/panes.md"),
    },
    Topic {
        name: "board",
        text: include_str!("guide/board.md"),
    },
    Topic {
        name: "files",
        text: include_str!("guide/files.md"),
    },
    Topic {
        name: "person",
        text: include_str!("guide/person.md"),
    },
];

/// The topic printed when none is named: what the tool is, and the other topics.
const OVERVIEW: &str = "window";

/// A topic as it is printed: its name, its line in a list of topics, and its whole text.
struct Printed {
    name: &'static str,
    line: String,
    text: String,
}

/// The topic of the short commands, printed from their one table (`crate::short`).
const COMMANDS: &str = "commands";

/// Every topic: the overview, the short commands, the window's other topics, then each kind's,
/// whose line is its guide's first sentence.
fn topics(kinds: &[&'static About]) -> Vec<Printed> {
    let printed = |name: &'static str, text: &str| Printed {
        name,
        line: text.lines().next().unwrap_or_default().to_string(),
        text: text.trim_end().to_string(),
    };
    let (overview, rest) = TOPICS.split_at(1);
    let own = (overview.iter().map(|topic| printed(topic.name, topic.text)))
        .chain(std::iter::once(printed(COMMANDS, &crate::short::guide())))
        .chain(rest.iter().map(|topic| printed(topic.name, topic.text)));
    let theirs = kinds.iter().map(|about| {
        let first = about.guide.split_inclusive(". ").next().unwrap_or_default();
        Printed {
            name: about.id,
            line: format!(
                "{}, in `{}` tabs.",
                first.trim_end().trim_end_matches('.'),
                about.letter
            ),
            text: format!("{} (`{}` tabs): {}", about.title, about.letter, about.guide),
        }
    });
    own.chain(theirs).collect()
}

/// The list of every topic but the overview, a line each.
fn listed(topics: &[Printed]) -> String {
    let mut said = String::new();
    for topic in topics.iter().filter(|topic| topic.name != OVERVIEW) {
        let _ = write!(said, "\n- {}: {}", topic.name, topic.line);
    }
    said
}

/// The guide on `topic`, or the overview and the list of topics with none named; with `kinds`,
/// the kinds of tab listed besides the window's own. Else what topics there are.
pub fn guide(topic: Option<&str>, kinds: &[&'static About]) -> Result<String, String> {
    let all = topics(kinds);
    let name = topic.unwrap_or(OVERVIEW);
    let Some(found) = all.iter().find(|topic| topic.name == name) else {
        let names: Vec<&str> = all.iter().map(|topic| topic.name).collect();
        return Err(format!(
            "no topic `{name}`: the topics are {}.",
            names.join(", ")
        ));
    };
    if name != OVERVIEW {
        return Ok(found.text.clone());
    }
    Ok(format!(
        "{}\n\nThe topics, each printed by `llxterm guide <topic>`:{}",
        found.text,
        listed(&all)
    ))
}

/// The tool's description with the kinds of tab the app listed.
pub fn description() -> String {
    described(&crate::kind::abouts())
}

/// The tool's description: the overview up to its first example, then a line a topic (`kinds`
/// among them) and where the rest of it is.
pub fn described(kinds: &[&'static About]) -> String {
    let all = topics(kinds);
    let overview = TOPICS[0].text;
    let lead = overview.split("```").next().unwrap_or(overview).trim_end();
    format!(
        "{lead}\n\nThe rest, a topic each, is printed by `llxterm guide <topic>` in a shell, or \
         the `guide` action with its `topic`; every field says what it does in the schema.{}",
        listed(&all)
    )
}

#[cfg(test)]
#[path = "guide_tests.rs"]
mod tests;
