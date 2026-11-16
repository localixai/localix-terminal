//! Tests of what the app's terminals are told: where the app's command is, and how the Claude
//! Code plugin's folder joins the ones the person names.

use std::path::{Path, PathBuf};

use super::claude::joined;
use super::command_dir_of;

#[test]
fn the_command_is_the_bundles_link_in_an_app_and_the_program_beside_it_elsewhere() {
    let resources = PathBuf::from("/Applications/Localix Terminal.app/Contents/Resources");
    assert_eq!(
        command_dir_of(Some(resources.clone()), None),
        Some(resources.join("bin"))
    );
    // A build: the folder of the program that runs (its links resolved).
    let exe = std::env::current_exe().unwrap();
    let real = std::fs::canonicalize(&exe).unwrap();
    assert_eq!(command_dir_of(None, Some(exe)).as_deref(), real.parent());
    assert_eq!(command_dir_of(None, None), None);
}

#[test]
fn the_plugin_folder_joins_the_persons_and_is_never_named_twice() {
    let ours = "/data/claude-code/1.0.0";
    assert_eq!(joined(None, ours, true).as_deref(), Some(ours));
    assert_eq!(
        joined(Some("/mine".into()), ours, true).as_deref(),
        Some("/mine:/data/claude-code/1.0.0")
    );
    let both = "/mine:/data/claude-code/1.0.0";
    assert_eq!(joined(Some(both.into()), ours, true).as_deref(), Some(both));
}

#[test]
fn where_plugin_folders_are_refused_the_apps_is_taken_out_and_the_persons_left() {
    let ours = "/data/claude-code/1.0.0";
    assert_eq!(joined(None, ours, false), None);
    assert_eq!(joined(Some("/mine".into()), ours, false), None);
    // An environment that came through a terminal of the app's already names it.
    let both = "/mine:/data/claude-code/1.0.0";
    assert_eq!(
        joined(Some(both.into()), ours, false).as_deref(),
        Some("/mine")
    );
    assert_eq!(joined(Some(ours.into()), ours, false).as_deref(), Some(""));
}

#[test]
fn a_folder_with_a_separator_in_its_name_is_never_named() {
    let odd = Path::new("/data/a:b/claude-code");
    assert_eq!(joined(None, &odd.display().to_string(), true), None);
}
