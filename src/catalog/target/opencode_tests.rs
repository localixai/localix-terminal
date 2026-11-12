//! Tests of `opencode`: its servers written into its file, as no program of its writes them, and
//! taken out again, with the rest of the file kept.

use serde_json::{Value, json};

use super::super::tests::{command, env_at, need, remote, server, target};
use crate::catalog::{Env, With};
use crate::testing::Scratch;

fn file(scratch: &Scratch) -> Value {
    serde_json::from_str(
        &std::fs::read_to_string(scratch.path().join("xdg/opencode/opencode.json")).unwrap(),
    )
    .unwrap()
}

fn env(scratch: &Scratch) -> Env {
    Env {
        vars: [(
            "XDG_CONFIG_HOME".to_string(),
            scratch.path().join("xdg").display().to_string(),
        )]
        .into(),
        ..env_at(scratch.path())
    }
}

#[test]
fn a_server_is_written_in_and_taken_out_with_the_rest_kept() {
    let scratch = Scratch::new();
    let env = env(&scratch);
    scratch.file(
        "xdg/opencode/opencode.json",
        r#"{"theme": "dark", "mcp": {"mine": {"type": "local", "command": ["mine"]}}}"#,
    );
    let listing = server(
        "acme/weather",
        command("npx", &["-y", "@acme/weather"]),
        vec![need("API_KEY", "required secret")],
    );
    let given = With {
        values: [("API_KEY".to_string(), "k".to_string())].into(),
        ..With::default()
    };
    let done = target("opencode").install(&listing, &given, &env).unwrap();
    assert_eq!(
        done,
        "Added the server `weather` in OpenCode: start OpenCode again to use it."
    );
    let want = json!({"type": "local", "command": ["npx", "-y", "@acme/weather"], "enabled": true, "environment": {"API_KEY": "k"}});
    assert_eq!(file(&scratch)["mcp"]["weather"], want);
    assert_eq!(file(&scratch)["theme"], "dark");
    assert_eq!(
        target("opencode").has(&listing, &env).as_deref(),
        Some("1.0.0")
    );
    // Again: written over with what is given now.
    let given = With {
        values: [("API_KEY".to_string(), "k2".to_string())].into(),
        ..With::default()
    };
    let again = target("opencode").install(&listing, &given, &env).unwrap();
    assert!(again.starts_with("Updated the server `weather`"), "{again}");
    assert_eq!(
        file(&scratch)["mcp"]["weather"]["environment"]["API_KEY"],
        "k2"
    );
    let gone = target("opencode").remove(&listing, &env).unwrap();
    assert_eq!(gone, "Took the server `weather` out of OpenCode.");
    assert_eq!(
        file(&scratch)["mcp"],
        json!({"mine": {"type": "local", "command": ["mine"]}})
    );
    assert_eq!(target("opencode").has(&listing, &env), None);
    assert_eq!(
        target("opencode").remove(&listing, &env).unwrap(),
        "OpenCode has no server `weather`."
    );
}

#[test]
fn an_address_is_written_as_a_remote_entry_with_its_headers() {
    let scratch = Scratch::new();
    let env = env(&scratch);
    let listing = server(
        "w",
        remote("https://w.example/mcp", false),
        vec![need("Authorization", "secret header")],
    );
    let given = With {
        values: [("Authorization".to_string(), "Bearer t".to_string())].into(),
        ..With::default()
    };
    target("opencode").install(&listing, &given, &env).unwrap();
    let want = json!({"type": "remote", "url": "https://w.example/mcp", "enabled": true, "headers": {"Authorization": "Bearer t"}});
    assert_eq!(file(&scratch)["mcp"]["w"], want);
}

#[test]
fn the_persons_entry_of_that_name_is_never_written_over() {
    let scratch = Scratch::new();
    let env = env(&scratch);
    let theirs = r#"{"mcp": {"weather": {"type": "local", "command": ["node", "mine.js"]}}}"#;
    scratch.file("xdg/opencode/opencode.json", theirs);
    let listing = server("weather", command("npx", &["w"]), vec![]);
    assert!(
        target("opencode")
            .install(&listing, &With::default(), &env)
            .is_err()
    );
    assert!(target("opencode").remove(&listing, &env).is_err());
    let kept = std::fs::read_to_string(scratch.path().join("xdg/opencode/opencode.json")).unwrap();
    assert_eq!(kept, theirs);
}

#[test]
fn skills_go_beside_its_file() {
    let scratch = Scratch::new();
    let plan = target("opencode")
        .plan(&super::super::tests::skill("pdf"), &env(&scratch))
        .unwrap();
    assert_eq!(plan.steps, ["Copy the skill to ~/xdg/opencode/skills/pdf"]);
    let plan = target("opencode")
        .plan(&super::super::tests::skill("pdf"), &env_at(scratch.path()))
        .unwrap();
    assert_eq!(
        plan.steps,
        ["Copy the skill to ~/.config/opencode/skills/pdf"]
    );
}
