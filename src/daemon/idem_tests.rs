//! Tests of `idem`.

use std::sync::atomic::{AtomicUsize, Ordering};

use serde_json::json;

use super::*;

fn done(text: &str) -> Outcome {
    Outcome {
        result: Ok(json!({"text": text})),
        blob: None,
    }
}

#[test]
fn a_retry_gets_the_first_answer_and_the_work_is_not_done_again() {
    let idem = Idem::default();
    let runs = AtomicUsize::new(0);
    let work = || {
        runs.fetch_add(1, Ordering::SeqCst);
        done("first")
    };
    let first = idem.run("k", work);
    let retry = idem.run("k", || done("second"));
    assert_eq!(first.result, retry.result);
    assert_eq!(runs.load(Ordering::SeqCst), 1);
    // Another key is another request.
    assert_eq!(
        idem.run("other", || done("second")).result,
        Ok(json!({"text": "second"}))
    );
}

#[test]
fn a_retry_while_the_first_still_runs_waits_for_it() {
    let idem = Arc::new(Idem::default());
    let (started, go) = std::sync::mpsc::sync_channel::<()>(0);
    let first = {
        let idem = Arc::clone(&idem);
        std::thread::Builder::new()
            .name("first".into())
            .spawn(move || {
                idem.run("k", || {
                    started.send(()).unwrap();
                    // Held until the retry has surely begun to wait: a deadline, no event.
                    let (_open, held) = std::sync::mpsc::sync_channel::<()>(0);
                    let _ = held.recv_timeout(Duration::from_millis(150));
                    done("slow")
                })
            })
            .unwrap()
    };
    go.recv().unwrap();
    let retry = idem.run("k", || done("never"));
    assert_eq!(retry.result, Ok(json!({"text": "slow"})));
    first.join().unwrap();
}

#[test]
fn a_panic_is_that_requests_error_and_is_kept_like_any_answer() {
    let idem = Idem::default();
    let hit = idem.run("k", || panic!("a bug"));
    assert_eq!(hit.result.as_ref().unwrap_err().code, code::INTERNAL);
    assert_eq!(idem.run("k", || done("again")).result, hit.result);
}

#[test]
fn old_answers_are_dropped_past_the_limits() {
    let idem = Idem::default();
    for n in 0..ENTRIES + 5 {
        idem.run(&n.to_string(), || done("x"));
    }
    // The first was dropped, so its key runs again; the last is still kept.
    assert_eq!(
        idem.run("0", || done("again")).result,
        Ok(json!({"text": "again"}))
    );
    let last = (ENTRIES + 4).to_string();
    assert_eq!(
        idem.run(&last, || done("again")).result,
        Ok(json!({"text": "x"}))
    );
    let pictures = Idem::default();
    let big = || Outcome {
        result: Ok(json!({})),
        blob: Some(Blob {
            mime: "image/jpeg".into(),
            bytes: vec![0; BYTES / 2 + 1],
        }),
    };
    pictures.run("a", big);
    pictures.run("b", big);
    assert_eq!(
        pictures.run("a", || done("again")).result,
        Ok(json!({"text": "again"}))
    );
}
