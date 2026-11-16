//! Tests that the guide is the one text of how an agent uses the window: each topic prints on
//! its own, every call it shows is one the tool takes, the tool's description is built from it,
//! and the Claude Code plugin's skills are stubs whose calls are the guide's own.

use std::path::Path;

use super::{description, guide};

/// The topics this build has, the overview first.
const TOPICS: [&str; 8] = [
    "window",
    "commands",
    "pages",
    "terminals",
    "panes",
    "board",
    "files",
    "person",
];

/// Every topic's text, as `llxterm guide <topic>` prints it.
fn whole() -> String {
    TOPICS
        .iter()
        .map(|topic| guide(Some(topic), &[]).unwrap())
        .collect::<Vec<_>>()
        .join("\n")
}

/// The calls a text shows, `llxterm window '{…}'`, each from the command to its closing quote.
fn calls(text: &str) -> Vec<String> {
    (text.split(COMMAND).skip(1))
        .filter(|after| after.starts_with('{'))
        .filter_map(|after| after.split_once('\''))
        .map(|(json, _)| format!("{COMMAND}{json}'"))
        .collect()
}

const COMMAND: &str = "llxterm window '";

/// The short commands a text shows in its examples: each line of a `sh` block that runs a verb
/// of the table, as the words after `llxterm`.
fn short_calls(text: &str) -> Vec<(String, Vec<String>)> {
    let fenced = text.split("```").skip(1).step_by(2);
    (fenced.flat_map(str::lines).map(str::trim))
        .filter_map(|line| Some((line, line.strip_prefix("llxterm ")?)))
        .filter(|(_, rest)| {
            let verb = rest.split_whitespace().next().unwrap_or_default();
            crate::short::VERBS.iter().any(|known| known.name() == verb)
        })
        .filter(|(_, rest)| !rest.starts_with("do -"))
        .map(|(line, rest)| {
            let argv = crate::short::words(rest).unwrap_or_else(|why| panic!("{line}: {why}"));
            (line.to_string(), argv)
        })
        .collect()
}

/// A short command's steps, each checked as the tool checks a call.
fn checked(line: &str, argv: &[String]) {
    let steps = crate::short::steps(argv, &mut || Ok("document.title".into()))
        .unwrap_or_else(|why| panic!("{line}: {why}"));
    for step in steps {
        assert_eq!(super::super::check(&step), Ok(()), "{line}: {step}");
    }
}

/// A call's arguments, as the tool would be given them.
fn arguments(call: &str) -> serde_json::Value {
    let json = (call.strip_prefix(COMMAND))
        .and_then(|json| json.strip_suffix('\''))
        .unwrap_or_default();
    serde_json::from_str(json).unwrap_or_else(|why| panic!("{call}: {why}"))
}

/// A file of the plugin's, by its path from the plugin's folder.
fn plugin(path: &str) -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("plugins/claude-code");
    std::fs::read_to_string(root.join(path)).unwrap()
}

/// A skill's or an agent's body: what follows its frontmatter.
fn body(text: &str) -> &str {
    text.splitn(3, "---\n").nth(2).unwrap_or(text)
}

#[test]
fn every_topic_prints_alone_within_what_a_host_keeps() {
    assert_eq!(guide(None, &[]), guide(Some("window"), &[]));
    for topic in TOPICS {
        let text = guide(Some(topic), &[]).unwrap();
        // Some hosts keep only the first 4096 bytes of what a call prints or a tool says.
        assert!(text.len() <= 4096, "{topic}: {} bytes", text.len());
    }
    // The overview names each other topic, so one printout leads to all.
    let overview = guide(None, &[]).unwrap();
    for topic in &TOPICS[1..] {
        assert!(overview.contains(&format!("- {topic}: ")), "{overview}");
    }
    let why = guide(Some("more"), &[]).unwrap_err();
    assert!(
        why.starts_with("no topic `more`") && why.contains("pages"),
        "{why}"
    );
}

#[test]
fn every_call_the_guide_shows_is_one_the_tool_takes() {
    let text = whole();
    let shown = calls(&text);
    let short = short_calls(&text);
    assert!(shown.len() + short.len() >= 20, "{} calls", shown.len());
    assert!(short.len() >= 8, "{} short commands", short.len());
    for call in shown {
        assert_eq!(super::super::check(&arguments(&call)), Ok(()), "{call}");
    }
    for (line, argv) in short {
        checked(&line, &argv);
    }
}

#[test]
fn the_description_is_the_overviews_lead_and_a_line_a_topic() {
    let said = description();
    let overview = guide(None, &[]).unwrap();
    let lead = overview.split("```").next().unwrap_or_default().trim_end();
    assert!(said.starts_with(lead), "{said}");
    for topic in &TOPICS[1..] {
        let first = guide(Some(topic), &[]).unwrap();
        let line = format!("- {topic}: {}", first.lines().next().unwrap_or_default());
        assert!(said.contains(&line), "{said}\nhas no {line}");
    }
    assert!(said.contains("llxterm guide <topic>"), "{said}");
    // It is the short form: the rest is a topic away.
    assert!(said.len() <= 2560, "{} bytes", said.len());
}

#[test]
fn the_plugins_skills_are_stubs_of_the_guide() {
    let text = whole();
    for skill in ["window", "board"] {
        let file = plugin(&format!("skills/{skill}/SKILL.md"));
        let stub = body(&file);
        assert!(stub.len() <= 1024, "{skill}: {} bytes", stub.len());
        assert!(
            file.contains("Bash(llxterm guide:*)"),
            "{skill} may not run the guide"
        );
        let shown = calls(stub);
        let short = short_calls(stub);
        assert!(
            shown.len() + short.len() >= 3,
            "{skill} shows {} calls",
            shown.len()
        );
        for call in shown {
            assert!(text.contains(&call), "{skill}: the guide has no {call}");
        }
        for (line, _) in short {
            assert!(text.contains(&line), "{skill}: the guide has no {line}");
        }
        // Each topic a stub sends to is one the guide has.
        for after in stub.split("llxterm guide ").skip(1) {
            let topic: String = after.chars().take_while(char::is_ascii_lowercase).collect();
            assert!(
                TOPICS.contains(&topic.as_str()),
                "{skill}: no topic `{topic}`"
            );
        }
    }
    // The worker's calls are the guide's too.
    for call in calls(&plugin("agents/worker.md")) {
        assert!(text.contains(&call), "worker: the guide has no {call}");
    }
}

#[test]
fn the_window_skill_may_run_each_short_command_and_nothing_else_of_the_app() {
    let file = plugin("skills/window/SKILL.md");
    let allowed = (file.lines())
        .find_map(|line| line.strip_prefix("allowed-tools: "))
        .unwrap_or_default();
    for verb in &crate::short::VERBS {
        let rule = format!("Bash(llxterm {}:*)", verb.name());
        assert!(
            allowed.contains(&rule),
            "the window skill may not run {rule}"
        );
    }
    // The whole command would also let a skill quit the app or forget a secret.
    assert!(!allowed.contains("Bash(llxterm:*)"), "{allowed}");
    for never in ["quit", "secret"] {
        assert!(!allowed.contains(&format!("llxterm {never}")), "{allowed}");
    }
}
