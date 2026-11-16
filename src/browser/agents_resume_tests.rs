//! Tests of `agents_resume`: what of an agent's command line is carried into the command that
//! resumes its session, and what never is.

use std::os::unix::fs::PermissionsExt;

use super::*;
use crate::testing::Scratch;

const ID: &str = "0b8f6a52-6c1e-4b5e-9a57-3d5f0c2e9a11";

fn argv(line: &str) -> Vec<String> {
    line.split_whitespace().map(str::to_string).collect()
}

fn read(line: &str) -> Option<Launch> {
    launch(&argv(line))
}

#[test]
fn claude_names_its_session_by_resume_or_session_id_and_keeps_only_what_the_list_allows() {
    let read = read(&format!(
        "/Users/a/.local/bin/claude --resume {ID} --model opus --effort high \
         --append-system-prompt be-brief --settings {{\"apiKey\":\"x\"}} fix the cart"
    ))
    .unwrap();
    assert_eq!(read.agent, "claude");
    assert_eq!(read.session.as_deref(), Some(ID));
    assert_eq!(read.kept, argv("--model opus --effort high"));
    assert!(!read.skips && !read.bypass);
    let pinned = launch(&argv(&format!("claude --session-id {ID}"))).unwrap();
    assert_eq!(pinned.session.as_deref(), Some(ID));
    let inline = launch(&argv(&format!("claude --resume={ID} -c"))).unwrap();
    assert_eq!(inline.session.as_deref(), Some(ID));
    // Without an id, or with a name in its place, the session is not known from the line.
    assert_eq!(read_session("claude"), None);
    assert_eq!(read_session("claude --continue"), None);
    assert_eq!(read_session("claude --resume cart-work"), None);
    // A fork is another session than the one it names.
    assert_eq!(
        read_session(&format!("claude --resume {ID} --fork-session")),
        None
    );
}

fn read_session(line: &str) -> Option<String> {
    read(line).and_then(|read| read.session)
}

#[test]
fn a_run_that_prints_and_ends_or_a_command_of_its_own_is_never_resumed() {
    for line in [
        "claude -p hello",
        "claude --print --output-format json hi",
        "claude --no-session-persistence",
        "claude --bg",
        "claude mcp list",
        "claude update",
        "codex exec fix-it",
        "codex review",
        "codex fork --last",
        "copilot -p hi",
        "copilot config",
        "opencode run hello",
        "opencode serve",
        "gemini -p hi",
        "gemini mcp list",
        "vim claude.md",
        "zsh",
    ] {
        assert!(read(line).is_none(), "{line}");
    }
    assert!(launch(&[]).is_none());
}

#[test]
fn a_prompt_is_never_carried_even_after_a_flag_that_takes_no_value() {
    let read = read("claude --verbose write the tests -- --model x").unwrap();
    assert_eq!(read.kept, argv("--verbose"));
    // A value that is not one a flag takes is dropped with it.
    let read =
        read_line("claude --effort extreme --model sk-ant-api03-abcdef --permission-mode yes");
    assert!(read.kept.is_empty(), "{:?}", read.kept);
}

fn read_line(line: &str) -> Launch {
    read(line).unwrap()
}

#[test]
fn the_permission_skip_is_carried_and_marked_and_a_sandbox_bypass_never_is() {
    let read = read_line(&format!(
        "claude --resume {ID} --dangerously-skip-permissions"
    ));
    assert_eq!(read.kept, argv("--dangerously-skip-permissions"));
    assert!(read.skips);
    let mode = read_line("claude --permission-mode bypassPermissions");
    assert_eq!(mode.kept, argv("--permission-mode bypassPermissions"));
    assert!(mode.skips);
    let codex = read_line(&format!(
        "codex resume {ID} -m gpt-5 --dangerously-bypass-approvals-and-sandbox -s danger-full-access -c token=secret"
    ));
    assert_eq!(codex.session.as_deref(), Some(ID));
    assert_eq!(codex.kept, argv("-m gpt-5"));
    assert!(codex.bypass && !codex.skips);
    let sandboxed = read_line("codex -s workspace-write -a on-request");
    assert_eq!(sandboxed.kept, argv("-s workspace-write -a on-request"));
    assert!(!sandboxed.bypass && sandboxed.session.is_none());
}

#[test]
fn each_agent_with_a_session_of_its_own_names_it_its_own_way() {
    let copilot = read_line(&format!(
        "copilot --resume={ID} --model gpt-5 --allow-all-tools"
    ));
    assert_eq!(
        (copilot.agent, copilot.session.as_deref()),
        ("copilot", Some(ID))
    );
    assert_eq!(copilot.kept, argv("--model gpt-5 --allow-all-tools"));
    assert!(copilot.skips);
    let opencode = read_line("opencode -s ses_4fK2mQ9xZ1 -m anthropic/claude --prompt hi");
    assert_eq!(opencode.session.as_deref(), Some("ses_4fK2mQ9xZ1"));
    assert_eq!(opencode.kept, argv("-m anthropic/claude"));
    // Gemini CLI's `--resume` takes `latest` or a number, no session's id.
    let gemini = read_line("gemini --resume latest -m gemini-2.5-pro");
    assert_eq!((gemini.agent, gemini.session), ("gemini", None));
}

#[test]
fn the_command_that_resumes_is_built_again_from_what_was_kept() {
    let scratch = Scratch::new();
    let bin = scratch.path().join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    for name in ["claude", "codex"] {
        std::fs::write(bin.join(name), "#!/bin/sh\n").unwrap();
        std::fs::set_permissions(bin.join(name), std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let path = bin.as_os_str();
    let kept = argv("--model opus --dangerously-skip-permissions");
    let claude = resumed("claude", ID, &kept, path).unwrap();
    assert_eq!(
        claude.line,
        format!("claude --resume {ID} --model opus --dangerously-skip-permissions")
    );
    assert_eq!(
        claude.said(false),
        "Resumed Claude Code session · permissions skipped"
    );
    // What a row of the store holds is read through the list again: nothing else gets in.
    let planted = argv("--model opus;rm -rf / --settings x");
    let line = resumed("claude", ID, &planted, path).unwrap().line;
    assert_eq!(line, format!("claude --resume {ID}"));
    let codex = resumed("codex", ID, &argv("-m gpt-5"), path).unwrap();
    assert_eq!(codex.line, format!("codex resume {ID} -m gpt-5"));
    assert_eq!(codex.said(false), "Resumed Codex session");
    assert_eq!(
        codex.said(true),
        "Resumed Codex session · sandbox bypass not carried"
    );
    // An id not in the agent's form, an agent not installed or not known: nothing to type.
    assert!(resumed("claude", "x; rm -rf ~", &[], path).is_err());
    assert!(resumed("copilot", ID, &[], path).is_err());
    assert!(resumed("gemini", ID, &[], path).is_err());
    assert!(resumed("nobody", ID, &[], path).is_err());
    assert!(names_session("claude", ID) && names_session("opencode", "ses_4fK2mQ9xZ1"));
    assert!(!names_session("claude", "ses_4fK2mQ9xZ1") && !names_session("gemini", ID));
}

#[test]
fn options_before_a_first_word_leave_it_the_first_so_a_command_or_a_resume_is_still_read() {
    let resumed = read_line(&format!("codex -m o3 resume {ID}"));
    assert_eq!(resumed.session.as_deref(), Some(ID));
    assert_eq!(resumed.kept, argv("-m o3"));
    for line in [
        "codex -m o3 exec fix-it",
        "claude --model x mcp serve",
        "claude --verbose update",
    ] {
        assert!(read(line).is_none(), "{line}");
    }
    // A prompt after options is still a prompt, never carried.
    assert_eq!(
        read_line("claude --model x fix the cart").kept,
        argv("--model x")
    );
}

#[test]
fn a_folder_is_held_to_its_form_alone_and_never_looked_for_on_the_disk() {
    let scratch = Scratch::new();
    // A folder that is there, its name with terminal controls in it (^U erases the line a
    // shell holds, ^O runs it in some), is never carried nor typed.
    let tricky = scratch.path().join("a\u{15}b\u{0f}c");
    std::fs::create_dir_all(&tricky).unwrap();
    let tricky = tricky.display().to_string();
    for value in [
        tricky.as_str(),
        "/tmp/x\u{202e}gpj.sh",
        "/tmp/zero\u{200b}width",
        "/work/../etc",
        "relative/dir",
    ] {
        let line = vec!["claude".into(), "--add-dir".into(), value.to_string()];
        assert!(launch(&line).unwrap().kept.is_empty(), "{value:?}");
    }
    // A plain absolute folder is carried by its form: nothing is asked of the disk (one under
    // Documents would make macOS ask the person as the app starts).
    let away = read_line("claude --add-dir /nowhere/that/is/there --plugin-dir /also/not");
    assert_eq!(
        away.kept,
        argv("--add-dir /nowhere/that/is/there --plugin-dir /also/not")
    );
    // A row of the store that holds such a word types nothing of it.
    let bin = scratch.path().join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::write(bin.join("claude"), "#!/bin/sh\n").unwrap();
    std::fs::set_permissions(bin.join("claude"), std::fs::Permissions::from_mode(0o755)).unwrap();
    let planted = vec!["--add-dir".to_string(), tricky];
    let line = resumed("claude", ID, &planted, bin.as_os_str())
        .unwrap()
        .line;
    assert_eq!(line, format!("claude --resume {ID}"));
    assert_eq!(quoted("/a b\u{15}"), None);
    assert_eq!(quoted("/a b").as_deref(), Some("'/a b'"));
}

#[test]
fn a_new_session_from_plus_carries_an_id_of_its_own_where_the_agent_takes_one() {
    let line = pinned("claude").unwrap();
    let id = line.strip_prefix("claude --session-id ").unwrap();
    assert!(is_uuid(id), "{line}");
    assert_ne!(pinned("claude"), pinned("claude"));
    assert!(
        pinned("copilot")
            .unwrap()
            .starts_with("copilot --session-id ")
    );
    // Where the agent has no way to name a new session, its command stays as it is; and a line
    // that is more than the agent's command is left alone.
    assert_eq!(pinned("codex"), None);
    assert_eq!(pinned("claude --continue"), None);
    assert_eq!(pinned("zsh"), None);
}
