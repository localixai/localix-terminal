//! Tests of `relay`, against daemons that are faked: one that answers, one that hangs up, one
//! that never answers a ping.

use std::collections::VecDeque;
use std::io::BufReader;
use std::os::unix::net::UnixStream;
use std::sync::atomic::AtomicUsize;

use super::*;
use crate::wire::{Message, Next, PROTOCOL, code, pipe::Outbox, read_frame};

/// What a faked daemon does with a request.
enum Act {
    Answer(Value),
    /// Send these notifications first, then answer.
    Steps(Vec<Value>, Value),
    Silence,
    HangUp,
}

/// A daemon over a socket pair: it handshakes and subscribes, and `behave` says the rest.
fn daemon(behave: impl Fn(&str, &Value) -> Act + Send + 'static) -> UnixStream {
    let (ours, theirs) = UnixStream::pair().unwrap();
    let out = Outbox::spawn(ours.try_clone().unwrap(), "fake-write").unwrap();
    std::thread::Builder::new()
        .name("fake-daemon".into())
        .spawn(move || {
            let mut input = BufReader::new(ours.try_clone().unwrap());
            while let Ok(Next::Got(frame)) = read_frame(&mut input) {
                let Message::Request { id, method, params } = frame.message else {
                    continue;
                };
                let act = match method.as_str() {
                    "initialize" => Act::Answer(json!({
                        "protocolVersion": PROTOCOL,
                        "serverInfo": {"version": "0", "build": "fake", "pid": 1},
                    })),
                    "events/subscribe" => Act::Answer(json!({})),
                    other => behave(other, &params),
                };
                match act {
                    Act::Answer(result) => {
                        out.send(Message::done(id, result).encode(None));
                    }
                    Act::Steps(steps, result) => {
                        for step in steps {
                            out.send(Message::notification("goal/step", step).encode(None));
                        }
                        out.send(Message::done(id, result).encode(None));
                    }
                    Act::Silence => {}
                    Act::HangUp => {
                        let _ = ours.shutdown(std::net::Shutdown::Both);
                        return;
                    }
                }
            }
        })
        .unwrap();
    theirs
}

fn timing() -> Timing {
    Timing {
        ping_every: Duration::from_millis(40),
        ping_timeout: Duration::from_millis(150),
        reconnect_wait: Duration::from_secs(3),
        backoff_first: Duration::from_millis(10),
        backoff_last: Duration::from_millis(50),
    }
}

/// A relay that connects to the daemons in `queue`, one after the other; the count of connects.
fn relay(queue: Vec<UnixStream>, timing: Timing) -> (Relay, Arc<AtomicUsize>) {
    let queue = Mutex::new(VecDeque::from(queue));
    let connects = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&connects);
    let connect: Box<Connect> = Box::new(move |_start, notes| {
        counted.fetch_add(1, Ordering::SeqCst);
        let next = queue.lock().unwrap().pop_front();
        match next {
            Some(stream) => Client::over(stream, notes)
                .map(Some)
                .map_err(|t| t.to_string()),
            None => Err("no daemon here".into()),
        }
    });
    (Relay::start("me", timing, connect).unwrap(), connects)
}

fn params(key: &str) -> Value {
    json!({"owner": "me", "idempotencyKey": key, "args": {"action": "list"}})
}

#[test]
fn a_call_fails_with_the_reason_in_time_when_there_is_no_daemon() {
    let mut short = timing();
    short.reconnect_wait = Duration::from_millis(300);
    let (relay, _) = relay(vec![], short);
    let began = Instant::now();
    let Err(Failure::Unavailable(why)) =
        relay.call("window/call", &params("k"), None, Start::Spawn)
    else {
        panic!("a call without a daemon came to something");
    };
    assert!(why.contains("no daemon here"), "{why}");
    assert!(
        began.elapsed() < Duration::from_secs(3),
        "{:?}",
        began.elapsed()
    );
    relay.close();
}

#[test]
fn a_call_made_again_after_the_connection_broke_carries_the_same_key() {
    let keys = Arc::new(Mutex::new(vec![]));
    let seen = Arc::clone(&keys);
    let hangs_up = daemon(move |method, params| {
        seen.lock()
            .unwrap()
            .push((method.to_string(), params["idempotencyKey"].clone()));
        Act::HangUp
    });
    let seen = Arc::clone(&keys);
    let answers = daemon(move |method, params| {
        seen.lock()
            .unwrap()
            .push((method.to_string(), params["idempotencyKey"].clone()));
        Act::Answer(json!({"text": "done once"}))
    });
    let (relay, connects) = relay(vec![hangs_up, answers], timing());
    let answer = relay
        .call("window/call", &params("same-key"), None, Start::Spawn)
        .unwrap();
    assert_eq!(answer.value["text"], "done once");
    let keys = keys.lock().unwrap();
    assert_eq!(keys.len(), 2, "{keys:?}");
    assert!(
        keys.iter()
            .all(|(method, key)| method == "window/call" && key == "same-key")
    );
    assert_eq!(connects.load(Ordering::SeqCst), 2);
    relay.close();
}

#[test]
fn a_ping_that_goes_unanswered_ends_the_connection_and_a_new_one_is_made() {
    let deaf = daemon(|method, _| {
        if method == "ping" {
            Act::Silence
        } else {
            Act::Answer(json!({}))
        }
    });
    let alive = daemon(|_, _| Act::Answer(json!({"text": "back"})));
    let (relay, connects) = relay(vec![deaf, alive], timing());
    // The first connection is made for the call; its pings get no answer.
    let answer = relay
        .call("window/call", &params("k"), None, Start::Spawn)
        .unwrap();
    assert_eq!(answer.value, json!({}));
    let began = Instant::now();
    while connects.load(Ordering::SeqCst) < 2 {
        assert!(
            began.elapsed() < Duration::from_secs(5),
            "never connected again"
        );
        let (_gate, held) = std::sync::mpsc::sync_channel::<()>(0);
        let _ = held.recv_timeout(Duration::from_millis(20));
    }
    let after = relay
        .call("window/call", &params("k2"), None, Start::Spawn)
        .unwrap();
    assert_eq!(after.value["text"], "back");
    relay.close();
}

#[test]
fn a_goals_steps_go_to_the_sink_of_the_call_that_runs_it() {
    let steps = vec![
        json!({"key": "g1", "tab": "w1", "text": "CLICK button \"Go\""}),
        json!({"key": "other", "tab": "w2", "text": "not mine"}),
        json!({"key": "g1", "tab": "w1", "text": "DONE"}),
    ];
    let goal = daemon(move |_, _| Act::Steps(steps.clone(), json!({"text": "finished"})));
    let (relay, _) = relay(vec![goal], timing());
    let told = Arc::new(Mutex::new(vec![]));
    let sink: Sink = {
        let told = Arc::clone(&told);
        Arc::new(move |text| told.lock().unwrap().push(text.to_string()))
    };
    let answer = relay
        .call("window/call", &params("g1"), Some(sink), Start::Spawn)
        .unwrap();
    assert_eq!(answer.value["text"], "finished");
    assert_eq!(
        *told.lock().unwrap(),
        vec!["w1: CLICK button \"Go\"", "w1: DONE"]
    );
    relay.close();
}

#[test]
fn the_daemons_refusal_is_the_calls_not_a_reason_to_ask_again() {
    let (ours, theirs) = UnixStream::pair().unwrap();
    let out = Outbox::spawn(ours.try_clone().unwrap(), "fake-write").unwrap();
    std::thread::Builder::new()
        .name("fake-daemon".into())
        .spawn(move || {
            let mut input = BufReader::new(ours);
            while let Ok(Next::Got(frame)) = read_frame(&mut input) {
                if let Message::Request { id, method, .. } = frame.message {
                    let message = if method == "window/call" {
                        Message::failed(
                            id,
                            RpcError::new(code::FAILED, "there is no tab w9 among yours"),
                        )
                    } else {
                        Message::done(id, json!({"serverInfo": {}}))
                    };
                    out.send(message.encode(None));
                }
            }
        })
        .unwrap();
    let (relay, connects) = relay(vec![theirs], timing());
    let Err(Failure::Refused(error)) = relay.call("window/call", &params("k"), None, Start::Spawn)
    else {
        panic!("a refusal came to something else");
    };
    assert!(error.message.contains("w9"));
    assert_eq!(connects.load(Ordering::SeqCst), 1);
    relay.close();
}

#[test]
fn a_call_that_may_not_start_the_daemon_finds_none_quickly_and_starts_none() {
    let spawned = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&spawned);
    let connect: Box<Connect> = Box::new(move |start, _| {
        if start == Start::Spawn {
            counted.fetch_add(1, Ordering::SeqCst);
        }
        Ok(None)
    });
    let relay = Relay::start("me", timing(), connect).unwrap();
    let began = Instant::now();
    let asked = relay.call("window/said", &params("k"), None, Start::Never);
    assert!(matches!(asked, Err(Failure::Absent)), "{asked:?}");
    assert!(
        began.elapsed() < Duration::from_secs(2),
        "{:?}",
        began.elapsed()
    );
    assert_eq!(spawned.load(Ordering::SeqCst), 0);
    relay.close();
}
