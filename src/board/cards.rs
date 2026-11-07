//! New cards: validated text, a fresh id, and their first history step.

use super::{Board, Card, Column, MAX_CARDS, MAX_NOTE, Step, heading, now, words};

impl Board {
    /// A new card at the bottom of Todo; its id.
    pub fn add(
        &mut self,
        title: &str,
        note: &str,
        tab: Option<&str>,
        by: &str,
    ) -> Result<String, String> {
        self.add_to(Column::Todo, title, note, tab, by)
    }

    /// A new card at the bottom of `column`, Todo or Backlog (an idea); its id.
    pub fn add_to(
        &mut self,
        column: Column,
        title: &str,
        note: &str,
        tab: Option<&str>,
        by: &str,
    ) -> Result<String, String> {
        if !matches!(column, Column::Todo | Column::Backlog) {
            return Err(format!(
                "a card is added to Todo or to Backlog, not to {}",
                column.title()
            ));
        }
        if self.cards.len() >= MAX_CARDS {
            return Err(format!(
                "the board holds {MAX_CARDS} cards: remove some first"
            ));
        }
        let title = heading(title)?;
        let note = if note.trim().is_empty() {
            String::new()
        } else {
            words(note, "note", MAX_NOTE)?
        };
        let id = format!("c{}", self.next);
        self.next = self
            .next
            .checked_add(1)
            .ok_or("the board has no card ids left")?;
        self.cards.push(Card {
            id: id.clone(),
            title,
            note,
            column,
            who: None,
            blocked: None,
            result: None,
            tab: tab.map(String::from),
            history: vec![Step {
                at: now(),
                by: by.to_string(),
                to: Some(column),
                text: None,
            }],
        });
        Ok(id)
    }
}
