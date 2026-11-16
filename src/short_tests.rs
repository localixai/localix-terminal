//! Tests of the short commands: each verb is one `window` action, `+` joins steps, a step's text
//! is one argument with no escapes in it, and a usage error names the command's synopsis.

use serde_json::{Value, json};

use super::{VERBS, guide, help, line, steps, words};

/// `argv` as the steps it is, with no stdin.
fn parsed(argv: &[&str]) -> Result<Vec<Value>, String> {
    let argv: Vec<String> = argv.iter().map(|word| (*word).to_string()).collect();
    steps(&argv, &mut || Err("no stdin here".into()))
}

/// The one step `argv` is.
fn one(argv: &[&str]) -> Value {
    let mut all = parsed(argv).unwrap_or_else(|why| panic!("{argv:?}: {why}"));
    assert_eq!(all.len(), 1, "{argv:?}: {all:?}");
    all.remove(0)
}

#[test]
fn each_verb_that_opens_or_acts_is_one_window_action() {
    let cases: Vec<(&[&str], Value)> = vec![
        (
            &["open", "localhost:3000"],
            json!({"action": "open", "url": "localhost:3000"}),
        ),
        (
            &[
                "term",
                "python3 -q",
                "--wait",
                ">>>",
                "--ms",
                "5000",
                "--in",
                "sub",
            ],
            json!({"action": "open", "kind": "terminal", "command": "python3 -q",
                "wait_for": ">>>", "yield_ms": 5000, "workdir": "sub"}),
        ),
        (
            &["term", "--beside", "t1", "--down"],
            json!({"action": "open", "kind": "terminal", "beside": "t1", "side": "down"}),
        ),
        (
            &["edit", "src/main.rs", "--line", "12"],
            json!({"action": "open", "kind": "file", "path": "src/main.rs", "line": 12}),
        ),
        (
            &["goto", "w2", "localhost:3000"],
            json!({"action": "interact", "tab": "w2", "url": "localhost:3000"}),
        ),
        (
            &["click", "e12"],
            json!({"action": "interact", "click": "e12"}),
        ),
        (
            &["click", "w1", "e1", "@e2"],
            json!({"action": "interact", "tab": "w1", "click": ["e1", "@e2"]}),
        ),
        (
            &["click", "Create account"],
            json!({"action": "interact", "click": "Create account"}),
        ),
        (
            &["fill", "e4", "a b", "e5", "c"],
            json!({"action": "interact", "fill": {"e4": "a b", "e5": "c"}}),
        ),
        (
            &["select", "w1", "e7", "Large"],
            json!({"action": "interact", "tab": "w1", "select": {"e7": "Large"}}),
        ),
        (
            &["press", "Tab", "Enter", "--delay", "100"],
            json!({"action": "interact", "keys": ["Tab", "Enter"], "key_delay_ms": 100}),
        ),
        (
            &["type", "t1", "ls -la", "--enter"],
            json!({"action": "interact", "tab": "t1", "input": "ls -la", "keys": ["Enter"]}),
        ),
        (
            &["wait", "t3", "ready>"],
            json!({"action": "interact", "tab": "t3", "wait_for": "ready>"}),
        ),
        (
            &["wait", "Saved", "--ms", "500"],
            json!({"action": "interact", "wait_for": "Saved", "yield_ms": 500}),
        ),
    ];
    for (argv, expected) in cases {
        assert_eq!(one(argv), expected, "{argv:?}");
    }
}

#[test]
fn each_verb_that_reads_or_shows_is_one_window_action() {
    let cases: Vec<(&[&str], Value)> = vec![
        (&["read"], json!({"action": "interact"})),
        (
            &["read", "w1", "--text", "--from", "6000"],
            json!({"action": "interact", "tab": "w1", "read": true, "offset": 6000}),
        ),
        (
            &["eval", "w1", "document.title"],
            json!({"action": "interact", "tab": "w1", "script": "document.title"}),
        ),
        (
            &["size", "iPhone 15 Pro"],
            json!({"action": "interact", "size": "iPhone 15 Pro"}),
        ),
        (
            &["ask", "w1", "Sign in, then press Done."],
            json!({"action": "interact", "tab": "w1", "ask": "Sign in, then press Done."}),
        ),
        (
            &["shot", "w1", "--of", "main"],
            json!({"action": "screenshot", "tab": "w1", "selector": "main"}),
        ),
        (&["shot"], json!({"action": "screenshot"})),
        (
            &["show", "t2", "--zoom"],
            json!({"action": "show", "tab": "t2", "zoom": true}),
        ),
        (
            &["show", "t2", "--unzoom"],
            json!({"action": "show", "tab": "t2", "zoom": false}),
        ),
        (
            &["close", "w1", "t2"],
            json!({"action": "close", "tabs": ["w1", "t2"]}),
        ),
        (&["list"], json!({"action": "list"})),
        (
            &["click", "e6", "--dialog", "accept", "--answer", "yes"],
            json!({"action": "interact", "click": "e6", "dialog": "accept", "dialog_text": "yes"}),
        ),
    ];
    for (argv, expected) in cases {
        assert_eq!(one(argv), expected, "{argv:?}");
    }
    // Every verb of the table has a case above, or is `do`, which is steps of them.
    for verb in VERBS.iter().map(|verb| verb.name()) {
        assert!(
            verb == "do" || help().contains(&format!("llxterm {verb}")),
            "{verb}"
        );
    }
}

#[test]
fn a_tab_is_first_only_when_what_the_verb_needs_still_follows() {
    // `wait t3` waits for the text "t3"; with more after it, t3 is the tab.
    assert_eq!(
        one(&["wait", "t3"]),
        json!({"action": "interact", "wait_for": "t3"})
    );
    // A ref is never a tab, whatever follows it.
    assert_eq!(
        one(&["click", "e1", "e2"]),
        json!({"action": "interact", "click": ["e1", "e2"]})
    );
    // A verb that needs nothing more takes its first word as the tab.
    assert_eq!(
        one(&["read", "t1"]),
        json!({"action": "interact", "tab": "t1"})
    );
    // Several words of text are one text, as `echo` joins them.
    assert_eq!(
        one(&["type", "t1", "echo", "hi"]),
        json!({"action": "interact", "tab": "t1", "input": "echo hi"})
    );
}

#[test]
fn text_has_no_escapes_and_a_plus_inside_a_word_or_after_two_dashes_is_text() {
    // `\n` stays a backslash and an n: Enter is `--enter`.
    assert_eq!(
        one(&["type", "t1", r#"printf "a\n""#, "--enter"]),
        json!({"action": "interact", "tab": "t1", "input": r#"printf "a\n""#, "keys": ["Enter"]})
    );
    assert_eq!(
        one(&["press", "Ctrl+A"]),
        json!({"action": "interact", "keys": ["Ctrl+A"]})
    );
    // After `--` the rest is the step's values, a lone `+` and `--help` among them.
    assert_eq!(
        one(&["type", "t1", "--", "+"]),
        json!({"action": "interact", "tab": "t1", "input": "+"})
    );
    assert_eq!(
        one(&["type", "t1", "--", "--help"]),
        json!({"action": "interact", "tab": "t1", "input": "--help"})
    );
}

#[test]
fn a_plus_joins_steps_and_do_takes_them_from_stdin_a_line_each() {
    let joined = parsed(&["click", "e12", "+", "wait", "Saved", "+", "read"]).unwrap();
    assert_eq!(
        joined,
        [
            json!({"action": "interact", "click": "e12"}),
            json!({"action": "interact", "wait_for": "Saved"}),
            json!({"action": "interact"}),
        ]
    );
    let given = parsed(&["do", "click e12", "wait 'a + b'"]).unwrap();
    assert_eq!(given[1], json!({"action": "interact", "wait_for": "a + b"}));

    let heredoc = "click e12\n# a comment\n\nfill e4 \"it's 'quoted'\" e5 $HOME\nwait 'a + b; c'\n";
    let argv = ["do".to_string(), "-".to_string()];
    let read = steps(&argv, &mut || Ok(heredoc.to_string())).unwrap();
    assert_eq!(
        read,
        [
            json!({"action": "interact", "click": "e12"}),
            json!({"action": "interact", "fill": {"e4": "it's 'quoted'", "e5": "$HOME"}}),
            json!({"action": "interact", "wait_for": "a + b; c"}),
        ]
    );
    // A script's text from stdin, where quoting it would be the work.
    let argv = ["eval".to_string(), "w1".to_string(), "-".to_string()];
    let script = steps(&argv, &mut || Ok("document.title\n".to_string())).unwrap();
    assert_eq!(
        script,
        [json!({"action": "interact", "tab": "w1", "script": "document.title"})]
    );
}

#[test]
fn a_step_string_splits_as_a_shell_would_with_no_substitution() {
    assert_eq!(
        words(r#"wait "a b" 'c "d"' e\ f"#).unwrap(),
        ["wait", "a b", r#"c "d""#, "e f"]
    );
    assert_eq!(words(r"type 'a\nb'").unwrap(), ["type", r"a\nb"]);
    assert!(words("wait 'open").unwrap_err().contains("quote"));
    assert_eq!(
        line("click \"Create account\"").unwrap(),
        json!({"action": "interact", "click": "Create account"})
    );
}

#[test]
fn a_usage_error_says_the_commands_synopsis() {
    let why = parsed(&["clik", "e1"]).unwrap_err();
    assert!(
        why.contains("no command `clik`") && why.contains("did you mean `click`"),
        "{why}"
    );
    let why = parsed(&["click"]).unwrap_err();
    assert!(why.contains("llxterm click [TAB] REF"), "{why}");
    let why = parsed(&["fill", "e4"]).unwrap_err();
    assert!(why.contains("e4 has no text"), "{why}");
    let why = parsed(&["click", "e1", "--bogus"]).unwrap_err();
    assert!(why.contains("--bogus"), "{why}");
    let why = parsed(&["click", "e1", "+"]).unwrap_err();
    assert!(why.contains("nothing after `+`"), "{why}");
    let why = parsed(&["click", "e1", "+", "clik", "e2"]).unwrap_err();
    assert!(why.starts_with("step 2: "), "{why}");
    let why = parsed(&["click", "e1", "+", "do", "-"]).unwrap_err();
    assert!(why.contains("do"), "{why}");
    let why = parsed(&["wait", "--ms", "x", "Saved"]).unwrap_err();
    assert!(why.contains("--ms"), "{why}");
}

#[test]
fn the_guide_topic_shows_every_verb_from_the_one_table() {
    let topic = guide();
    assert!(topic.len() <= 4096, "{} bytes", topic.len());
    for verb in &VERBS {
        assert!(topic.contains(verb.synopsis()), "{}", verb.synopsis());
    }
    assert!(topic.contains("llxterm do - <<'EOF'"), "{topic}");
}
