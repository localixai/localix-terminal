//! Tests of `client`.

use std::io::Write as _;
use std::os::unix::net::UnixStream;

use super::*;
use crate::wire::{Message, RpcError, code};

#[test]
fn a_connection_the_app_refused_is_reported_with_its_reason_not_as_another_protocol() {
    let (mut app, client) = UnixStream::pair().unwrap();
    let refusal = RpcError::new(code::UNAVAILABLE, "64 connections are open: try again");
    app.write_all(&Message::failed(Value::Null, refusal).encode(None))
        .unwrap();
    let notes: Notes = Arc::new(|_, _| {});
    let Err(why) = Client::over(client, &notes) else {
        panic!("a refused connection shook hands");
    };
    assert_eq!(why, "64 connections are open: try again");
}
