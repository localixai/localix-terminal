//! Tests of the plugin's copy in the app's folder: made once per version, whole or not at all,
//! the copies of other versions let go.

use super::kept;
use crate::testing::Scratch;

/// A plugin at `scratch/source`, its manifest saying `version`, with a program, a link and what
/// a copy leaves out.
fn plugin(scratch: &Scratch, version: &str) -> std::path::PathBuf {
    let manifest = format!(r#"{{"name": "llxterm", "version": "{version}"}}"#);
    scratch.file("source/.claude-plugin/plugin.json", manifest);
    let program = scratch.file("source/bin/llxterm", "#!/bin/sh\n");
    let executable = std::os::unix::fs::PermissionsExt::from_mode(0o755);
    std::fs::set_permissions(&program, executable).unwrap();
    scratch.file("source/node_modules/x/index.js", "x");
    scratch.file("source/.claude-plugin/types/api.d.ts", "x");
    scratch.file("source/.DS_Store", "x");
    let link = scratch.path().join("source/skills");
    std::fs::create_dir_all(scratch.path().join("source/real-skills")).unwrap();
    let _ = std::os::unix::fs::symlink("real-skills", &link);
    scratch.path().join("source")
}

#[test]
fn the_plugin_is_copied_whole_once_per_version_and_other_versions_go() {
    let scratch = Scratch::new();
    let source = plugin(&scratch, "1.0.0");
    let under = scratch.path().join("claude-code");
    let copy = kept(&source, &under, "1.0.0", false).unwrap();
    assert_eq!(copy, under.join("1.0.0"));
    assert!(copy.join(".claude-plugin/plugin.json").is_file());
    let mode = std::os::unix::fs::PermissionsExt::mode(
        &std::fs::metadata(copy.join("bin/llxterm"))
            .unwrap()
            .permissions(),
    );
    assert_eq!(mode & 0o111, 0o111, "the program stays a program");
    assert!(
        std::fs::symlink_metadata(copy.join("skills"))
            .unwrap()
            .is_symlink()
    );
    for left_out in ["node_modules", ".claude-plugin/types", ".DS_Store"] {
        assert!(!copy.join(left_out).exists(), "{left_out}");
    }

    // What Claude Code writes into its copy stays: the copy is not made again for its version.
    scratch.file("claude-code/1.0.0/.claude-plugin/types/api.d.ts", "written");
    kept(&source, &under, "1.0.0", false).unwrap();
    assert!(copy.join(".claude-plugin/types/api.d.ts").exists());

    // Another version is a copy of its own, and the old one goes.
    let newer = kept(&source, &under, "1.1.0", false).unwrap();
    assert!(newer.join(".claude-plugin/plugin.json").is_file());
    assert!(!copy.exists());
}

#[test]
fn a_copy_cut_short_is_never_used_and_a_copy_asked_again_is_made_again() {
    let scratch = Scratch::new();
    let source = plugin(&scratch, "1.0.0");
    let under = scratch.path().join("claude-code");
    // A copy left half made by a run that ended: its name is a draft's, never the version's.
    scratch.file("claude-code/.1.0.0.99999/bin/llxterm", "half");
    let copy = kept(&source, &under, "1.0.0", false).unwrap();
    assert!(copy.join(".claude-plugin/plugin.json").is_file());
    // A folder of the version without a manifest is not a copy.
    std::fs::remove_file(copy.join(".claude-plugin/plugin.json")).unwrap();
    kept(&source, &under, "1.0.0", false).unwrap();
    assert!(copy.join(".claude-plugin/plugin.json").is_file());
    // Asked again (a plugin of a source tree, which changes under one version), it is new.
    scratch.file("source/skills-new.md", "new");
    kept(&source, &under, "1.0.0", true).unwrap();
    assert!(copy.join("skills-new.md").is_file());
    let names: Vec<String> = (std::fs::read_dir(&under).unwrap())
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| !name.starts_with(".1.0.0.99999"))
        .collect();
    assert_eq!(names, ["1.0.0"], "no draft and no old copy is left");
}
