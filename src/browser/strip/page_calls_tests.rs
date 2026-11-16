//! Tests of the calls the window makes in the page shown that nobody waits for.

use serde_json::Value;

use super::*;

#[test]
fn a_step_of_find_reaches_the_page_as_data_past_the_probes_key() {
    let query = "</script>'); alert(1); ('\u{2028}";
    let call = find("k3y", 3, "type", query, "#0a84ff");
    let head = "window.__dloop_call ? window.__dloop_call(\"k3y\", \"0\", \"\", 'find', ";
    let args = (call.strip_prefix(head))
        .and_then(|rest| rest.strip_suffix(") : false"))
        .unwrap_or_else(|| panic!("a call of the probe's: {call}"));
    let args: Value = serde_json::from_str(args).unwrap();
    assert_eq!(args["tab"], 3);
    assert_eq!(args["step"], "type");
    assert_eq!(args["query"], query);
    assert_eq!(args["accent"], "#0a84ff");
    // No line end a script may not hold.
    assert!(!call.contains('\u{2028}'));
}
