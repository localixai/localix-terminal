//! Tests of `params`.

use super::*;

fn caller() -> Caller {
    Caller {
        project: PathBuf::from("/p"),
        agent: None,
    }
}

#[test]
fn a_call_carries_an_idempotency_key_and_its_environment_only_when_it_starts_a_program() {
    let listing = window_params(&caller(), "/p", &json!({"action": "list"}));
    assert!(listing["env"].is_null());
    assert_eq!(listing["idempotencyKey"].as_str().unwrap().len(), 32);
    let other = window_params(&caller(), "/p", &json!({"action": "list"}));
    assert_ne!(listing["idempotencyKey"], other["idempotencyKey"]);
    let opening = window_params(
        &caller(),
        "/p",
        &json!({"action": "open", "kind": "terminal", "command": "sh"}),
    );
    assert!(opening["env"].is_object());
    // A word in a value is no action.
    let filling = window_params(
        &caller(),
        "/p",
        &json!({"action": "interact", "fill": {"e1": "open"}}),
    );
    assert!(filling["env"].is_null());
}

#[test]
fn the_variables_an_integration_names_its_own_stay_out() {
    let vars = [
        "CLAUDECODE",
        "CLAUDE_PLUGIN_ROOT",
        "CLAUDE_CODE_USE_BEDROCK",
        "PATH",
        "LLXTERM_SCRUB",
    ]
    .map(|name| (name.to_string(), "1".to_string()));
    let kept = scrubbed_of(vars.into_iter(), "CLAUDECODE, CLAUDE_PLUGIN_*,*");
    let names: Vec<_> = kept.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(names, ["CLAUDE_CODE_USE_BEDROCK", "PATH"]);
}

#[test]
fn a_call_from_a_folder_of_no_repository_in_a_terminals_project_is_that_projects() {
    let scratch = crate::testing::Scratch::new();
    let project = scratch.path().join("notes");
    let deeper = project.join("2026/november");
    std::fs::create_dir_all(&deeper).unwrap();
    let pane = Some(project.clone().into_os_string());
    assert_eq!(project_of(&deeper, None, pane.clone()), project);
    // Without the terminal's word, the folder is a project of its own.
    assert_eq!(project_of(&deeper, None, None), deeper);
    // A folder outside the terminal's project is not of it.
    let elsewhere = scratch.path().join("elsewhere");
    std::fs::create_dir_all(&elsewhere).unwrap();
    assert_eq!(project_of(&elsewhere, None, pane), elsewhere);
}

#[test]
fn a_call_inside_a_repository_keeps_it_and_a_named_project_comes_first() {
    let scratch = crate::testing::Scratch::new();
    let project = scratch.path().join("notes");
    let repository = project.join("site");
    std::fs::create_dir_all(repository.join(".git")).unwrap();
    std::fs::create_dir_all(repository.join("src")).unwrap();
    let pane = Some(project.into_os_string());
    assert_eq!(
        project_of(&repository.join("src"), None, pane.clone()),
        repository
    );
    let named = Some(std::ffi::OsString::from("/elsewhere"));
    assert_eq!(
        project_of(&repository, named, pane),
        PathBuf::from("/elsewhere")
    );
}
