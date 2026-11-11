//! Tests of the shapes of keys: each hidden whole and named, ordinary words left as they are.

use super::hidden;

#[test]
fn a_key_is_hidden_whole_and_named_for_what_it_opens() {
    let token = format!("ghp_{}", "a1B2".repeat(9));
    assert_eq!(
        hidden(&format!("token: {token}\nnext")),
        "token: [hidden: GitHub token]\nnext"
    );
    let cases = [
        (format!("sk-ant-api03-{}", "aB3-".repeat(22)), "API key"),
        (format!("sk-proj-{}", "x9Yz".repeat(10)), "API key"),
        (
            format!("AIza{}", "Sy0_".repeat(8) + "abc"),
            "Google API key",
        ),
        ("AKIAIOSFODNN7EXAMPLE".to_string(), "AWS access key"),
        (format!("github_pat_{}", "Ab1_".repeat(20)), "GitHub token"),
        (
            format!("xoxb-1234567890-{}", "ab12".repeat(6)),
            "Slack token",
        ),
        (format!("sk_live_{}", "4eC3".repeat(6)), "Stripe key"),
        (format!("glpat-{}", "Zx9-".repeat(5)), "GitLab token"),
        (format!("npm_{}", "q7W8".repeat(9)), "npm token"),
    ];
    for (key, name) in cases {
        assert_eq!(
            hidden(&format!("export K={key};")),
            format!("export K=[hidden: {name}];"),
            "{key}"
        );
    }
}

#[test]
fn a_signed_token_and_a_private_key_are_hidden() {
    let jwt = format!(
        "eyJ{}.eyJ{}.{}",
        "hbGci".repeat(4),
        "zdWIi".repeat(6),
        "sig_-".repeat(8)
    );
    assert_eq!(
        hidden(&format!("Authorization: Bearer {jwt}")),
        "Authorization: Bearer [hidden: signed token]"
    );
    let pem = "-----BEGIN OPENSSH PRIVATE KEY-----\nb3BlbnNzaC1rZXk\nAAAAB3Nza\n-----END OPENSSH PRIVATE KEY-----";
    assert_eq!(
        hidden(&format!("$ cat id\n{pem}\n$")),
        "$ cat id\n[hidden: private key]\n$"
    );
    // A screen that ends before the key does shows none of what it has of it.
    let cut = "-----BEGIN RSA PRIVATE KEY-----\nMIIEow\nIBAAKC";
    assert_eq!(hidden(cut), "[hidden: private key]");
}

#[test]
fn ordinary_words_stay_as_they_are() {
    for text in [
        "pip install sk-learn-contrib-extension-tools",
        "the task-sk-12345678901234567890abc id",
        "ghp_short",
        "see AKIA in the docs",
        "eyJhbGciOi is how one starts",
        "-----BEGIN CERTIFICATE-----\nMIID\n-----END CERTIFICATE-----",
        "disk-usage npm_config_cache=/tmp xoxb- AIza",
        "",
    ] {
        assert_eq!(hidden(text), text);
    }
}

#[test]
fn every_key_of_a_text_is_hidden() {
    let (one, two) = (format!("ghp_{}", "a1B2".repeat(9)), "AKIAIOSFODNN7EXAMPLE");
    assert_eq!(
        hidden(&format!("{one} {two}\u{2014}{one}")),
        "[hidden: GitHub token] [hidden: AWS access key]\u{2014}[hidden: GitHub token]"
    );
}
