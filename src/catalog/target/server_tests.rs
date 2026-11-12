//! Tests of `server`: the key a server is kept by, how an entry is known to be a listing's, what
//! the person must give first, and that a secret is never said.

use super::super::tests::{command, env_at, need, remote, server, target};
use super::{Found, Seen, found, key, line, masked, unversioned, wanted};
use crate::catalog::{Env, Listing, With};
use crate::testing::Scratch;

fn with(values: &[(&str, &str)]) -> With {
    With {
        values: (values.iter())
            .map(|(name, value)| (name.to_string(), value.to_string()))
            .collect(),
        ..With::default()
    }
}

fn seen(command: &str, args: &[&str]) -> Seen {
    Seen {
        command: Some(command.into()),
        args: args.iter().map(|arg| arg.to_string()).collect(),
        url: None,
    }
}

/// A server that wants a secret key and, if given, a region.
fn weather() -> Listing {
    let needs = vec![need("API_KEY", "required secret"), need("REGION", "")];
    server(
        "io.github.acme/weather",
        command("npx", &["-y", "@acme/weather@1.0.0"]),
        needs,
    )
}

#[test]
fn a_server_is_kept_by_the_last_part_of_its_name() {
    assert_eq!(
        key("io.github.acme/weather-server").unwrap(),
        "weather-server"
    );
    assert_eq!(key("Weather Server!").unwrap(), "Weather-Server");
    assert_eq!(key("acme/my.tool_v2").unwrap(), "my-tool_v2");
    assert_eq!(key(&"a".repeat(80)).unwrap().len(), 64);
    assert!(key("acme/...").is_err());
    assert!(key("").is_err());
}

#[test]
fn a_version_in_an_argument_is_read_as_a_package_runner_reads_it() {
    assert_eq!(
        unversioned("@acme/weather@1.2.0"),
        ("@acme/weather", "1.2.0")
    );
    assert_eq!(unversioned("weather@latest"), ("weather", "latest"));
    assert_eq!(unversioned("weather==1.2"), ("weather", "1.2"));
    assert_eq!(unversioned("@acme/weather"), ("@acme/weather", ""));
    assert_eq!(unversioned("user@host"), ("user@host", ""));
}

#[test]
fn an_entry_is_the_listings_by_what_it_runs() {
    let runs = command("npx", &["-y", "@acme/weather@1.2.0"]);
    assert_eq!(
        found(&seen("npx", &["-y", "@acme/weather@1.2.0"]), &runs),
        Found::Same
    );
    let older = seen("npx", &["-y", "@acme/weather@1.0.0"]);
    assert_eq!(found(&older, &runs), Found::Older("1.0.0".into()));
    assert_eq!(
        found(&seen("npx", &["-y", "@acme/other@1.2.0"]), &runs),
        Found::Other
    );
    assert_eq!(
        found(&seen("uvx", &["-y", "@acme/weather@1.2.0"]), &runs),
        Found::Other
    );
    assert_eq!(
        found(&seen("npx", &["@acme/weather@1.2.0"]), &runs),
        Found::Other
    );
    let url = remote("https://w.example/mcp", false);
    let at = Seen {
        url: Some("https://w.example/mcp".into()),
        ..Seen::default()
    };
    assert_eq!(found(&at, &url), Found::Same);
    let elsewhere = Seen {
        url: Some("https://x.example/mcp".into()),
        ..Seen::default()
    };
    assert_eq!(found(&elsewhere, &url), Found::Other);
}

#[test]
fn an_entry_is_read_in_each_agents_json_shape() {
    let entry = serde_json::json!({"type": "stdio", "command": "npx", "args": ["w"]});
    assert_eq!(Seen::of(&entry), Some(seen("npx", &["w"])));
    let local = serde_json::json!({"type": "local", "command": ["npx", "w"], "enabled": true});
    assert_eq!(Seen::of(&local), Some(seen("npx", &["w"])));
    let http = serde_json::json!({"httpUrl": "https://w.example/mcp"});
    assert_eq!(
        Seen::of(&http).unwrap().url.as_deref(),
        Some("https://w.example/mcp")
    );
    assert_eq!(Seen::of(&serde_json::json!({"type": "stdio"})), None);
}

#[test]
fn what_it_needs_is_asked_by_name_before_anything_is_written() {
    let listing = weather();
    let why = wanted(&listing, &With::default()).err().unwrap();
    assert_eq!(why, "API_KEY is needed first: the API_KEY");
    let scratch = Scratch::new();
    let env = env_at(scratch.path());
    let refused = target("opencode").install(&listing, &With::default(), &env);
    assert_eq!(refused.unwrap_err(), "API_KEY is needed first: the API_KEY");
    assert!(!scratch.path().join(".config").exists());
    // An optional one left out is left out; a default stands in for one not given.
    let mut listing = weather();
    if let crate::catalog::Recipe::Server(server) = &mut listing.recipe {
        server.needs[1].default = "eu".into();
    }
    let given = wanted(&listing, &with(&[("API_KEY", "sk-1")])).unwrap();
    assert_eq!(
        given.env,
        [
            ("API_KEY".into(), "sk-1".into()),
            ("REGION".into(), "eu".into())
        ]
    );
    assert_eq!(given.secrets, ["sk-1"]);
}

#[test]
fn a_bad_name_or_program_or_address_is_refused() {
    let bad_need = server("w", command("npx", &["w"]), vec![need("A KEY", "required")]);
    assert_eq!(
        wanted(&bad_need, &With::default()).err().unwrap(),
        "`A KEY` is no name for a value"
    );
    let flag = server("w", command("--eval", &[]), vec![]);
    assert!(wanted(&flag, &With::default()).is_err());
    let file = server("w", remote("file:///etc/passwd", false), vec![]);
    assert_eq!(
        wanted(&file, &With::default()).err().unwrap(),
        "`file:///etc/passwd` is no web address"
    );
    let header = server(
        "w",
        remote("https://w.example/mcp", false),
        vec![need("X-Key", "required secret header")],
    );
    let two_lines = wanted(&header, &with(&[("X-Key", "a\nInjected: b")]));
    assert_eq!(two_lines.err().unwrap(), "X-Key is more than one line");
}

#[test]
fn a_plan_names_what_is_written_and_never_a_value() {
    let scratch = Scratch::new();
    let env = env_at(scratch.path());
    let plan = target("opencode").plan(&weather(), &env).unwrap();
    assert_eq!(
        plan.steps,
        [
            "Add the server `weather` to ~/.config/opencode/opencode.json: npx -y @acme/weather@1.0.0",
            "With: API_KEY=…",
            "With: REGION=… (if given)",
        ]
    );
    assert_eq!(plan.after, "Start OpenCode again to use it");
    let header = server(
        "w",
        remote("https://w.example/mcp", true),
        vec![need("Authorization", "required secret header")],
    );
    let plan = target("claude").plan(&header, &env).unwrap();
    assert_eq!(
        plan.steps,
        [
            "Add the server `w` to ~/.claude.json: https://w.example/mcp (server-sent events)",
            "With the header: Authorization: …",
        ]
    );
}

#[test]
fn a_command_line_is_shown_quoted_as_a_shell_reads_it() {
    let runs = command("uvx", &["my tool", "it's", "$HOME", ""]);
    assert_eq!(line(&runs), r#"uvx 'my tool' 'it'\''s' '$HOME' ''"#);
}

#[test]
fn a_secret_is_masked_in_what_a_program_says() {
    let listing = weather();
    let given = wanted(
        &listing,
        &with(&[("API_KEY", "sk-secret"), ("REGION", "eu")]),
    )
    .unwrap();
    let said = masked("bad entry: API_KEY=sk-secret REGION=eu", &given);
    assert_eq!(said, "bad entry: API_KEY=… REGION=eu");
}

#[test]
#[cfg(unix)]
fn a_program_that_fails_is_told_without_the_secret_it_was_given() {
    use std::os::unix::fs::PermissionsExt;
    let scratch = Scratch::new();
    // A stand-in for Claude Code's program, which says back what it was given.
    let program = scratch.file(
        "bin/claude",
        "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$HOME/said\"\necho \"refused: $*\" >&2\nexit 1\n",
    );
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
    let env = Env {
        path: scratch.path().join("bin").display().to_string(),
        ..env_at(scratch.path())
    };
    let given = with(&[("API_KEY", "sk-secret")]);
    let why = target("claude")
        .install(&weather(), &given, &env)
        .unwrap_err();
    assert!(
        why.starts_with("refused: mcp add-json --scope user weather"),
        "{why}"
    );
    assert!(!why.contains("sk-secret"), "{why}");
    let said = std::fs::read_to_string(scratch.path().join("said")).unwrap();
    let words: Vec<&str> = said.lines().collect();
    assert_eq!(
        words[..5],
        ["mcp", "add-json", "--scope", "user", "weather"]
    );
    let entry: serde_json::Value = serde_json::from_str(words[5]).unwrap();
    assert_eq!(
        entry,
        serde_json::json!({
            "type": "stdio",
            "command": "npx",
            "args": ["-y", "@acme/weather@1.0.0"],
            "env": {"API_KEY": "sk-secret"},
        })
    );
}

#[test]
fn an_entry_of_the_persons_by_that_name_is_left_as_it_is() {
    let scratch = Scratch::new();
    let env = env_at(scratch.path());
    let theirs = r#"{"mcpServers": {"weather": {"command": "node", "args": ["mine.js"]}}}"#;
    scratch.file(".claude.json", theirs);
    let why = "~/.claude.json already has a server `weather` that LLX Desk did not add: it is left as it is";
    let claude = target("claude");
    assert_eq!(claude.plan(&weather(), &env).unwrap_err(), why);
    let given = with(&[("API_KEY", "sk-1")]);
    assert_eq!(claude.install(&weather(), &given, &env).unwrap_err(), why);
    assert_eq!(claude.remove(&weather(), &env).unwrap_err(), why);
    assert_eq!(claude.has(&weather(), &env), None);
}

#[test]
fn an_older_entry_is_had_at_its_version_and_planned_as_replaced() {
    let scratch = Scratch::new();
    let env = env_at(scratch.path());
    let older = r#"{"mcpServers": {"weather": {"type": "stdio", "command": "npx", "args": ["-y", "@acme/weather@0.9.0"]}}}"#;
    scratch.file(".claude.json", older);
    assert_eq!(
        target("claude").has(&weather(), &env).as_deref(),
        Some("0.9.0")
    );
    let plan = target("claude").plan(&weather(), &env).unwrap();
    assert!(
        plan.steps[0].starts_with("Replace the server `weather` (version 0.9.0) in ~/.claude.json")
    );
}
