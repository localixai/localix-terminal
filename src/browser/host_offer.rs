//! The offer bar's buttons (offer.rs): the one that does it, which the desk carries out; the
//! other puts the offer off.

use super::On;

impl On<'_> {
    /// The bar's button that does it (`take`), or the other.
    pub(super) fn offer(&mut self, take: bool) {
        let Some(open) = self.deck.as_ref() else {
            return;
        };
        let bar = open.offered();
        let taken = self.shared.offers.clicked(bar.as_ref(), take);
        if taken.is_some_and(|id| id.starts_with(crate::browser::updates::OF_UPDATE)) {
            self.shared.heard("", crate::browser::Heard::Update);
        }
        open.refresh(self.shared);
    }
}
