//! Tests of `agents`.

use std::os::unix::fs::PermissionsExt;

use super::*;
use crate::testing::Scratch;

#[test]
fn an_agent_is_offered_where_its_command_can_run_on_the_path() {
    let scratch = Scratch::new();
    let (bin, other) = (scratch.path().join("bin"), scratch.path().join("other"));
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::create_dir_all(&other).unwrap();
    let path = std::env::join_paths([&bin, &other]).unwrap();
    let copilot = &AGENTS[3];
    assert_eq!(copilot.command_on(&path), None);
    // A file that cannot run is no command.
    std::fs::write(other.join("copilot"), "#!/bin/sh\n").unwrap();
    assert_eq!(copilot.command_on(&path), None);
    let run = std::fs::Permissions::from_mode(0o755);
    std::fs::set_permissions(other.join("copilot"), run.clone()).unwrap();
    assert_eq!(copilot.command_on(&path), Some("copilot"));
    // A program that only shares the agent's name is not its interface: the shell helper that
    // suggests one command, the runner that does one task and leaves.
    for (agent, not_it) in [(copilot, "ghcs"), (&AGENTS[5], "hermes-agent")] {
        let alone = scratch.path().join(not_it);
        std::fs::create_dir_all(&alone).unwrap();
        std::fs::write(alone.join(not_it), "#!/bin/sh\n").unwrap();
        std::fs::set_permissions(alone.join(not_it), run.clone()).unwrap();
        assert_eq!(agent.command_on(alone.as_os_str()), None, "{not_it}");
    }
}

#[test]
fn every_mark_is_one_svg_the_menu_can_draw() {
    for agent in &AGENTS {
        assert!(
            agent.mark.starts_with("<svg") && agent.mark.trim_end().ends_with("</svg>"),
            "{}",
            agent.name
        );
        assert!(!agent.mark.contains("<script"), "{}", agent.name);
    }
}

#[test]
fn the_agent_a_command_line_starts_is_read_as_a_shell_would() {
    let id = |line: &str| of_command(line).map(|agent| agent.id);
    assert_eq!(id("claude"), Some("claude"));
    assert_eq!(id("/opt/homebrew/bin/codex --full-auto"), Some("codex"));
    assert_eq!(id("FOO=1 env -i nohup copilot"), Some("copilot"));
    assert_eq!(id("exec hermes chat"), Some("hermes"));
    assert_eq!(id("ghcs 'list files'"), None);
    assert_eq!(id("hermes-agent --query x"), None);
    // Its name further on is no agent: the program is what runs.
    assert_eq!(id("vim claude.md"), None);
    assert_eq!(id("zsh"), None);
    assert_eq!(id(""), None);
}

#[test]
fn a_page_gets_each_mark_in_the_text_colour() {
    let marks: serde_json::Value = serde_json::from_str(&marks()).unwrap();
    for agent in &AGENTS {
        let svg = marks[agent.id].as_str().unwrap();
        assert!(
            svg.starts_with("<svg") && svg.contains("currentColor"),
            "{}",
            agent.id
        );
        assert!(
            !svg.contains("#000") && !svg.contains("</script"),
            "{}",
            agent.id
        );
    }
}

#[test]
fn the_logins_path_comes_first_and_the_process_keeps_what_it_added() {
    let process = std::env::join_paths(["/usr/bin", "/opt/dev"]).unwrap();
    let merged = merged("/home/me/.local/bin:/usr/bin", &process);
    let dirs: Vec<_> = std::env::split_paths(&merged).collect();
    assert_eq!(
        dirs,
        ["/home/me/.local/bin", "/usr/bin", "/opt/dev"].map(std::path::PathBuf::from)
    );
}

#[test]
fn an_agent_starts_on_a_first_prompt_quoted_for_the_shell_where_its_command_takes_one() {
    let (claude, gemini, copilot) = (&AGENTS[0], &AGENTS[2], &AGENTS[3]);
    assert_eq!(
        claude.seeded("claude", "take c3; it's yours").as_deref(),
        Some(r"claude 'take c3; it'\''s yours'")
    );
    assert_eq!(
        gemini.seeded("gemini", "take c3").as_deref(),
        Some("gemini -i 'take c3'")
    );
    assert_eq!(
        copilot.seeded("copilot", "take c3").as_deref(),
        Some("copilot -i 'take c3'")
    );
    // A command not its own takes no first prompt.
    assert_eq!(claude.seeded("codex", "take c3"), None);
    for agent in &AGENTS {
        assert!(
            (agent.seeds.iter()).all(|(command, _)| agent.commands.contains(command)),
            "{}",
            agent.id
        );
    }
    assert!(
        started_on("nope", "take c3")
            .unwrap_err()
            .contains("no agent `nope`")
    );
}
