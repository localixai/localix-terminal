//! The offer bar's buttons (offer.rs): the one that does it, on a thread, the bar saying so
//! meanwhile; the other puts the offer off, or closes what came of it.

use super::On;

impl On<'_> {
    /// Integration `id` was read, on (`on`) or off: the bar is drawn again.
    pub(super) fn offered(&mut self, id: &'static str, on: bool) {
        self.shared.offers.known(id, on);
        if let Some(open) = self.deck.as_ref() {
            open.refresh(self.shared);
        }
    }

    /// The bar's button that does it (`take`), or the other.
    pub(super) fn offer(&mut self, take: bool) {
        let Some(open) = self.deck.as_ref() else {
            return;
        };
        let bar = open.offered();
        if let Some(id) = self.shared.offers.clicked(bar.as_ref(), take) {
            crate::browser::offer::take(id, self.shared, self.proxy);
        }
        open.refresh(self.shared);
    }
}
