//! Tests of reading Claude Code's managed settings for whether it takes a plugin folder: each
//! source a file of a temporary folder, never the machine's own.

use super::{Sources, takes_plugin_folders};
use crate::testing::Scratch;

/// The sources, all in `scratch`: none of them there until a test writes it.
fn sources(scratch: &Scratch) -> Sources {
    Sources {
        remote: scratch.path().join("claude/remote-settings.json"),
        mdm: vec![
            scratch
                .path()
                .join("Managed Preferences/com.anthropic.claudecode.plist"),
        ],
        folder: scratch.path().join("ClaudeCode"),
    }
}

const REFUSES: &str = r#"{"disableSideloadFlags": true}"#;

/// An XML property list of `body`, as a configuration profile delivers it.
#[cfg(target_os = "macos")]
fn plist(body: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>{body}</dict></plist>"#
    )
}

#[test]
fn no_managed_settings_take_plugin_folders() {
    let scratch = Scratch::new();
    assert!(takes_plugin_folders(&sources(&scratch)));
    // A cache of server settings for an organization that set none.
    scratch.file("claude/remote-settings.json", "{}");
    assert!(takes_plugin_folders(&sources(&scratch)));
}

#[test]
fn a_managed_file_that_turns_off_sideloading_refuses() {
    let scratch = Scratch::new();
    scratch.file("ClaudeCode/managed-settings.json", REFUSES);
    assert!(!takes_plugin_folders(&sources(&scratch)));
}

#[test]
fn a_drop_in_file_alone_refuses_and_a_later_one_turns_it_back() {
    let scratch = Scratch::new();
    scratch.file("ClaudeCode/managed-settings.d/20-plugins.json", REFUSES);
    // Hidden files and files that are not JSON are not read.
    scratch.file(
        "ClaudeCode/managed-settings.d/.30-off.json",
        r#"{"disableSideloadFlags": false}"#,
    );
    scratch.file(
        "ClaudeCode/managed-settings.d/40-off.txt",
        r#"{"disableSideloadFlags": false}"#,
    );
    assert!(!takes_plugin_folders(&sources(&scratch)));
    scratch.file(
        "ClaudeCode/managed-settings.d/30-on.json",
        r#"{"disableSideloadFlags": false}"#,
    );
    assert!(takes_plugin_folders(&sources(&scratch)));
}

#[test]
fn under_first_wins_a_higher_source_with_a_policy_hides_a_lower_ones_refusal() {
    let scratch = Scratch::new();
    scratch.file("ClaudeCode/managed-settings.json", REFUSES);
    // A source with only the control keys holds no policy: the next one is read.
    scratch.file(
        "claude/remote-settings.json",
        r#"{"managedSourcesBehavior": "first-wins"}"#,
    );
    assert!(!takes_plugin_folders(&sources(&scratch)));
    scratch.file("claude/remote-settings.json", r#"{"model": "opus"}"#);
    assert!(takes_plugin_folders(&sources(&scratch)));
}

#[test]
fn under_merge_a_refusal_in_any_source_refuses_and_a_lower_source_cannot_ask_for_merge() {
    let scratch = Scratch::new();
    scratch.file("claude/remote-settings.json", r#"{"model": "opus"}"#);
    let lower = r#"{"managedSourcesBehavior": "merge", "disableSideloadFlags": true}"#;
    scratch.file("ClaudeCode/managed-settings.json", lower);
    assert!(takes_plugin_folders(&sources(&scratch)));
    let higher = r#"{"managedSourcesBehavior": "merge", "model": "opus"}"#;
    scratch.file("claude/remote-settings.json", higher);
    assert!(!takes_plugin_folders(&sources(&scratch)));
}

/// A configuration profile is macOS's, read by its `plutil`: elsewhere there is none to read.
#[cfg(target_os = "macos")]
#[test]
fn a_configuration_profile_is_read_as_the_second_source() {
    let scratch = Scratch::new();
    let profile = "Managed Preferences/com.anthropic.claudecode.plist";
    scratch.file(profile, plist("<key>disableSideloadFlags</key><true/>"));
    assert!(!takes_plugin_folders(&sources(&scratch)));
    scratch.file(profile, plist("<key>model</key><string>opus</string>"));
    scratch.file("ClaudeCode/managed-settings.json", REFUSES);
    assert!(takes_plugin_folders(&sources(&scratch)));
}

#[test]
fn a_source_that_is_there_but_cannot_be_read_refuses() {
    let mut unreadable = vec![
        ("ClaudeCode/managed-settings.json", "{ not json"),
        ("ClaudeCode/managed-settings.json", "[true]"),
        ("claude/remote-settings.json", "{"),
    ];
    if cfg!(target_os = "macos") {
        unreadable.push((
            "Managed Preferences/com.anthropic.claudecode.plist",
            "not a property list",
        ));
    }
    for (name, content) in unreadable {
        let scratch = Scratch::new();
        scratch.file(name, content);
        assert!(
            !takes_plugin_folders(&sources(&scratch)),
            "{name}: {content}"
        );
    }
    // A drop-in folder that cannot be listed.
    let scratch = Scratch::new();
    let folder = scratch.path().join("ClaudeCode/managed-settings.d");
    std::fs::create_dir_all(&folder).unwrap();
    let closed = std::os::unix::fs::PermissionsExt::from_mode(0o000);
    std::fs::set_permissions(&folder, closed).unwrap();
    // The superuser lists a folder whatever its mode (a container's tests run so): there is no
    // folder it cannot list, and that case is not checked.
    let listed = std::fs::read_dir(&folder).is_ok();
    let refused = !takes_plugin_folders(&sources(&scratch));
    let open = std::os::unix::fs::PermissionsExt::from_mode(0o755);
    std::fs::set_permissions(&folder, open).unwrap();
    if listed {
        eprintln!("skipped: a folder of mode 000 is listed (the tests run as the superuser)");
        return;
    }
    assert!(refused);
}

#[test]
fn a_policy_a_program_computes_is_not_known_and_refuses() {
    let scratch = Scratch::new();
    scratch.file(
        "ClaudeCode/managed-settings.json",
        r#"{"policyHelper": {"path": "/usr/local/bin/policy"}}"#,
    );
    assert!(!takes_plugin_folders(&sources(&scratch)));
}
