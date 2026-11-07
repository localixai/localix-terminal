//! Tests of `agents`.

use std::os::unix::fs::PermissionsExt;

use super::*;
use crate::testing::Scratch;

#[test]
fn an_agent_is_offered_by_the_first_of_its_commands_on_the_path() {
    let scratch = Scratch::new();
    let (bin, other) = (scratch.path().join("bin"), scratch.path().join("other"));
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::create_dir_all(&other).unwrap();
    let path = std::env::join_paths([&bin, &other]).unwrap();
    let copilot = &AGENTS[3];
    assert_eq!(copilot.command_on(&path), None);
    // A file that cannot run is no command.
    std::fs::write(other.join("ghcs"), "#!/bin/sh\n").unwrap();
    assert_eq!(copilot.command_on(&path), None);
    std::fs::set_permissions(other.join("ghcs"), std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(copilot.command_on(&path), Some("ghcs"));
    std::fs::write(bin.join("copilot"), "#!/bin/sh\n").unwrap();
    std::fs::set_permissions(bin.join("copilot"), std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(copilot.command_on(&path), Some("copilot"));
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
    assert_eq!(id("FOO=1 env -i nohup ghcs"), Some("copilot"));
    assert_eq!(id("exec hermes-agent chat"), Some("hermes"));
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
