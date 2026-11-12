//! Tests of `update::release`.

use super::*;

/// The latest release as the host answers, cut to what is read of it and a little more.
const LATEST: &str = r###"{
  "tag_name": "v0.2.0",
  "name": "Localix Terminal 0.2.0",
  "draft": false,
  "prerelease": false,
  "body": "- **macOS 12+, Apple silicon:** [Localix-Terminal-macos.dmg](https://example.com/x.dmg), signed.\n\n## Added\n\n- The app updates itself.\n",
  "assets": [{"name": "Localix-Terminal-macos.zip", "size": 9000111}, {"name": "SHA256SUMS", "size": 300}]
}"###;

#[test]
fn a_version_is_three_numbers_and_a_later_one_is_newer() {
    let version = |text: &str| Version::read(text);
    assert_eq!(version("0.1.5"), Some(Version(0, 1, 5)));
    assert_eq!(version("v0.1.5"), Some(Version(0, 1, 5)));
    // A build's own mark (`0.1.5+4089808-1794411590`) is of the same version.
    assert_eq!(version("0.1.5+4089808-1794411590"), Some(Version(0, 1, 5)));
    assert!(version("0.1.10") > version("0.1.9"));
    assert!(version("0.2.0") > version("0.1.99"));
    assert!(version("1.0.0") > version("0.99.99"));
    // What is not a release's version is none: a pre-release, a word, a number too many.
    for odd in [
        "0.2.0-beta.1",
        "latest",
        "1.2",
        "1.2.3.4",
        "",
        "v",
        "1.-2.3",
        "1.2.x",
    ] {
        assert_eq!(version(odd), None, "{odd}");
    }
    assert_eq!(Version(0, 2, 0).to_string(), "0.2.0");
}

#[test]
fn the_latest_release_is_read_with_its_notes() {
    let release = Release::read(LATEST).unwrap();
    assert_eq!(release.version, Version(0, 2, 0));
    assert!(release.notes.starts_with("- **macOS 12+"));
    assert!(release.notes.ends_with("- The app updates itself."));
    assert_eq!(release.size("Localix-Terminal-macos.zip"), Some(9_000_111));
    assert_eq!(release.size("Localix-Terminal-macos.dmg"), None);
    // A draft or a pre-release is not one to update to, and neither is what does not read.
    let draft = LATEST.replace(r#""draft": false"#, r#""draft": true"#);
    assert!(Release::read(&draft).is_err());
    let early = LATEST.replace(r#""prerelease": false"#, r#""prerelease": true"#);
    assert!(Release::read(&early).is_err());
    assert!(Release::read(&LATEST.replace("v0.2.0", "nightly")).is_err());
    assert!(Release::read("<html>rate limited</html>").is_err());
    // Notes are held to a length a person reads, cut at a line's end.
    let long = LATEST.replace("itself.", &format!("itself.{}", "\\n- More.".repeat(2000)));
    let notes = Release::read(&long).unwrap().notes;
    assert!(
        notes.len() <= NOTES_MOST && notes.ends_with("- More."),
        "{}",
        notes.len()
    );
}

#[test]
fn only_a_newer_release_is_an_update() {
    let release = Release::read(LATEST).unwrap();
    assert!(release.newer_than("0.1.5"));
    assert!(release.newer_than("0.1.5+4089808-1794411590"));
    assert!(!release.newer_than("0.2.0"));
    assert!(!release.newer_than("0.3.0"));
    // A build whose own version does not read never updates: it is not a release's.
    assert!(!release.newer_than("dev"));
}

#[test]
fn a_file_s_checksum_is_the_one_listed_for_its_name() {
    let sums = "\
0f3a  llxterm-x86_64-unknown-linux-gnu.tar.gz
a3b1c5d7e9f00112233445566778899aabbccddeeff001122334455667788990  Localix-Terminal-macos.zip
ffff  Localix-Terminal-macos.dmg
";
    assert_eq!(
        checksum(sums, "Localix-Terminal-macos.zip").as_deref(),
        Some("a3b1c5d7e9f00112233445566778899aabbccddeeff001122334455667788990")
    );
    // The name whole, a checksum whole: 64 hex digits, as the release writes them.
    assert_eq!(checksum(sums, "Localix-Terminal-macos"), None);
    assert_eq!(checksum(sums, "Localix-Terminal-macos.dmg"), None);
    assert_eq!(checksum("", "Localix-Terminal-macos.zip"), None);
    // `sha256sum`'s mark of a binary file before the name is the same file.
    let binary = sums.replace(
        "  Localix-Terminal-macos.zip",
        " *Localix-Terminal-macos.zip",
    );
    assert!(checksum(&binary, "Localix-Terminal-macos.zip").is_some());
}

#[test]
fn each_system_has_its_archive_or_none() {
    assert_eq!(
        archive("macos", "aarch64"),
        Some("Localix-Terminal-macos.zip")
    );
    assert_eq!(
        archive("linux", "x86_64"),
        Some("llxterm-x86_64-unknown-linux-gnu.tar.gz")
    );
    assert_eq!(archive("macos", "x86_64"), None);
    assert_eq!(archive("linux", "aarch64"), None);
}

#[test]
fn a_release_s_files_are_under_its_tag() {
    let from = From::default();
    assert_eq!(
        from.latest,
        "https://api.github.com/repos/localixai/localix-terminal/releases/latest"
    );
    assert_eq!(
        from.file(Version(0, 2, 0), "SHA256SUMS"),
        "https://github.com/localixai/localix-terminal/releases/download/v0.2.0/SHA256SUMS"
    );
    // Another place holds both under one address (a mirror, a test's folder).
    let other = From::at("file:///tmp/releases/");
    assert_eq!(other.latest, "file:///tmp/releases/latest");
    assert_eq!(
        other.file(Version(0, 2, 0), "Localix-Terminal-macos.zip"),
        "file:///tmp/releases/download/v0.2.0/Localix-Terminal-macos.zip"
    );
}
