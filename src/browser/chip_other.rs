//! The tab held in a side's list, drawn where the pointer is: macOS only (chip.rs); elsewhere
//! the side's page draws its own card.

pub(super) struct Chip;

impl Chip {
    pub(super) fn lift(
        _side: &wry::WebView,
        _rect: (f64, f64, f64, f64),
        _grab: (f64, f64),
        _inner: Option<f64>,
    ) -> Self {
        Self
    }

    pub(super) fn follow(&self, _x: f64, _y: f64) {}

    pub(super) fn show(&self, _shown: bool) {}

    pub(super) fn height(&self) -> f64 {
        44.0
    }
}
