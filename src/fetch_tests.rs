//! Tests of `fetch`, against a folder on disk: no test reaches the network.

use std::sync::Mutex;
use std::time::Duration;

use super::*;

const MOST: Duration = Duration::from_secs(20);

/// A folder of this test's own, empty.
pub(crate) fn folder(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("llx-desk-fetch-{}-{name}", std::process::id()));
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

/// The arguments of `command`, as text.
fn args(command: &Command) -> Vec<String> {
    (command.get_args())
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect()
}

#[test]
fn a_short_answer_is_asked_for_again_only_if_it_changed_and_no_larger_than_allowed() {
    let dir = folder("asking");
    let (body, tag) = (dir.join("body"), dir.join("tag"));
    let ask = Ask {
        within: MOST,
        most: 300_000,
        etag: Some("\"e7938c50\""),
    };
    let asked = args(&asking("https://example.com/index.json", &body, &tag, &ask));
    let after = |flag: &str| {
        let at = asked.iter().position(|arg| arg == flag).unwrap();
        asked[at + 1].clone()
    };
    assert_eq!(after("--header"), "If-None-Match: \"e7938c50\"");
    assert_eq!(after("--max-filesize"), "300000");
    assert_eq!(after("--etag-save"), tag.display().to_string());
    assert_eq!(after("--write-out"), "%{http_code}");
    assert_eq!(after("--proto"), "=https");
    assert_eq!(after("--user-agent"), "llx-desk");
    // An ETag that would carry a second header, or none at all, is not sent.
    for etag in [Some("\"a\"\r\nCookie: x"), Some(""), None] {
        let ask = Ask { etag, ..ask };
        let asked = args(&asking("https://example.com/i", &body, &tag, &ask));
        assert!(!asked.iter().any(|arg| arg == "--header"), "{etag:?}");
    }
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_changed_answer_comes_whole_and_a_longer_one_than_allowed_is_refused() {
    let dir = folder("changed");
    std::fs::write(dir.join("index.json"), "x".repeat(4096)).unwrap();
    let url = format!("file://{}/index.json", dir.display());
    let fetched = changed(&url, &dir, &Ask::within(MOST)).unwrap();
    let Fetched::New { text, etag } = fetched else {
        panic!("{fetched:?}")
    };
    assert_eq!(text.len(), 4096);
    // A file has no ETag.
    assert_eq!(etag, None);
    let small = Ask {
        most: 1000,
        ..Ask::within(MOST)
    };
    let refused = changed(&url, &dir, &small).unwrap_err();
    assert!(refused.contains("was not fetched"), "{refused}");
    // Nothing of either asking is left behind.
    let left: Vec<_> = std::fs::read_dir(&dir).unwrap().collect();
    assert_eq!(left.len(), 1, "{left:?}");
    let _ = std::fs::remove_dir_all(dir);
}

/// What the live host answers, asked twice: the second time by the ETag of the first. Run by
/// hand: `cargo test --lib fetch:: -- --ignored`.
#[test]
#[ignore = "reaches the network"]
fn a_live_index_is_the_same_when_asked_again_by_its_etag() {
    let dir = folder("live");
    let url =
        "https://raw.githubusercontent.com/openai/plugins/main/.agents/plugins/marketplace.json";
    let first = changed(url, &dir, &Ask::within(MOST)).unwrap();
    let Fetched::New {
        etag: Some(etag), ..
    } = first
    else {
        panic!("{first:?}")
    };
    let again = Ask {
        etag: Some(&etag),
        ..Ask::within(MOST)
    };
    assert_eq!(changed(url, &dir, &again).unwrap(), Fetched::Same);
    let _ = std::fs::remove_dir_all(dir);
}
