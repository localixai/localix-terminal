//! The relay: the feed's consumer that tells subscribers what changed, through [`Events`].
//!
//! It is a reactor reading notices, which carry refs only: what it tells it reads again from the
//! owner when it handles the notice, so a note delivered since (a call carried it, or it was
//! acknowledged) is not told, and a notice handled twice tells nothing new.

use std::sync::Weak;

use serde_json::json;

use super::Desk;
use crate::feed::{Consumer, Entity, Kind, Notice, Op};

pub(super) struct Relay {
    pub(super) desk: Weak<Desk>,
}

impl Consumer for Relay {
    fn name(&self) -> &'static str {
        "relay"
    }

    fn kind(&self) -> Kind {
        Kind::Reactor
    }

    fn handle(&mut self, notice: &Notice) -> Result<(), String> {
        let Some(desk) = self.desk.upgrade() else {
            return Ok(());
        };
        match (&notice.entity, notice.op) {
            (Entity::Said { owner, id }, Op::Upserted) => {
                let pending = desk.pending(owner);
                if let Some(said) = pending.iter().find(|said| said.id == *id) {
                    (desk.events).publish("comment/added", Some(owner), desk.note(owner, said));
                }
            }
            // A tab of no one's (the person's in no project's own) is told to no one.
            (
                Entity::Tab {
                    owner: Some(owner),
                    key,
                },
                Op::Deleted,
            ) => {
                (desk.events).publish("tab/closed", Some(owner), json!({"tab": key}));
            }
            // What subscribers are not told of.
            (Entity::Said { .. }, Op::Deleted)
            | (Entity::Tab { owner: None, .. }, Op::Deleted)
            | (Entity::Tab { .. }, Op::Upserted)
            | (
                Entity::Board { .. } | Entity::Terminal { .. } | Entity::Ask { .. },
                Op::Upserted | Op::Deleted,
            ) => {}
        }
        Ok(())
    }
}
