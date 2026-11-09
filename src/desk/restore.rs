//! Reopen a project's panes from the ledger, preserving their rows and layout.

use super::{Call, Desk};
use crate::ledger::{self, Kind, Row};

impl Desk {
    /// Pane `row` of the history opened again for the person, as `call` (of the project the
    /// window keys `group`).
    pub(super) fn again(&self, call: &Call, group: &str, row: Row) -> Result<(), String> {
        let tab = (self.feed.store().read(|db| ledger::get(db, row)))?
            .ok_or("that tab is no longer in the history")?;
        self.again_tab(call, group, &tab)
    }

    /// The person went to `call`'s project, which has no tab open: what it had open when the
    /// app last quit opens again, the same panes in the same tabs, places and splits; else their
    /// shell in its folder.
    pub(super) fn reopen_project(&self, call: &Call, group: &str) -> Result<(), String> {
        let open: std::collections::HashSet<Row> = (crate::lock(&self.ledger).values())
            .map(|(row, _)| *row)
            .collect();
        let project = call.project.id;
        let (left, order) = (self.feed.store()).read(|db| {
            let left = ledger::left_open(db, project)?;
            Ok((left, Self::panes_in_order(db, project)?))
        })?;
        let mut left: Vec<ledger::Pane> = (left.into_iter())
            .filter(|tab| !open.contains(&tab.row))
            .collect();
        if left.is_empty() {
            return self.person_terminal(call, group, "");
        }
        left.sort_by_key(|tab| order.iter().position(|row| *row == tab.row));
        // They are this run's now: nothing of the project's closes as the first of them opens.
        crate::lock(&self.begun).insert(project);
        for tab in &left {
            if let Err(why) = self.again_tab(call, group, tab) {
                log::warn!("a tab of {} did not open again: {why}", call.project.name);
                let now = crate::board::now();
                let closed = self.feed.write(|write| {
                    ledger::close(write.tx(), tab.row, now)?;
                    let ledger = crate::feed::Entity::Ledger { project };
                    write.record(&ledger, crate::feed::Op::Upserted)?;
                    Self::unplace_pane(write, tab.row, now)
                });
                if let Err(why) = closed {
                    log::warn!("it stays open in the ledger: {why}");
                }
            }
        }
        Ok(())
    }

    /// The pane `tab` opened again for the person: a page at its address, their shell in a terminal's
    /// folder (its program is not run again), the board. It is the same pane as before: its row,
    /// its history, its place in its tab.
    fn again_tab(&self, call: &Call, group: &str, tab: &ledger::Pane) -> Result<(), String> {
        let call = &Call {
            again: Some(tab.row),
            ..*call
        };
        match tab.kind {
            Kind::Page => {
                let url = (tab.url.as_deref())
                    .filter(|url| !url.is_empty())
                    .unwrap_or(crate::browser::HOME);
                (self.open(call, &serde_json::json!({"kind": "browser", "url": url})))
                    .map(|_| ())
                    .map_err(|failure| failure.message().to_string())
            }
            Kind::Terminal => {
                let cwd = (tab.cwd.clone())
                    .filter(|cwd| cwd.is_dir())
                    .unwrap_or_else(|| call.cwd.to_path_buf());
                let before = (self.feed.store().read(|db| ledger::screen(db, tab.row)))
                    .ok()
                    .flatten()
                    .unwrap_or_default();
                self.person_terminal_after(&Call { cwd: &cwd, ..*call }, group, "", &before)
            }
            Kind::Board => (self.board(call, &serde_json::json!({"op": "show"})))
                .map(|_| ())
                .map_err(|failure| failure.message().to_string()),
        }
    }
}
