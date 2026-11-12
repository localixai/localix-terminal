//! Tests of `skill`: a skill's folder copied whole where each agent reads it, with this app's
//! mark, and nothing of the person's written over, removed or followed.

use std::path::Path;

use super::super::tests::{env_at, skill, target};
use super::{MARK, There, copy_tree, safe, there};
use crate::catalog::{Env, With};
use crate::testing::Scratch;

/// A fetched skill's folder under `scratch`: its `SKILL.md` saying `says`, and a file inside.
fn fetched(scratch: &Scratch, name: &str, says: &str) -> With {
    scratch.file(&format!("fetched/{name}/SKILL.md"), says);
    scratch.file(&format!("fetched/{name}/scripts/run.sh"), "echo run\n");
    With {
        folder: Some(scratch.path().join("fetched").join(name)),
        ..With::default()
    }
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap()
}

/// What is in `dir`, hidden names too: nothing is left on its way in or out.
fn names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = (std::fs::read_dir(dir).unwrap().flatten())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn a_skill_is_copied_where_each_agent_reads_skills() {
    let scratch = Scratch::new();
    let home = scratch.path().join("home");
    let env = Env {
        vars: [(
            "XDG_CONFIG_HOME".to_string(),
            home.join("xdg").display().to_string(),
        )]
        .into(),
        ..env_at(&home)
    };
    let with = fetched(&scratch, "pdf", "# PDF\n");
    let places = [
        ("claude", ".claude/skills/pdf"),
        ("codex", ".agents/skills/pdf"),
        ("copilot", ".copilot/skills/pdf"),
        ("opencode", "xdg/opencode/skills/pdf"),
    ];
    for (id, place) in places {
        let done = target(id).install(&skill("pdf"), &with, &env).unwrap();
        let folder = home.join(place);
        assert!(done.contains(&format!("~/{place}")), "{done}");
        assert_eq!(read(&folder.join("SKILL.md")), "# PDF\n");
        assert_eq!(read(&folder.join("scripts/run.sh")), "echo run\n");
        let mark: serde_json::Value = serde_json::from_str(&read(&folder.join(MARK))).unwrap();
        assert_eq!(
            mark,
            serde_json::json!({"listing": "src/pdf", "version": "1.0.0"})
        );
        assert_eq!(
            target(id).has(&skill("pdf"), &env).as_deref(),
            Some("1.0.0")
        );
        assert_eq!(names(folder.parent().unwrap()), ["pdf"]);
    }
}

#[test]
fn claude_code_reads_skills_from_the_folder_its_variable_names() {
    let scratch = Scratch::new();
    let moved = scratch.path().join("claude-elsewhere");
    let env = Env {
        vars: [("CLAUDE_CONFIG_DIR".to_string(), moved.display().to_string())].into(),
        ..env_at(scratch.path())
    };
    let with = fetched(&scratch, "pdf", "# PDF\n");
    target("claude")
        .install(&skill("pdf"), &with, &env)
        .unwrap();
    assert!(moved.join("skills/pdf/SKILL.md").is_file());
}

#[test]
fn codex_and_gemini_share_one_folder_and_the_plan_says_so() {
    let scratch = Scratch::new();
    let env = env_at(scratch.path());
    let plan = target("gemini").plan(&skill("pdf"), &env).unwrap();
    assert_eq!(
        plan.steps,
        ["Copy the skill to ~/.agents/skills/pdf, which Codex and Gemini CLI both read"]
    );
    assert_eq!(plan.after, "Start Gemini CLI again to use it");
    let with = fetched(&scratch, "pdf", "# PDF\n");
    let done = target("codex").install(&skill("pdf"), &with, &env).unwrap();
    assert!(
        done.contains("which Codex and Gemini CLI both read"),
        "{done}"
    );
    // What Codex was given, Gemini CLI has too.
    assert_eq!(
        target("gemini").has(&skill("pdf"), &env).as_deref(),
        Some("1.0.0")
    );
    let plan = target("gemini").plan(&skill("pdf"), &env).unwrap();
    assert!(plan.steps[0].starts_with("Replace the skill in ~/.agents/skills/pdf (version 1.0.0)"));
}

#[test]
fn an_older_version_is_replaced_whole() {
    let scratch = Scratch::new();
    let env = env_at(scratch.path());
    let mut older = skill("pdf");
    older.version = "0.9.0".into();
    let first = fetched(&scratch, "old", "# Old\n");
    scratch.file("fetched/old/gone.md", "only in the old one\n");
    target("claude").install(&older, &first, &env).unwrap();
    assert_eq!(
        target("claude").has(&skill("pdf"), &env).as_deref(),
        Some("0.9.0")
    );
    let plan = target("claude").plan(&skill("pdf"), &env).unwrap();
    assert_eq!(
        plan.steps,
        ["Replace the skill in ~/.claude/skills/pdf (version 0.9.0)"]
    );
    target("claude")
        .install(&skill("pdf"), &fetched(&scratch, "new", "# New\n"), &env)
        .unwrap();
    let folder = scratch.path().join(".claude/skills/pdf");
    assert_eq!(read(&folder.join("SKILL.md")), "# New\n");
    assert!(!folder.join("gone.md").exists());
    assert_eq!(
        target("claude").has(&skill("pdf"), &env).as_deref(),
        Some("1.0.0")
    );
    assert_eq!(names(&scratch.path().join(".claude/skills")), ["pdf"]);
}

#[test]
fn a_skill_is_removed_and_nothing_is_left() {
    let scratch = Scratch::new();
    let env = env_at(scratch.path());
    let with = fetched(&scratch, "pdf", "# PDF\n");
    target("copilot")
        .install(&skill("pdf"), &with, &env)
        .unwrap();
    let done = target("copilot").remove(&skill("pdf"), &env).unwrap();
    assert_eq!(done, "Removed the skill from ~/.copilot/skills/pdf.");
    assert_eq!(target("copilot").has(&skill("pdf"), &env), None);
    assert!(names(&scratch.path().join(".copilot/skills")).is_empty());
    // A second time there is nothing to take, and that is no failure.
    let again = target("copilot").remove(&skill("pdf"), &env).unwrap();
    assert_eq!(
        again,
        "GitHub Copilot has no skill at ~/.copilot/skills/pdf."
    );
}

#[test]
fn the_persons_own_skill_is_never_written_over_or_removed() {
    let scratch = Scratch::new();
    let env = env_at(scratch.path());
    let own = scratch.file(".claude/skills/pdf/SKILL.md", "# Mine\n");
    let why =
        "~/.claude/skills/pdf is a skill Localix Terminal did not install: it is left as it is";
    let claude = target("claude");
    assert_eq!(claude.plan(&skill("pdf"), &env).unwrap_err(), why);
    let with = fetched(&scratch, "pdf", "# PDF\n");
    assert_eq!(claude.install(&skill("pdf"), &with, &env).unwrap_err(), why);
    assert_eq!(claude.remove(&skill("pdf"), &env).unwrap_err(), why);
    assert_eq!(claude.has(&skill("pdf"), &env), None);
    assert_eq!(read(&own), "# Mine\n");
}

#[test]
fn another_listings_skill_of_the_same_name_is_left() {
    let scratch = Scratch::new();
    let env = env_at(scratch.path());
    let mut other = skill("pdf");
    other.id = "elsewhere/pdf".into();
    target("claude")
        .install(&other, &fetched(&scratch, "pdf", "# Other\n"), &env)
        .unwrap();
    let why = target("claude").remove(&skill("pdf"), &env).unwrap_err();
    assert_eq!(
        why,
        "~/.claude/skills/pdf holds the skill of another listing (elsewhere/pdf): remove that one first"
    );
    assert_eq!(target("claude").has(&skill("pdf"), &env), None);
    assert_eq!(
        read(&scratch.path().join(".claude/skills/pdf/SKILL.md")),
        "# Other\n"
    );
}

#[test]
#[cfg(unix)]
fn a_link_at_the_place_is_the_persons_and_is_never_followed() {
    let scratch = Scratch::new();
    let env = env_at(scratch.path());
    let elsewhere = scratch.path().join("elsewhere");
    scratch.file("elsewhere/SKILL.md", "# Linked\n");
    scratch.file(
        "elsewhere/.llxterm.json",
        r#"{"listing": "src/pdf", "version": "1.0.0"}"#,
    );
    std::fs::create_dir_all(scratch.path().join(".claude/skills")).unwrap();
    let link = scratch.path().join(".claude/skills/pdf");
    std::os::unix::fs::symlink(&elsewhere, &link).unwrap();
    assert_eq!(there(&link, &skill("pdf")), There::Persons);
    assert!(target("claude").remove(&skill("pdf"), &env).is_err());
    assert_eq!(target("claude").has(&skill("pdf"), &env), None);
    assert_eq!(read(&elsewhere.join("SKILL.md")), "# Linked\n");
}

#[test]
#[cfg(unix)]
fn a_link_inside_the_fetched_folder_is_left_out_of_the_copy() {
    let scratch = Scratch::new();
    let secret = scratch.file("secret.txt", "not for the skill\n");
    scratch.file("from/SKILL.md", "# PDF\n");
    std::os::unix::fs::symlink(&secret, scratch.path().join("from/leak.txt")).unwrap();
    std::os::unix::fs::symlink(scratch.path(), scratch.path().join("from/up")).unwrap();
    let to = scratch.path().join("to");
    copy_tree(&scratch.path().join("from"), &to).unwrap();
    assert_eq!(names(&to), ["SKILL.md"]);
}

#[test]
fn a_folder_without_a_skill_is_not_copied() {
    let scratch = Scratch::new();
    let env = env_at(scratch.path());
    scratch.file("fetched/readme.md", "no skill here\n");
    let with = With {
        folder: Some(scratch.path().join("fetched")),
        ..With::default()
    };
    let why = target("claude")
        .install(&skill("pdf"), &with, &env)
        .unwrap_err();
    assert!(why.ends_with("holds no SKILL.md"), "{why}");
    let unfetched = target("claude").install(&skill("pdf"), &With::default(), &env);
    assert_eq!(unfetched.unwrap_err(), "the skill's folder was not fetched");
    assert!(!scratch.path().join(".claude").exists());
}

#[test]
fn a_name_that_is_not_one_safe_segment_is_refused_before_anything_is_written() {
    for name in [
        "",
        "..",
        "../up",
        "a/b",
        "PDF",
        "-pdf",
        "pdf-",
        "p--df",
        "p df",
        &"a".repeat(65),
    ] {
        assert!(safe(name).is_err(), "{name:?}");
    }
    for name in ["pdf", "pdf-tools", "a1", &"a".repeat(64)] {
        assert_eq!(safe(name), Ok(()), "{name:?}");
    }
    let scratch = Scratch::new();
    let env = env_at(scratch.path());
    let with = fetched(&scratch, "x", "# X\n");
    let why = target("claude")
        .install(&skill("../up"), &with, &env)
        .unwrap_err();
    assert_eq!(why, "`../up` is no skill's name (a-z, 0-9 and -)");
    assert!(target("claude").plan(&skill("../up"), &env).is_err());
    assert_eq!(names(scratch.path()), ["fetched"]);
}
