//! Tests of `upgrade`.

use std::os::unix::fs::PermissionsExt;
use std::time::Duration;

use super::*;
use crate::testing::Scratch;

fn script(scratch: &Scratch, name: &str, body: &str) -> PathBuf {
    let path = scratch.file(name, format!("#!/bin/sh\n{body}\n"));
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path
}

#[expect(
    clippy::disallowed_methods,
    reason = "the host writes later: a test has no event to wait on"
)]
fn pause(time: Duration) {
    std::thread::sleep(time);
}

/// `from` put in place of `to` as a new file.
fn replace(from: &Path, to: &Path) {
    std::fs::rename(from, to).unwrap();
}

#[test]
fn only_another_build_that_answers_and_has_settled_is_due() {
    let scratch = Scratch::new();
    let exe = script(&scratch, "exe", "echo llx-desk 0.1.0");
    let mut upgrade = Upgrade::new(exe.clone(), Duration::ZERO);
    assert!(!upgrade.due(), "the build running is not due");

    replace(&script(&scratch, "silent", "exit 1"), &exe);
    assert!(!upgrade.due(), "a build that fails is not due");

    replace(&script(&scratch, "other", "echo something else"), &exe);
    assert!(!upgrade.due(), "another program is not due");

    replace(&script(&scratch, "next", "echo llx-desk 0.2.0"), &exe);
    let mut waiting = Upgrade {
        settle: Duration::from_secs(60),
        ..Upgrade::new(scratch.path().join("none"), Duration::ZERO)
    };
    waiting.exe.clone_from(&exe);
    assert!(!waiting.due(), "a file just written has not settled");
    assert!(upgrade.due());
}

#[test]
fn the_idle_check_runs_while_nothing_waits_and_never_inside_a_message() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let (mut host, server) = std::os::unix::net::UnixStream::pair().unwrap();
    let idled = AtomicUsize::new(0);
    let mut waiting = Waiting {
        input: server,
        every: Duration::from_millis(10),
        idle: || {
            idled.fetch_add(1, Ordering::SeqCst);
        },
        between: true,
    };
    let mut buf = [0u8; 64];
    let later = |mut host: std::os::unix::net::UnixStream, bytes: &'static [u8]| {
        std::thread::Builder::new()
            .name("host-writes".into())
            .spawn(move || {
                pause(Duration::from_millis(80));
                io::Write::write_all(&mut host, bytes).unwrap();
                host
            })
            .unwrap()
    };

    // Between messages, with nothing to read: it idles until something comes.
    let writer = later(host, b"{\"half\":");
    assert_eq!(waiting.read(&mut buf).unwrap(), 8);
    host = writer.join().unwrap();
    let before = idled.load(Ordering::SeqCst);
    assert!(before > 0);

    // Inside a message: it waits for the rest and never idles.
    let writer = later(host, b"1}\n");
    assert_eq!(waiting.read(&mut buf).unwrap(), 3);
    host = writer.join().unwrap();
    assert_eq!(idled.load(Ordering::SeqCst), before);

    // The host gone: the read says so.
    drop(host);
    assert_eq!(waiting.read(&mut buf).unwrap(), 0);
}
