//! Tests of `told`: what a session tells of the commands its shell ended.

use std::sync::{Arc, Mutex};
use std::time::Instant;

use super::super::{DEFAULT_WAIT, Session, Until, nap};

fn start(command: &str) -> Session {
    Session::start(command, &std::env::temp_dir(), None, None).unwrap()
}

#[test]
fn each_command_the_shell_ends_is_told_once_with_whether_the_persons_keys_typed_it() {
    // A shell's marks, said by hand: the first line typed by the person, its end said twice;
    // the second typed by an agent.
    let session = start(
        r#"sh -c 'printf "\033]133;A\007one"; read x; printf "\033]133;C\007\033]133;D;3\007\033]133;D;3\007\033]133;A\007two"; read y; printf "\033]133;C\007\033]133;D;0\007three"; sleep 20'"#,
    );
    let heard = Arc::new(Mutex::new(vec![]));
    let told = Arc::clone(&heard);
    session.on_command(move |ended, theirs| told.lock().unwrap().push((ended.code, theirs)));
    assert_eq!(session.wait(Some("one"), 0, DEFAULT_WAIT), Until::Matched);
    session.typed("x\r");
    assert_eq!(session.wait(Some("two"), 0, DEFAULT_WAIT), Until::Matched);
    session.type_in("y\n", &[]).unwrap();
    assert_eq!(session.wait(Some("three"), 0, DEFAULT_WAIT), Until::Matched);
    let deadline = Instant::now() + DEFAULT_WAIT;
    while heard.lock().unwrap().len() < 2 && Instant::now() < deadline {
        nap(20);
    }
    nap(100);
    assert_eq!(*heard.lock().unwrap(), [(Some(3), true), (Some(0), false)]);
}
