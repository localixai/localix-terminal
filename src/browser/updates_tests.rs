//! Tests of `browser::updates`: what the person reads of each stage of an update.

use super::*;
use crate::update::{Release, Version};

fn release() -> Release {
    Release {
        version: Version(0, 2, 0),
        notes: String::new(),
        files: vec![],
    }
}

fn told(stage: Stage) -> Told {
    Told {
        current: "0.1.5+4089808-1794411590".into(),
        stage,
        checked: None,
        unable: None,
    }
}

const NOW: u64 = 1_800_000_000;

#[test]
fn the_row_names_the_build_and_says_how_far_an_update_is() {
    let idle = row(&told(Stage::Idle), NOW);
    assert_eq!(idle.title, "LLX Desk 0.1.5");
    assert_eq!(
        (idle.says.as_str(), idle.button),
        ("Not checked yet.", Some("Check Now"))
    );
    assert_eq!(
        (idle.act, idle.busy, idle.failed),
        (Some(Act::Check), false, false)
    );

    let checked = Told {
        checked: Some(NOW - 300),
        ..told(Stage::Idle)
    };
    assert_eq!(
        row(&checked, NOW).says,
        "Up to date. Checked 5 minutes ago."
    );

    let checking = row(&told(Stage::Checking), NOW);
    assert_eq!(
        (checking.says.as_str(), checking.busy),
        ("Checking for updates…", true)
    );

    let ready = row(&told(Stage::Ready { release: release() }), NOW);
    assert_eq!(ready.says, "0.2.0 is ready to install.");
    assert_eq!(
        (ready.act, ready.button),
        (Some(Act::Apply), Some("Restart to Update"))
    );
    assert_eq!(ready.notes.as_deref(), Some("0.2.0"));

    let failed = row(
        &told(Stage::Failed {
            why: "the host did not answer".into(),
        }),
        NOW,
    );
    assert_eq!(
        failed.says,
        "The update did not go through: the host did not answer"
    );
    assert_eq!(
        (failed.failed, failed.act, failed.button),
        (true, Some(Act::Check), Some("Try Again"))
    );
}

#[test]
fn a_download_says_its_part_when_its_size_is_known() {
    let fetching = |got, of| {
        row(
            &told(Stage::Fetching {
                release: release(),
                got,
                of,
            }),
            NOW,
        )
    };
    let half = fetching(4_500_000, Some(9_000_000));
    assert_eq!(
        (half.says.as_str(), half.progress),
        ("Downloading 0.2.0… 50%", Some(50))
    );
    assert!(half.busy);
    // More than the release said is all of it; a size not said, or of nothing, is no part.
    assert_eq!(fetching(10, Some(5)).progress, Some(100));
    for of in [None, Some(0)] {
        let unknown = fetching(10, of);
        assert_eq!(
            (unknown.says.as_str(), unknown.progress),
            ("Downloading 0.2.0…", None)
        );
    }
}

#[test]
fn a_build_that_is_not_updated_says_why_and_offers_nothing() {
    let built = Told {
        unable: Some("This build is not a release.".into()),
        ..told(Stage::Idle)
    };
    let shown = row(&built, NOW);
    assert_eq!(shown.says, "This build is not a release.");
    assert_eq!((shown.act, shown.button), (None, None));
}

#[test]
fn how_long_ago_is_said_in_words() {
    let said = |seconds| ago(NOW - seconds, NOW);
    assert_eq!(said(0), "just now");
    assert_eq!(said(59), "just now");
    assert_eq!(said(60), "a minute ago");
    assert_eq!(said(59 * 60), "59 minutes ago");
    assert_eq!(said(60 * 60), "an hour ago");
    assert_eq!(said(5 * 60 * 60), "5 hours ago");
    assert_eq!(said(30 * 60 * 60), "yesterday");
    assert_eq!(said(3 * 24 * 60 * 60), "3 days ago");
    // A clock set back is no time ago.
    assert_eq!(ago(NOW + 500, NOW), "just now");
}

#[test]
fn only_an_update_that_is_ready_is_offered_under_its_version() {
    let ready = told(Stage::Ready { release: release() });
    assert_eq!(
        offer(&ready),
        Some((
            "update:0.2.0".into(),
            "LLX Desk 0.2.0 is ready to install.".into()
        ))
    );
    let fetching = Stage::Fetching {
        release: release(),
        got: 1,
        of: None,
    };
    for stage in [
        Stage::Idle,
        Stage::Checking,
        fetching,
        Stage::Failed { why: String::new() },
    ] {
        assert_eq!(offer(&told(stage)), None);
    }
    assert_eq!(
        notes("0.2.0"),
        "https://github.com/rsaryev/llx-desk/releases/tag/v0.2.0"
    );
}

#[test]
fn an_update_that_is_ready_is_announced_with_both_versions_and_what_is_new() {
    let told = Told {
        current: "0.1.5".into(),
        stage: Stage::Ready {
            release: Release {
                version: Version(0, 2, 0),
                notes: "### Added\n\n- **Dialogs**: one look.".into(),
                files: vec![],
            },
        },
        checked: None,
        unable: None,
    };
    let (offered, dialog) = announcement(&told).expect("an announcement");
    assert_eq!(offered, "update:0.2.0", "Later puts this offer off");
    assert_eq!(
        dialog.id, "announced:0.2.0",
        "the next version is announced anew"
    );
    assert_eq!(dialog.versions, Some(("0.1.5".into(), "0.2.0".into())));
    assert_eq!(
        (dialog.yes.as_str(), dialog.no.as_str()),
        ("Restart to Update", "Later")
    );
    assert!(
        !dialog.asked,
        "raised by the app: it leaves the keys where they are"
    );
    assert_eq!(dialog.notes.len(), 2, "{:?}", dialog.notes);
    let idle = Told {
        stage: Stage::Idle,
        ..told
    };
    assert!(
        announcement(&idle).is_none(),
        "nothing ready: nothing announced"
    );
}
