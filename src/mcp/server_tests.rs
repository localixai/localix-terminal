//! Tests of `server`.

use std::sync::Mutex;

use super::*;

/// A stream a test reads back.
#[derive(Clone, Default)]
struct Buffer(Arc<Mutex<Vec<u8>>>);

impl Write for Buffer {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        crate::lock(&self.0).extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Buffer {
    fn messages(&self) -> Vec<Value> {
        let bytes = crate::lock(&self.0).clone();
        String::from_utf8(bytes)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }
}

#[test]
fn a_goals_steps_are_progress_for_the_token_each_further_along() {
    let buffer = Buffer::default();
    let out = Out::spawn(buffer.clone()).unwrap();
    let sink = progress(&out, json!("tok-1"));
    sink("w1: CLICK button \"Go\"");
    sink("w1: DONE");
    out.flush();
    let told = buffer.messages();
    assert_eq!(told.len(), 2);
    assert_eq!(told[0]["method"], "notifications/progress");
    assert_eq!(told[0]["params"]["progressToken"], "tok-1");
    assert_eq!(told[0]["params"]["message"], "w1: CLICK button \"Go\"");
    assert_eq!(
        (
            told[0]["params"]["progress"].clone(),
            told[1]["params"]["progress"].clone()
        ),
        (json!(1), json!(2))
    );
    assert!(told[0].get("id").is_none(), "a notification has no id");
}

#[test]
fn the_revision_the_host_asks_for_is_spoken_when_known_and_the_latest_otherwise() {
    assert_eq!(
        initialized(&json!({"protocolVersion": "2025-03-26"}))["protocolVersion"],
        "2025-03-26"
    );
    assert_eq!(
        initialized(&json!({"protocolVersion": "2099-01-01"}))["protocolVersion"],
        "2025-06-18"
    );
    assert_eq!(initialized(&json!({}))["protocolVersion"], "2025-06-18");
}

#[test]
fn a_message_that_is_not_json_is_answered_and_the_stream_goes_on() {
    let buffer = Buffer::default();
    let out = Out::spawn(buffer.clone()).unwrap();
    // No daemon is wanted by any of this.
    let relay = Relay::start(
        "me",
        super::super::relay::Timing::standard(),
        Box::new(|_, _| Ok(None)),
    )
    .unwrap();
    let server = Server {
        relay: relay.clone(),
        out: out.clone(),
        owner: "me".into(),
        cwd: "/tmp".into(),
        running: Arc::new(AtomicUsize::new(0)),
    };
    let input = "not json\n\n{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"ping\"}\n";
    server.serve(input.as_bytes());
    out.flush();
    relay.close();
    let told = buffer.messages();
    assert_eq!(told[0]["error"]["code"], -32700);
    assert_eq!(
        (told[1]["id"].clone(), told[1]["result"].clone()),
        (json!(3), json!({}))
    );
}
