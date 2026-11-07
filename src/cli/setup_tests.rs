//! Tests of `setup`'s arguments.

use super::parse;

fn args(words: &[&str]) -> Vec<String> {
    words.iter().map(|word| (*word).to_string()).collect()
}

#[test]
fn off_goes_with_one_integration_and_nothing_else() {
    assert!(parse(&args(&["codex", "--off"])).is_some_and(|(off, _)| off));
    assert!(parse(&args(&["codex"])).is_some_and(|(off, _)| !off));
    assert!(parse(&args(&[])).is_some());
    assert!(parse(&args(&["all"])).is_some());
    // `--off` never turns `all` or `mcp` into something else, or lists with nothing named.
    assert!(parse(&args(&["all", "--off"])).is_none());
    assert!(parse(&args(&["mcp", "--off"])).is_none());
    assert!(parse(&args(&["--off"])).is_none());
    // No other option is known.
    assert!(parse(&args(&["--force"])).is_none());
    assert!(parse(&args(&["codex", "-x"])).is_none());
}
