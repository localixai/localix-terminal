//! Tests of `api`: the one table of methods, each named once and described.

use super::{METHODS, describe, find};

#[test]
fn every_method_is_named_once_and_described_with_its_params() {
    for (at, method) in METHODS.iter().enumerate() {
        assert!(
            METHODS
                .iter()
                .skip(at + 1)
                .all(|other| other.name != method.name),
            "{} twice",
            method.name
        );
        assert_eq!((method.params)()["type"], "object", "{}", method.name);
        assert!(find(method.name).is_some());
    }
    let described = describe("b");
    assert_eq!(
        described["methods"].as_array().unwrap().len(),
        METHODS.len()
    );
    assert!(described["window"]["schema"]["properties"]["action"].is_object());
    assert_eq!(find("window/call").map(|m| m.work), Some(true));
    assert!(find("nothing/here").is_none());
}
