//! A closed connection drains its pending calls and keeps the same reason for future callers.

use super::*;

#[test]
fn every_pending_call_hears_why_the_connection_ended() {
    for ended in [
        Ended::Gone("the app stopped".into()),
        Ended::Foreign("another protocol".into()),
        Ended::Refused(RpcError::new(crate::wire::code::UNAVAILABLE, "too busy")),
    ] {
        let shared = Shared::default();
        let (first, a) = sync_channel(1);
        let (second, b) = sync_channel(1);
        {
            let mut pending = crate::lock(&shared.pending);
            pending.waiting.insert(1, first);
            pending.waiting.insert(2, second);
        }
        end(&shared, &ended);
        for answer in [a, b] {
            assert_eq!(answer.try_recv().unwrap().unwrap_err(), ended.error());
        }
        let pending = crate::lock(&shared.pending);
        assert!(pending.waiting.is_empty());
        assert_eq!(pending.closed.as_ref().unwrap().error(), ended.error());
    }
}
