//! Methods that exist in test builds alone, so that tests can reach what no window is there to
//! cause: the person's comment, bytes after a response, a request that panics, a count of how
//! often a request was really done.

use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{Value, json};

use super::Shared;
use super::idem::Outcome;
use crate::wire::Blob;

/// How many times `test/count` was done.
static COUNTED: AtomicU64 = AtomicU64::new(0);

#[expect(
    clippy::panic,
    reason = "a test build asks for a panic, to prove that the daemon survives one"
)]
pub(super) fn call(shared: &Shared, method: &str, params: &Value) -> Option<Outcome> {
    let text = |name: &str| params.get(name).and_then(Value::as_str).unwrap_or_default();
    let result = match method {
        "test/say" => {
            shared.desk.person_says(text("tab"), text("words"));
            json!({})
        }
        "test/count" => json!({"count": COUNTED.fetch_add(1, Ordering::SeqCst) + 1}),
        "test/panic" => panic!("a bug a test asked for"),
        "test/blob" => {
            let length = params.get("length").and_then(Value::as_u64).unwrap_or(0);
            let length = usize::try_from(length).unwrap_or(0);
            let bytes = (0..length)
                .map(|n| u8::try_from(n % 251).unwrap_or(0))
                .collect();
            let blob = Blob {
                mime: "image/jpeg".into(),
                bytes,
            };
            return Some(Outcome {
                result: Ok(json!({"text": "bytes follow"})),
                blob: Some(blob),
            });
        }
        _ => return None,
    };
    Some(Outcome {
        result: Ok(result),
        blob: None,
    })
}
