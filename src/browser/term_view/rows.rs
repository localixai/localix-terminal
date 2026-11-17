//! What a view keeps of the rows it drew: one picture for each look a row has (its layout's key),
//! and which key each row in view has by its id. A row the frame says is clean keeps its
//! picture; a dirty row that draws as one already kept (a row scrolled back into view, a blank
//! row) takes that one; only what looks new is drawn. What leaves the view is let go of.

use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};

use super::layout::{self, RowPaint};
use super::theme::Theme;
use crate::surface::{Frame, Row, RowId};

pub(crate) struct Cache<I> {
    by_id: HashMap<RowId, u64>,
    pictures: HashMap<u64, I>,
}

impl<I> Default for Cache<I> {
    fn default() -> Self {
        Self {
            by_id: HashMap::new(),
            pictures: HashMap::new(),
        }
    }
}

/// How many rows a frame drew anew and how many it kept.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Painted {
    pub drawn: usize,
    pub kept: usize,
}

impl<I> Cache<I> {
    /// Brings the pictures up to `frame`, drawing with `draw` what no picture shows yet.
    pub(crate) fn keep(
        &mut self,
        frame: &Frame,
        theme: Theme,
        mut draw: impl FnMut(&Row, &RowPaint) -> Option<I>,
    ) -> Painted {
        let mut painted = Painted::default();
        let mut used = HashSet::with_capacity(frame.rows.len());
        for row in &frame.rows {
            let known = self.by_id.get(&row.id).copied();
            let key = match (row.dirty, known) {
                (false, Some(key)) if self.pictures.contains_key(&key) => key,
                _ => {
                    let (paint, key) = layout::row(row, frame, theme);
                    match self.pictures.entry(key) {
                        Entry::Occupied(_) => painted.kept += 1,
                        Entry::Vacant(slot) => {
                            if let Some(picture) = draw(row, &paint) {
                                slot.insert(picture);
                                painted.drawn += 1;
                            }
                        }
                    }
                    self.by_id.insert(row.id, key);
                    used.insert(key);
                    continue;
                }
            };
            painted.kept += 1;
            used.insert(key);
        }
        let in_view: HashSet<RowId> = frame.rows.iter().map(|row| row.id).collect();
        self.by_id.retain(|id, _| in_view.contains(id));
        self.pictures.retain(|key, _| used.contains(key));
        painted
    }

    /// The picture of the row `id` and the key of what it shows, when it is in view.
    pub(crate) fn picture(&self, id: RowId) -> Option<(u64, &I)> {
        let key = *self.by_id.get(&id)?;
        self.pictures.get(&key).map(|picture| (key, picture))
    }

    /// Every picture kept.
    pub(crate) fn pictures(&self) -> impl Iterator<Item = &I> {
        self.pictures.values()
    }

    /// Lets go of every picture: the next frame draws all of them.
    pub(crate) fn clear(&mut self) {
        self.by_id.clear();
        self.pictures.clear();
    }
}

#[cfg(test)]
#[path = "rows_tests.rs"]
mod tests;
