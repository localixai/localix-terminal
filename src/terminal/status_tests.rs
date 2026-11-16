//! Tests of `status`: a program's reports of its state read by the protocol's grammar and limits,
//! and the records a terminal keeps of them.

use base64::Engine;

use super::*;

fn b64(text: &str) -> String {
    base64::engine::general_purpose::STANDARD.encode(text)
}

fn report(body: &str) -> Report {
    match read(body) {
        Some(Said::Report(report)) => report,
        other => panic!("{body:?} reads as {other:?}"),
    }
}

/// The records after `bodies`, applied in turn.
fn after(bodies: &[&str]) -> Records {
    let mut records = Records::default();
    for body in bodies {
        if let Some(Said::Report(report)) = read(body) {
            records.apply(report);
        }
    }
    records
}

fn states(records: &Records) -> Vec<(String, Doing)> {
    (records.rows().into_iter())
        .map(|row| (row.id, row.state))
        .collect()
}

#[test]
fn what_claude_code_sends_is_read() {
    let asks = format!(
        "state=blocked:app=claude-code:kind=permission:msg={}",
        b64("approve Bash: touch /tmp/x")
    );
    let read = report(&asks);
    assert_eq!(read.id, "");
    assert_eq!(read.state, Some(Doing::Blocked));
    assert_eq!(read.asks, Some(Asks::Permission));
    assert_eq!(read.app.as_deref(), Some("claude-code"));
    assert_eq!(read.msg.as_deref(), Some("approve Bash: touch /tmp/x"));
    assert_eq!(
        report("state=working:app=claude-code").state,
        Some(Doing::Working)
    );
    assert_eq!(
        report("state=done:app=claude-code").state,
        Some(Doing::Done)
    );
    assert_eq!(
        report("state=clear").state,
        None,
        "clear is no state of its own"
    );
}

#[test]
fn the_query_is_told_apart_and_answered_with_what_this_terminal_reads() {
    assert!(matches!(read("?"), Some(Said::Query)));
    let reply = std::str::from_utf8(REPLY).unwrap();
    assert!(reply.starts_with("\x1b]7501;?"), "{reply:?}");
    assert!(reply.ends_with("\x1b\\"), "{reply:?}");
    for state in ["idle", "working", "done", "blocked", "error"] {
        assert!(reply.contains(state), "{reply:?} names {state}");
    }
    for kind in ["permission", "question", "auth"] {
        assert!(reply.contains(kind), "{reply:?} names {kind}");
    }
}

#[test]
fn a_bad_pair_is_passed_over_and_the_last_of_a_key_wins() {
    let read = report("state=idle:BAD=1:app=one:x=a b:progress=7:app=two:nokey:junk=1");
    assert_eq!(read.state, Some(Doing::Idle));
    assert_eq!(read.app.as_deref(), Some("two"));
    let read = report("state=working:progress=40:progress=101");
    assert_eq!(
        read.progress, None,
        "the last one, past 100, says no progress known"
    );
    assert_eq!(
        report(&format!("state=working:{}=1", "k".repeat(17))).state,
        Some(Doing::Working),
        "a key past 16 bytes is a bad pair"
    );
}

#[test]
fn a_report_with_no_state_or_one_not_known_is_dropped_whole() {
    assert!(read("app=claude-code").is_none());
    assert!(read("state=thinking:app=claude-code").is_none());
    assert!(read("state=").is_none());
    assert!(read("").is_none());
}

#[test]
fn a_bad_id_drops_the_report_and_never_becomes_the_root() {
    assert_eq!(report("state=working:id=a1/b.c_d+e-f").id, "a1/b.c_d+e-f");
    let deep = ["a"; 8].join("/");
    assert_eq!(report(&format!("state=working:id={deep}")).id, deep);
    for id in [
        "",
        "a//b",
        "/a",
        "a/",
        &"x".repeat(33),
        &["a"; 9].join("/"),
        "a=b",
        "a,b",
    ] {
        assert!(
            read(&format!("state=working:id={id}")).is_none(),
            "id {id:?}"
        );
    }
    let records = after(&["state=working", "state=done:id=a//b"]);
    assert_eq!(states(&records), [(String::new(), Doing::Working)]);
}

#[test]
fn texts_are_base64_of_plain_words_within_their_limits() {
    let words = report(&format!(
        "state=working:title={}",
        b64("  fix   the cart  ")
    ));
    assert_eq!(
        words.title.as_deref(),
        Some("fix the cart"),
        "spaces single"
    );
    // Text that would turn the line around or hide in it is taken out.
    let hidden = report(&format!(
        "state=working:title={}",
        b64("a\u{202e}b\u{200b}c\u{2066}d\u{feff}e")
    ));
    assert_eq!(hidden.title.as_deref(), Some("abcde"));
    for broken in [
        "state=working:title=a".to_string(),
        format!("state=working:msg={}", b64("bell\u{7}")),
        format!("state=working:msg={}", b64("csi\u{9b}")),
        format!("state=working:msg={}", b64("del\u{7f}")),
        format!("state=working:title={}", b64(&"t".repeat(193))),
        format!("state=working:msg={}", b64(&"m".repeat(2049))),
        format!(
            "state=working:msg={}",
            base64::engine::general_purpose::STANDARD.encode([0xff, 0xfe])
        ),
    ] {
        assert!(read(&broken).is_none(), "{broken}");
    }
    let most = report(&format!("state=blocked:msg={}", b64(&"m".repeat(2048))));
    assert_eq!(most.msg.map(|msg| msg.len()), Some(2048));
    let unpadded = report("state=working:title=aGk");
    assert_eq!(unpadded.title.as_deref(), Some("hi"));
}

#[test]
fn kind_counts_only_while_blocked_and_progress_only_while_at_work() {
    assert_eq!(report("state=done:kind=question").asks, None);
    assert_eq!(
        report("state=blocked:kind=question").asks,
        Some(Asks::Question)
    );
    assert_eq!(report("state=blocked:kind=auth").asks, Some(Asks::Auth));
    assert_eq!(report("state=blocked:kind=other").asks, None);
    assert_eq!(report("state=done:progress=50").progress, None);
    assert_eq!(report("state=blocked:progress=50").progress, Some(50));
}

#[test]
fn a_report_replaces_its_record_whole_and_the_same_again_is_no_change() {
    let mut records = after(&["state=working:app=claude-code:progress=10"]);
    let before = records.changes();
    assert!(!records.apply(report("state=working:app=claude-code:progress=10")));
    assert_eq!(records.changes(), before);
    assert!(records.apply(report("state=working")));
    let root = &records.rows()[0];
    assert_eq!((root.app.as_deref(), root.progress), (None, None));
    assert!(records.changes() > before);
}

#[test]
fn clear_takes_a_record_and_its_children_and_clear_alone_takes_all() {
    let mut records = after(&[
        "state=working",
        "state=working:id=a",
        "state=blocked:id=a/b",
        "state=working:id=ab",
    ]);
    assert!(records.apply(report("state=clear:id=a")));
    assert_eq!(
        states(&records),
        [
            (String::new(), Doing::Working),
            ("ab".to_string(), Doing::Working)
        ]
    );
    assert!(
        !records.apply(report("state=clear:id=zz")),
        "nothing to clear"
    );
    assert!(records.apply(report("state=clear")));
    assert!(records.rows().is_empty());
}

#[test]
fn a_child_takes_its_app_from_the_nearest_parent_that_names_one() {
    let records = after(&[
        "state=working:app=claude-code",
        "state=working:id=a1",
        "state=working:id=a1/b:app=helper",
        "state=working:id=a1/b/c",
        "state=working:id=x/y",
    ]);
    let apps: Vec<(String, Option<String>)> = (records.rows().into_iter())
        .map(|row| (row.id, row.app))
        .collect();
    let app = |name: &str| Some(name.to_string());
    assert_eq!(
        apps,
        [
            (String::new(), app("claude-code")),
            ("a1".into(), app("claude-code")),
            ("a1/b".into(), app("helper")),
            ("a1/b/c".into(), app("helper")),
            ("x/y".into(), app("claude-code")),
        ]
    );
}

#[test]
fn the_most_urgent_is_blocked_then_error_done_working_idle_the_newest_first() {
    let mut records = after(&["state=idle", "state=working:id=a", "state=done:id=b"]);
    assert_eq!(
        records.most_urgent().map(|row| row.state),
        Some(Doing::Done)
    );
    records.apply(report("state=error:id=c"));
    records.apply(report("state=blocked:id=d"));
    assert_eq!(
        records.most_urgent().map(|row| row.id),
        Some("d".to_string())
    );
    records.apply(report("state=blocked:id=e"));
    assert_eq!(
        records.most_urgent().map(|row| row.id),
        Some("e".to_string()),
        "of two alike, the one told last"
    );
}

#[test]
fn a_program_gone_takes_what_was_at_work_and_the_person_seeing_takes_the_rest() {
    let mut records = after(&[
        "state=working",
        "state=blocked:id=a",
        "state=idle:id=b",
        "state=done:id=c",
        "state=error:id=d",
    ]);
    assert!(records.drop_transient());
    assert_eq!(
        states(&records),
        [
            ("c".to_string(), Doing::Done),
            ("d".to_string(), Doing::Error)
        ]
    );
    assert!(!records.drop_transient());
    assert!(records.seen());
    assert!(records.rows().is_empty());
    assert!(!records.seen());
    let mut reset = after(&["state=done", "state=working:id=a"]);
    assert!(reset.reset());
    assert!(reset.rows().is_empty());
}

#[test]
fn past_the_most_records_the_one_told_of_longest_ago_goes() {
    let mut records = Records::default();
    for n in 0..MOST {
        records.apply(report(&format!("state=working:id=r{n}")));
    }
    // The first told again is the newest now: the second goes in its place.
    records.apply(report("state=working:id=r0"));
    records.apply(report("state=working:id=new"));
    let ids: Vec<String> = records.rows().into_iter().map(|row| row.id).collect();
    assert_eq!(ids.len(), MOST);
    assert!(ids.contains(&"r0".to_string()) && ids.contains(&"new".to_string()));
    assert!(!ids.contains(&"r1".to_string()));
}
