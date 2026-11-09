//! AppKit's numbers, named once (macOS): the bits of a window's style, its collection behaviour
//! and a view's autoresizing mask, and the window levels and orderings the app uses. Each file
//! that sends AppKit one of these takes it from here.

/// NSWindowStyleMask: no frame.
pub(in crate::browser) const BORDERLESS: usize = 0;
/// NSWindowStyleMask: a panel that never makes the app active.
pub(in crate::browser) const NONACTIVATING_PANEL: usize = 1 << 7;

/// NSWindowCollectionBehavior: on every Space.
pub(in crate::browser) const CAN_JOIN_ALL_SPACES: usize = 1 << 0;
/// NSWindowCollectionBehavior: out of the window cycling (⌘`).
pub(in crate::browser) const IGNORES_CYCLE: usize = 1 << 6;
/// NSWindowCollectionBehavior: a window that may go full screen and be tiled.
pub(in crate::browser) const FULL_SCREEN_PRIMARY: usize = 1 << 7;
/// NSWindowCollectionBehavior: over a full-screen app.
pub(in crate::browser) const FULL_SCREEN_AUXILIARY: usize = 1 << 8;

/// NSAutoresizingMaskOptions: the left margin follows the superview.
pub(in crate::browser) const MIN_X_MARGIN: usize = 1 << 0;
/// NSAutoresizingMaskOptions: the width follows the superview.
pub(in crate::browser) const WIDTH_SIZABLE: usize = 1 << 1;
/// NSAutoresizingMaskOptions: the right margin follows the superview.
pub(in crate::browser) const MAX_X_MARGIN: usize = 1 << 2;
/// NSAutoresizingMaskOptions: the bottom margin follows the superview.
pub(in crate::browser) const MIN_Y_MARGIN: usize = 1 << 3;
/// NSAutoresizingMaskOptions: the height follows the superview.
pub(in crate::browser) const HEIGHT_SIZABLE: usize = 1 << 4;
/// NSAutoresizingMaskOptions: the top margin follows the superview.
pub(in crate::browser) const MAX_Y_MARGIN: usize = 1 << 5;

/// NSWindowLevel: the ordinary windows'.
pub(in crate::browser) const NORMAL_LEVEL: isize = 0;
/// NSWindowLevel: floating, above the ordinary windows.
pub(in crate::browser) const FLOATING_LEVEL: isize = 3;
/// NSWindowLevel (NSPopUpMenuWindowLevel): over the app's windows.
pub(in crate::browser) const POP_UP_MENU_LEVEL: isize = 101;

/// NSWindowOrderingMode: below the view named, or above it.
pub(in crate::browser) const BELOW: isize = -1;
pub(in crate::browser) const ABOVE: isize = 1;

#[cfg(test)]
mod tests {
    use super::*;

    /// The numbers are AppKit's: a name that drifted from its value would send another bit.
    #[test]
    fn each_name_holds_appkits_number() {
        assert_eq!((BORDERLESS, NONACTIVATING_PANEL), (0, 0x80));
        assert_eq!(
            CAN_JOIN_ALL_SPACES | IGNORES_CYCLE | FULL_SCREEN_AUXILIARY,
            0x141
        );
        assert_eq!(FULL_SCREEN_PRIMARY, 0x80);
        assert_eq!(WIDTH_SIZABLE | HEIGHT_SIZABLE, 18);
        assert_eq!(
            MIN_X_MARGIN | MAX_X_MARGIN | MIN_Y_MARGIN | MAX_Y_MARGIN,
            45
        );
        assert_eq!(
            (NORMAL_LEVEL, FLOATING_LEVEL, POP_UP_MENU_LEVEL),
            (0, 3, 101)
        );
        assert_eq!((BELOW, ABOVE), (-1, 1));
    }
}
