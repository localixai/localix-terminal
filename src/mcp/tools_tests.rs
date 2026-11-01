//! Tests of `tools`.

use super::*;
use crate::wire::{Blob, RpcError};

fn window(name: &str, arguments: &Value) -> Value {
    let Some(Method::Window(args)) = request(name, arguments) else {
        panic!("{name} is not a window call");
    };
    args
}

#[test]
fn each_tool_is_a_call_of_the_window_tool() {
    assert_eq!(
        window("open", &json!({"kind": "terminal", "command": "python3"})),
        json!({"action": "open", "kind": "terminal", "command": "python3"})
    );
    assert_eq!(
        window("act", &json!({"tab": "w1", "keys": ["Enter"]})),
        json!({"action": "interact", "tab": "w1", "keys": ["Enter"]})
    );
    assert_eq!(
        window("look", &json!({"tab": "w1", "text": true})),
        json!({"action": "interact", "tab": "w1", "read": true})
    );
    assert_eq!(
        window("screenshot", &json!({"tab": "w1", "selector": "#a"})),
        json!({"action": "screenshot", "tab": "w1", "selector": "#a"})
    );
    assert_eq!(window("tabs", &json!({})), json!({"action": "list"}));
    assert_eq!(
        window("tabs", &json!({"action": "close", "tab": "w1"})),
        json!({"action": "close", "tab": "w1"})
    );
    assert!(matches!(
        request("comments", &json!({})),
        Some(Method::Said)
    ));
    assert!(request("nothing", &json!({})).is_none());
}

#[test]
fn a_look_does_not_pass_on_what_would_act() {
    // `look` is read-only: a script or keys in its arguments are not passed to the tab.
    let args = window(
        "look",
        &json!({"tab": "w1", "script": "document.body.remove()", "keys": ["Enter"]}),
    );
    assert_eq!(args, json!({"action": "interact", "tab": "w1"}));
}

#[test]
fn a_picture_is_image_content_beside_the_text() {
    let answer = Answer {
        value: json!({"text": "w1 shot"}),
        blob: Some(Blob {
            mime: "image/jpeg".into(),
            bytes: vec![1, 2, 3],
        }),
    };
    let result = result(Ok(answer), "-");
    assert_eq!(
        result["content"][0],
        json!({"type": "text", "text": "w1 shot"})
    );
    assert_eq!(
        result["content"][1],
        json!({"type": "image", "data": "AQID", "mimeType": "image/jpeg"})
    );
    assert!(result.get("isError").is_none());
}

#[test]
fn an_empty_answer_says_so_and_a_failure_is_a_result_the_model_can_read() {
    let empty = Answer {
        value: json!({"text": ""}),
        blob: None,
    };
    assert_eq!(
        result(Ok(empty), "No new comments.")["content"][0]["text"],
        "No new comments."
    );
    let refused = result(
        Err(Failure::Refused(RpcError::new(
            -32000,
            "there is no tab w9 among yours",
        ))),
        "-",
    );
    assert_eq!(refused["isError"], true);
    assert!(
        refused["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("w9")
    );
    let lost = result(
        Err(Failure::Unavailable(
            "the daemon could not be reached within 15s".into(),
        )),
        "-",
    );
    let text = lost["content"][0]["text"].as_str().unwrap();
    assert!(
        lost["isError"] == true
            && text.contains("could not be reached")
            && text.contains("fallback")
    );
}
