//! The fonts: a stack of names none of which is installed falls back on the system's monospace,
//! with a cell of whole sense (wider than nothing, taller than wide, its baseline inside it).

use super::super::super::draw::Faces;
use super::Fonts;

#[test]
fn a_stack_of_fonts_not_installed_falls_back_on_the_systems_monospace() {
    let Some(fonts) = Fonts::new("ui-monospace,SFMono-Regular,Menlo,monospace", 26.0, true) else {
        return;
    };
    let metrics = fonts.metrics();
    assert!(metrics.cell.0 > 4.0, "a cell's width: {:?}", metrics.cell);
    assert!(
        metrics.cell.1 > metrics.cell.0,
        "taller than wide: {:?}",
        metrics.cell
    );
    assert!(metrics.baseline > 0.0 && metrics.baseline < metrics.cell.1);
    assert_eq!(
        fonts.face(false, false).family().as_deref(),
        Some("monospace")
    );
}
