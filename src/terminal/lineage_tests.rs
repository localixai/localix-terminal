//! Tests of a process's lineage: a program is known by the one it descends from.

use super::{ancestor, parent, program_among};

fn own() -> libc::pid_t {
    libc::pid_t::try_from(std::process::id()).unwrap()
}

#[test]
fn a_program_started_through_shells_descends_from_the_one_that_started_them() {
    // A shell that starts a shell that starts the program: the program's pid is printed.
    let mut shell = std::process::Command::new("sh")
        .args(["-c", "sh -c 'sleep 20 & echo $!; wait'; :"])
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let mut line = String::new();
    let out = shell.stdout.take().unwrap();
    std::io::BufRead::read_line(&mut std::io::BufReader::new(out), &mut line).unwrap();
    let program: libc::pid_t = line.trim().parse().unwrap();
    let started = libc::pid_t::try_from(shell.id()).unwrap();

    assert_eq!(parent(started), Some(own()));
    assert_eq!(ancestor(program, &[own()]), Some(own()));
    // The nearest of several is the answer, and a process is its own.
    assert_eq!(ancestor(program, &[own(), started]), Some(started));
    assert_eq!(ancestor(started, &[started]), Some(started));
    // SAFETY: kill only signals the process this test started.
    unsafe { libc::kill(program, libc::SIGKILL) };
    let _ = shell.kill();
    let _ = shell.wait();
}

#[test]
fn a_program_of_no_one_named_descends_from_none() {
    assert_eq!(ancestor(own(), &[]), None);
    // The first process is nobody's child here.
    assert_eq!(ancestor(1, &[own()]), None);
}

#[test]
fn the_program_a_process_comes_from_is_the_nearest_one_known_with_its_own_folder() {
    let me = own();
    let mine = super::super::foreground::program(me).unwrap();
    // Itself when it is known, in the folder it works in; none where nothing up to the first
    // process is known.
    let here = std::env::current_dir().unwrap().canonicalize().unwrap();
    let (name, folder) = program_among(me, |name| name == mine).unwrap();
    assert_eq!(name, mine);
    assert_eq!(
        folder.and_then(|folder| folder.canonicalize().ok()),
        Some(here)
    );
    assert_eq!(program_among(me, |_| false), None);
    // Past itself to what started it.
    let above = (parent(me))
        .and_then(super::super::foreground::program)
        .unwrap();
    let found = program_among(me, |name| name == above).map(|(name, _)| name);
    assert_eq!(found, Some(above));
}
