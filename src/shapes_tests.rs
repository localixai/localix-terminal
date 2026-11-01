//! Tests of `shapes`.

use super::mask;

#[test]
fn a_provider_key_is_masked_wherever_it_stands() {
    assert_eq!(
        mask("export OPENAI_API_KEY=sk-proj-4f9aXbQ2kLm8Zr7Tq1Wv"),
        "export OPENAI_API_KEY=[credential]"
    );
    assert_eq!(
        mask("token ghp_16C7e42F292c6912E7710c838347Ae178B4a here"),
        "token [credential] here"
    );
    assert_eq!(mask("id AKIAIOSFODNN7EXAMPLE."), "id [credential].");
    assert_eq!(mask("id AKIAIOSFODNN7EXAMPLES"), "id AKIAIOSFODNN7EXAMPLES");
}

#[test]
fn a_bearer_token_a_password_in_an_address_and_a_named_value_are_masked() {
    assert_eq!(
        mask("Authorization: Bearer eyJhbGciOiJIUzI1NiJ9.abc123def456"),
        "Authorization: Bearer [credential]"
    );
    assert_eq!(
        mask("git remote: https://ada:s3cr3tpass@github.com/x.git"),
        "git remote: https://ada:[credential]@github.com/x.git"
    );
    assert_eq!(
        mask(r#"{"password": "hunter2!", "user": "ada"}"#),
        r#"{"password": "[credential]", "user": "ada"}"#
    );
    assert_eq!(mask("DB_PASSWORD=Tr0ub4dor&3"), "DB_PASSWORD=[credential]");
}

#[test]
fn words_that_are_only_words_are_never_masked() {
    for text in [
        "Password: required",
        "Forgot your password?",
        "sk-hynix memory prices",
        "The secret garden",
        "token: none",
        "password = ********",
        "Book a trip to Zurich on 2026-11-05",
        "skip-navigation-link-for-screen-readers",
    ] {
        assert_eq!(mask(text), text);
    }
}

#[test]
fn a_secrets_reference_is_never_read_as_a_credential() {
    for text in [
        "{{secret:GITHUB_TOKEN}}",
        "password: {{secret:BANK_LOGIN1}}",
        "typed {{secret:API-KEY_2}} into it",
    ] {
        assert_eq!(mask(text), text);
    }
}

#[test]
fn every_credential_of_an_address_query_is_masked_and_the_rest_of_it_kept() {
    assert_eq!(
        mask("https://x.com/login?user=bob&password=hunter22x"),
        "https://x.com/login?user=bob&password=[credential]"
    );
    assert_eq!(
        mask("https://x.com/a?user=bob&api_key=hunter22x&z=1"),
        "https://x.com/a?user=bob&api_key=[credential]&z=1"
    );
    assert_eq!(
        mask("/cb?access_token=abc123def456&state=1"),
        "/cb?access_token=[credential]&state=1"
    );
    assert_eq!(
        mask("https://ada:s3cr3tpass@x.com/?password=hunter22x"),
        "https://ada:[credential]@x.com/?password=[credential]"
    );
}
