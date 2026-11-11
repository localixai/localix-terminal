//! Tests of a socket's other end: the process there is known.

use super::peer_pid;

#[test]
fn the_other_end_of_a_socket_is_known_by_its_process() {
    let (near, _far) = std::os::unix::net::UnixStream::pair().unwrap();
    let own = libc::pid_t::try_from(std::process::id()).unwrap();
    assert_eq!(peer_pid(&near), Some(own));
}
