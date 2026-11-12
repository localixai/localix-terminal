//! Tests of `update`: the updater against a folder on disk standing for the releases' host, and
//! a file standing for the app.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use super::install::tests::published;
use super::*;
use crate::fetch::tests::folder;

const NAME: &str = "llxterm-x86_64-unknown-linux-gnu.tar.gz";

/// An updater of a build of `current`, its app the file `dir/app/llxterm` (which says `old`),
/// reading the releases laid out under `dir`. Every change of what it tells is kept.
fn updater(dir: &Path, current: &str) -> (Updater, PathBuf, Arc<Mutex<Vec<Stage>>>) {
    let app = dir.join("app/llxterm");
    std::fs::create_dir_all(app.parent().unwrap()).unwrap();
    std::fs::write(&app, "old").unwrap();
    let place = app.clone();
    let updater = Updater::new(Given {
        from: From::at(&format!("file://{}", dir.display())),
        current: current.into(),
        archive: Some(NAME),
        dir: dir.join("staging"),
        place: Box::new(move || Ok(place.clone())),
        trusted: |_| Ok(()),
    });
    let stages = Arc::new(Mutex::new(Vec::new()));
    let (heard, told) = (Arc::clone(&stages), updater.clone());
    updater.on_change(move || heard.lock().unwrap().push(told.told().stage));
    (updater, app, stages)
}

#[test]
fn a_newer_release_is_fetched_made_ready_and_put_in_place_when_asked() {
    let dir = folder("updater");
    let (_, release) = published(&dir, "0.2.0", NAME, "new");
    let (updater, app, stages) = updater(&dir, "0.1.5+4089808-1794411590");
    assert_eq!(updater.told().stage, Stage::Idle);
    assert_eq!(updater.apply().unwrap_err(), "No update is ready.");
    updater.check_now();
    let told = updater.told();
    assert_eq!(
        told.stage,
        Stage::Ready {
            release: release.clone()
        }
    );
    assert!(told.checked.is_some() && told.unable.is_none());
    // The person saw it asked for, fetched (from nothing to all of it), then ready.
    let stages = stages.lock().unwrap().clone();
    assert_eq!(stages.first(), Some(&Stage::Checking));
    let of = release.size(NAME);
    assert!(stages.contains(&Stage::Fetching {
        release: release.clone(),
        got: 0,
        of
    }));
    assert!(stages.contains(&Stage::Fetching {
        release,
        got: of.unwrap(),
        of
    }));
    assert_eq!(stages.last(), Some(&told.stage));
    // Until they say so the app is the old one.
    assert_eq!(std::fs::read_to_string(&app).unwrap(), "old");
    let again = updater.apply().unwrap();
    assert!(std::fs::read_to_string(&app).unwrap().contains("echo new"));
    assert_eq!((again.place, again.old), (app, dir.join("app/llxterm.old")));
    assert_eq!(
        std::fs::read_to_string(dir.join("app/llxterm.old")).unwrap(),
        "old"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_release_that_is_not_newer_is_no_update() {
    let dir = folder("updater-same");
    published(&dir, "0.2.0", NAME, "same");
    for current in ["0.2.0", "0.3.1", "dev"] {
        let (updater, _, _) = updater(&dir, current);
        updater.check_now();
        let told = updater.told();
        assert_eq!(told.stage, Stage::Idle, "{current}");
        assert!(told.checked.is_some());
        // Nothing was fetched for it, and the asking left nothing behind.
        let left = std::fs::read_dir(dir.join("staging")).map_or(0, Iterator::count);
        assert_eq!(left, 0, "{current}");
    }
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_newer_release_with_no_build_for_this_system_is_no_update() {
    let dir = folder("updater-other-systems");
    published(&dir, "0.2.0", "llxterm-for-another-system.tar.gz", "theirs");
    let (updater, _, _) = updater(&dir, "0.1.5");
    updater.check_now();
    let told = updater.told();
    assert_eq!(told.stage, Stage::Idle);
    assert!(told.checked.is_some());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_check_that_fails_says_why_and_the_next_one_starts_over() {
    let dir = folder("updater-failed");
    let (updater, _, _) = updater(&dir, "0.1.5");
    // The host answers nothing.
    updater.check_now();
    let Stage::Failed { why } = updater.told().stage else {
        panic!("{:?}", updater.told().stage);
    };
    assert!(why.contains("/latest was not fetched"), "{why}");
    assert_eq!(updater.told().checked, None);
    let (_, release) = published(&dir, "0.2.0", NAME, "new");
    updater.check_now();
    assert_eq!(
        updater.told().stage,
        Stage::Ready {
            release: release.clone()
        }
    );
    // Ready, and the host out of reach, or with a newer one: what is ready stays or is replaced.
    std::fs::remove_file(dir.join("latest")).unwrap();
    updater.check_now();
    assert_eq!(updater.told().stage, Stage::Ready { release });
    let (_, newer) = published(&dir, "0.2.1", NAME, "newer");
    updater.check_now();
    assert_eq!(updater.told().stage, Stage::Ready { release: newer });
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn what_is_fetched_and_is_not_the_app_is_thrown_away() {
    let dir = folder("updater-untrusted");
    published(&dir, "0.2.0", NAME, "who knows");
    let (mut_given, app, _) = updater(&dir, "0.1.5");
    drop(mut_given);
    let place = app.clone();
    let updater = Updater::new(Given {
        from: From::at(&format!("file://{}", dir.display())),
        current: "0.1.5".into(),
        archive: Some(NAME),
        dir: dir.join("staging"),
        place: Box::new(move || Ok(place.clone())),
        trusted: |_| Err("it is not signed and notarized as Localix Terminal is".into()),
    });
    updater.check_now();
    assert_eq!(
        updater.told().stage,
        Stage::Failed {
            why: "Localix Terminal 0.2.0 was fetched, but it is not signed and notarized as Localix Terminal is"
                .into()
        }
    );
    assert!(!dir.join("staging").exists());
    assert!(updater.apply().is_err());
    assert_eq!(std::fs::read_to_string(&app).unwrap(), "old");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_build_that_is_no_release_asks_nothing_and_says_why() {
    let dir = folder("updater-unable");
    published(&dir, "0.2.0", NAME, "new");
    let given = |archive, place: Result<PathBuf, String>| Given {
        from: From::at(&format!("file://{}", dir.display())),
        current: "0.1.5".into(),
        archive,
        dir: dir.join("staging"),
        place: Box::new(move || place.clone()),
        trusted: |_| Ok(()),
    };
    let built = Updater::new(given(
        Some(NAME),
        Err("This build is not a release.".into()),
    ));
    // Not known before a check: finding out reads the app's signature.
    assert_eq!(built.told().unable, None);
    built.check_now();
    let told = built.told();
    assert_eq!(told.unable.as_deref(), Some("This build is not a release."));
    assert_eq!((told.stage, told.checked), (Stage::Idle, None));
    let other = Updater::new(given(None, Ok(dir.join("app"))));
    other.check_now();
    assert_eq!(
        other.told().unable.as_deref(),
        Some("There is no release build for this system.")
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_newer_update_that_fails_after_the_ready_one_was_given_up_says_it_failed() {
    let dir = folder("updater-second-fails");
    published(&dir, "0.2.0", NAME, "new");
    let (updater, _, _) = updater(&dir, "0.1.5");
    updater.check_now();
    assert!(matches!(updater.told().stage, Stage::Ready { .. }));
    // A host out of reach takes nothing back: what is ready stays ready.
    std::fs::rename(dir.join("latest"), dir.join("latest.away")).unwrap();
    updater.check_now();
    assert!(matches!(updater.told().stage, Stage::Ready { .. }));
    // A newer one whose archive is not what its release lists: the ready one made room for
    // it, so nothing is ready, and it is said, not left as a download that never ends.
    published(&dir, "0.3.0", NAME, "newer");
    let sums = dir.join("download/v0.3.0/SHA256SUMS");
    std::fs::write(&sums, format!("{}  {NAME}\n", "0".repeat(64))).unwrap();
    updater.check_now();
    assert!(
        matches!(updater.told().stage, Stage::Failed { .. }),
        "{:?}",
        updater.told().stage
    );
    assert!(updater.apply().is_err());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn an_update_is_put_in_place_once_however_often_it_is_asked() {
    let dir = folder("updater-twice");
    let (_, release) = published(&dir, "0.2.0", NAME, "new");
    let (updater, app, _) = updater(&dir, "0.1.5");
    updater.check_now();
    updater.apply().unwrap();
    // Asked again while the app ends: refused, and what was done is not said to have failed.
    assert_eq!(
        updater.apply().unwrap_err(),
        "Localix Terminal 0.2.0 is being installed."
    );
    assert_eq!(updater.told().stage, Stage::Ready { release });
    assert!(std::fs::read_to_string(&app).unwrap().contains("echo new"));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn why_a_build_is_not_updated_is_asked_again_with_the_next_check() {
    let dir = folder("updater-place-again");
    published(&dir, "0.2.0", NAME, "new");
    let app = dir.join("app/llxterm");
    std::fs::create_dir_all(app.parent().unwrap()).unwrap();
    std::fs::write(&app, "old").unwrap();
    let asked = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let (count, place) = (Arc::clone(&asked), app);
    let updater = Updater::new(Given {
        from: From::at(&format!("file://{}", dir.display())),
        current: "0.1.5".into(),
        archive: Some(NAME),
        dir: dir.join("staging"),
        // Not where a release is the first time it is asked (it was being moved), then it is.
        place: Box::new(move || match count.fetch_add(1, Ordering::SeqCst) {
            0 => Err("Localix Terminal is not in a folder it can be replaced in.".into()),
            _ => Ok(place.clone()),
        }),
        trusted: |_| Ok(()),
    });
    updater.check_now();
    assert!(updater.told().unable.is_some());
    updater.check_now();
    assert!(updater.told().unable.is_none());
    assert!(matches!(updater.told().stage, Stage::Ready { .. }));
    // Once known it is not asked for again.
    updater.check_now();
    assert_eq!(asked.load(Ordering::SeqCst), 2);
    let _ = std::fs::remove_dir_all(dir);
}
