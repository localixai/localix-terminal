//! What a call is (its `action`), and the fields of the calls that are no single act on a tab:
//! several steps in one call, and a topic of the guide.

use serde_json::{Map, Value, json};

/// `action`, `steps` and `topic`.
pub(super) fn fields() -> Map<String, Value> {
    let action = json!({"type": "string", "enum":
        ["open", "interact", "screenshot", "close", "show", "list", "steps", "guide"]});
    let steps = json!({"type": "array", "items": {"anyOf": [{"type": "object"}, {"type": "string"}]}, "description":
        "steps: up to 16 actions run in order in one call, each an action's object or a short command \
         (`click e12`, `wait Saved`); one with no `tab` works in the tab of its kind the \
         step before used. The first that fails or misses stops the rest; the answer is a line a step and \
         the last step's tab."});
    let topic = json!({"type": "string", "description": "guide: the topic to read (`commands`, `pages`, `terminals`…); none: the overview and the topics."});
    Map::from_iter([
        ("action".into(), action),
        ("steps".into(), steps),
        ("topic".into(), topic),
    ])
}
