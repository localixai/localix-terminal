//! Tests of `mcp`: the protocol revision it speaks and the shape of a failed tool call.

use super::{failed, initialized};
use serde_json::json;

#[test]
fn the_revision_asked_is_spoken_when_known_else_the_newest() {
    let known = initialized(&json!({"protocolVersion": "2025-03-26"}));
    assert_eq!(known["protocolVersion"], "2025-03-26");
    let unknown = initialized(&json!({"protocolVersion": "1999-01-01"}));
    assert_eq!(unknown["protocolVersion"], "2025-06-18");
    assert_eq!(failed("no")["isError"], true);
}
