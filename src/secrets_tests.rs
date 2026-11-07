//! Tests of `secrets`.

use std::os::unix::fs::PermissionsExt;

use super::*;

#[test]
fn a_saved_secret_is_read_back_whole_from_a_file_only_its_owner_reads() {
    let scratch = crate::testing::Scratch::new();
    let dir = scratch.path().join("secrets");
    let path = save_in(&dir, "SITE_PW", "  hunter2 with spaces \n").unwrap();
    assert_eq!(
        read_in(&dir, "SITE_PW").as_deref(),
        Some("  hunter2 with spaces ")
    );
    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(
        std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777,
        0o700
    );
    assert_eq!(names_in(&dir), ["SITE_PW"]);
    assert!(forget_in(&dir, "SITE_PW").unwrap());
    assert!(!forget_in(&dir, "SITE_PW").unwrap());
    assert_eq!(read_in(&dir, "SITE_PW"), None);
}

#[test]
fn a_bad_name_or_a_short_value_is_refused_and_no_name_leaves_the_directory() {
    let scratch = crate::testing::Scratch::new();
    let dir = scratch.path().join("secrets");
    assert!(save_in(&dir, "../key", "long enough").is_err());
    assert!(save_in(&dir, "PIN", "123").is_err());
    assert_eq!(read_in(&dir, "../desk.key"), None);
    assert!(!valid_name(""));
    assert!(valid_name("work-mail_2"));
}

#[test]
fn a_text_names_its_secrets_once_each_in_order() {
    assert_eq!(
        named("{{secret:B}} then {{secret:A}} and {{secret:B}} {{secret:../x}} {{secret:open"),
        ["B", "A"]
    );
    assert!(named("no secrets here").is_empty());
}

#[test]
fn a_named_secret_is_put_in_and_every_answer_after_shows_its_reference() {
    let scratch = crate::testing::Scratch::new();
    let dir = scratch.path().join("secrets");
    save_in(&dir, "PW", "p@ss word!").unwrap();
    let vault = Vault::default();
    assert_eq!(
        vault.fill_in(Some(&dir), "sudo -S <<< {{secret:PW}}\n"),
        Ok("sudo -S <<< p@ss word!\n".into())
    );
    assert_eq!(
        vault.mask("typed p@ss word! and sent p%40ss+word%21"),
        "typed {{secret:PW}} and sent {{secret:PW}}"
    );
    // As a program may write it: base64 (plain or for an address, padded or not), JSON-escaped.
    assert_eq!(
        vault.mask("cEBzcyB3b3JkIQ== cEBzcyB3b3JkIQ \"p@ss word!\""),
        "{{secret:PW}}== {{secret:PW}} \"{{secret:PW}}\""
    );
    save_in(&dir, "Q", "a\"b\\c/~~~?").unwrap();
    vault.fill_in(Some(&dir), "{{secret:Q}}").unwrap();
    assert_eq!(
        vault.mask(r#"{"pw":"a\"b\\c/~~~?"} YSJiXGMvfn5-Pw YSJiXGMvfn5+Pw"#),
        r#"{"pw":"{{secret:Q}}"} {{secret:Q}} {{secret:Q}}"#
    );
    let missing = vault.fill_in(Some(&dir), "{{secret:NOPE}}").unwrap_err();
    assert!(missing.contains("llx-desk secret NOPE < FILE"), "{missing}");
}

#[test]
fn a_secret_is_masked_as_a_pages_script_escapes_it_and_inside_json_its_own_quotes_stay() {
    let known = vec![("PW".to_string(), "my pass!".to_string())];
    assert_eq!(
        masked(&known, "q=my+pass%21 u=my%20pass! raw my pass!"),
        "q={{secret:PW}} u={{secret:PW}} raw {{secret:PW}}"
    );
    let quoted = vec![("Q".to_string(), "\"quoted\"".to_string())];
    assert_eq!(
        masked(&quoted, r#"{"v":"\"quoted\""}"#),
        r#"{"v":"{{secret:Q}}"}"#
    );
}

#[test]
fn a_secret_inside_another_secrets_reference_is_left_alone() {
    let known = vec![
        ("PASSWORD".to_string(), "Tr0ub4dor".to_string()),
        ("SHORT".to_string(), "PASS".to_string()),
    ];
    assert_eq!(
        masked(&known, "Tr0ub4dor then {{secret:PASSWORD}} and PASS"),
        "{{secret:PASSWORD}} then {{secret:PASSWORD}} and {{secret:SHORT}}"
    );
}

#[test]
fn a_value_that_holds_a_reference_is_put_in_as_it_is_and_never_read_again() {
    let scratch = crate::testing::Scratch::new();
    let dir = scratch.path().join("secrets");
    save_in(&dir, "ONE", "see {{secret:TWO}} here").unwrap();
    save_in(&dir, "TWO", "second-value").unwrap();
    let vault = Vault::default();
    assert_eq!(
        vault.fill_in(Some(&dir), "{{secret:TWO}} {{secret:ONE}}"),
        Ok("second-value see {{secret:TWO}} here".into())
    );
}

#[test]
fn a_secret_saved_again_leaves_no_file_beside_it_and_a_name_is_never_the_temporary_one() {
    let scratch = crate::testing::Scratch::new();
    let dir = scratch.path().join("secrets");
    save_in(&dir, "PW", "first-value").unwrap();
    save_in(&dir, "PW", "second-value").unwrap();
    assert_eq!(read_in(&dir, "PW").as_deref(), Some("second-value"));
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
}
