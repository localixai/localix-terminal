//! The `board-file` projector: a project's board written out to `.localix/board.json` in the
//! project, for the plugin to read and for git to keep. The file is derived data: it is written
//! from the board's row after each change, and all of them again when the desk starts, so a file
//! deleted or edited by hand is put back as the board is.

use std::path::Path;
use std::sync::Weak;

use super::Desk;
use crate::board;
use crate::feed::{Consumer, Entity, Kind, Notice};
use crate::projects::ProjectId;

pub(super) struct BoardFile {
    pub(super) desk: Weak<Desk>,
}

impl BoardFile {
    /// Project `project`'s board, as its row holds it now, written to its file.
    fn export(desk: &Desk, project: ProjectId, path: &Path) -> Result<(), String> {
        let kept = (desk.feed.store()).read(|db| board::store::load(db, project))?;
        kept.map_or(Ok(()), |board| board::store::export(path, &board))
    }
}

impl Consumer for BoardFile {
    fn name(&self) -> &'static str {
        "board-file"
    }

    fn kind(&self) -> Kind {
        Kind::Projector
    }

    fn handle(&mut self, notice: &Notice) -> Result<(), String> {
        let Some(desk) = self.desk.upgrade() else {
            return Ok(());
        };
        match &notice.entity {
            Entity::Board { project } => match desk.project_by_id(*project) {
                Some(found) => Self::export(&desk, found.id, &found.path),
                None => Ok(()),
            },
            Entity::Said { .. }
            | Entity::Pane { .. }
            | Entity::Window { .. }
            | Entity::Workspace { .. }
            | Entity::Tab { .. }
            | Entity::Terminal { .. }
            | Entity::Ask { .. }
            | Entity::Ledger { .. }
            | Entity::Projects => Ok(()),
        }
    }

    fn reconcile(&mut self) {
        let Some(desk) = self.desk.upgrade() else {
            return;
        };
        let projects = (desk.feed.store()).read(board::store::projects);
        for (project, path) in projects.unwrap_or_default() {
            if let Err(why) = Self::export(&desk, project, &path) {
                log::warn!("board-file: {why}");
            }
        }
    }
}
