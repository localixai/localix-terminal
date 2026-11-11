//! Tests of `update::fetch`, against a folder on disk: no test reaches the network.

use std::sync::Mutex;
use std::time::Duration;

use super::*;

const MOST: Duration = Duration::from_secs(20);

/// A folder of this test's own, empty.
pub(in crate::update) fn folder(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("llx-desk-update-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir.canonicalize().unwrap()
}

#[test]
fn a_short_answer_is_read_as_text_and_a_missing_one_says_why() {
    let dir = folder("text");
    std::fs::write(dir.join("latest"), "{\"tag_name\": \"v0.2.0\"}").unwrap();
    let url = format!("file://{}/latest", dir.display());
    assert_eq!(
        text(&url, &dir, MOST).unwrap(),
        "{\"tag_name\": \"v0.2.0\"}"
    );
    let missing = text(&format!("file://{}/none", dir.display()), &dir, MOST).unwrap_err();
    assert!(missing.contains("/none was not fetched: "), "{missing}");
    assert!(!missing.contains("curl: "), "{missing}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_file_is_fetched_whole_with_its_progress_told_and_a_failed_one_leaves_nothing() {
    let dir = folder("file");
    let bytes: Vec<u8> = (0..300_000u32).map(|n| (n % 251) as u8).collect();
    std::fs::write(dir.join("archive"), &bytes).unwrap();
    let to = dir.join("fetched");
    let heard = Mutex::new(Vec::new());
    let told = |so_far: u64| heard.lock().unwrap().push(so_far);
    file(
        &format!("file://{}/archive", dir.display()),
        &to,
        MOST,
        &told,
    )
    .unwrap();
    assert_eq!(std::fs::read(&to).unwrap(), bytes);
    // The last told is all of it.
    assert_eq!(heard.lock().unwrap().last(), Some(&300_000));
    // One that is not there: an error, and no file half written, not even the one before.
    let missing = file(&format!("file://{}/none", dir.display()), &to, MOST, &told);
    assert!(missing.is_err());
    assert!(!to.exists());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn an_address_is_read_only_the_way_it_names() {
    // A plain HTTP address is refused before anything is asked of it.
    let dir = folder("refused");
    let refused = text("http://127.0.0.1:9/latest", &dir, Duration::from_secs(3)).unwrap_err();
    assert!(refused.to_lowercase().contains("protocol"), "{refused}");
    // And nothing of the asking is left behind.
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0);
    let _ = std::fs::remove_dir_all(dir);
}
