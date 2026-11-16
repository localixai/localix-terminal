//! Tests of the settings layers the app names for Gemini CLI and OpenCode.

use super::layers;
use crate::testing::Scratch;

#[test]
fn each_agent_is_named_a_file_of_the_apps_that_declares_its_server_by_the_bare_command() {
    let scratch = Scratch::new();
    let vars = layers(&|_| None, scratch.path());
    let names: Vec<&str> = vars.iter().map(|(name, _)| name.as_str()).collect();
    // The machine's own Gemini CLI defaults, where there are some, are not hidden.
    let machine =
        std::path::Path::new("/Library/Application Support/GeminiCli/system-defaults.json");
    let expected: &[&str] = if machine.exists() {
        &["OPENCODE_CONFIG"]
    } else {
        &["GEMINI_CLI_SYSTEM_DEFAULTS_PATH", "OPENCODE_CONFIG"]
    };
    assert_eq!(names, expected);
    for (name, file) in &vars {
        let json: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(file).unwrap()).unwrap();
        let server = match name.as_str() {
            "OPENCODE_CONFIG" => json["mcp"]["llxterm"]["command"].clone(),
            _ => serde_json::json!([
                json["mcpServers"]["llxterm"]["command"],
                json["mcpServers"]["llxterm"]["args"][0]
            ]),
        };
        assert_eq!(server, serde_json::json!(["llxterm", "mcp"]), "{name}");
        assert!(
            file.starts_with(&scratch.path().display().to_string()),
            "{file}"
        );
    }
}

#[test]
fn a_variable_the_person_set_is_left_as_it_is() {
    let scratch = Scratch::new();
    let theirs = |name: &str| (name == "OPENCODE_CONFIG").then(|| "/mine.json".to_string());
    let vars = layers(&theirs, scratch.path());
    assert!(
        vars.iter().all(|(name, _)| name != "OPENCODE_CONFIG"),
        "{vars:?}"
    );
    assert!(!scratch.path().join("opencode").exists());
}
