//! Tests of `wake`: a process's end heard as it happens.

use std::sync::mpsc::sync_channel;
use std::time::{Duration, Instant};

use super::on_exit;

#[test]
fn a_process_s_end_is_heard_as_it_ends_and_one_gone_cannot_be_waited_on() {
    let mut child = std::process::Command::new("/bin/sleep")
        .arg("0.3")
        .spawn()
        .unwrap();
    let pid = libc::pid_t::try_from(child.id()).unwrap();
    let (told, heard) = sync_channel(1);
    let began = Instant::now();
    on_exit(pid, move || {
        let _ = told.send(());
    })
    .unwrap();
    heard.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(
        began.elapsed() >= Duration::from_millis(250),
        "{:?}",
        began.elapsed()
    );
    child.wait().unwrap();
    // Reaped, it is no process to wait on.
    assert!(on_exit(pid, || {}).is_err());
}
