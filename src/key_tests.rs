//! Tests of `key`.

use std::os::unix::fs::PermissionsExt;

use super::*;

#[test]
fn a_saved_key_is_read_back_trimmed_from_a_file_only_its_owner_reads() {
    let dir = crate::testing::Scratch::new();
    let path = dir.path().join("llx-desk").join("openrouter.key");
    assert_eq!(read_at(&path), None);
    save_at(&path, "  sk-or-v1-abc\n").unwrap();
    assert_eq!(read_at(&path).as_deref(), Some("sk-or-v1-abc"));
    let mode = std::fs::metadata(&path).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o600);
    let dir_mode = std::fs::metadata(path.parent().unwrap())
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(dir_mode & 0o777, 0o700);
}

#[test]
fn an_empty_key_is_refused_and_a_blank_file_holds_none() {
    let dir = crate::testing::Scratch::new();
    let path = dir.path().join("openrouter.key");
    assert!(save_at(&path, " \n").is_err());
    std::fs::write(&path, "\n").unwrap();
    assert_eq!(read_at(&path), None);
}

#[test]
fn the_environment_comes_before_the_file() {
    let from = |env: Option<&str>, file: Option<&str>| {
        choose(env.map(String::from), || file.map(String::from))
    };
    assert_eq!(from(Some("env"), Some("file")).as_deref(), Some("env"));
    assert_eq!(from(Some(" "), Some("file")).as_deref(), Some("file"));
    assert_eq!(from(None, Some("file")).as_deref(), Some("file"));
    assert_eq!(from(None, None), None);
}

#[test]
fn the_file_is_under_the_config_directory() {
    assert_eq!(
        path_in(Some("/x".into()), Some("/h".into())),
        Some(PathBuf::from("/x/llx-desk/openrouter.key"))
    );
    assert_eq!(
        path_in(None, Some("/h".into())),
        Some(PathBuf::from("/h/.config/llx-desk/openrouter.key"))
    );
    assert_eq!(path_in(None, None), None);
}
