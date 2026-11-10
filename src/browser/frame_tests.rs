//! Tests of `frame`.

use tao::dpi::{LogicalSize, PhysicalPosition, PhysicalSize};
use tao::event::WindowEvent;

use super::*;

fn rect(rect: wry::Rect) -> (f64, f64, f64, f64) {
    let at = rect.position.to_logical::<f64>(1.0);
    let size = rect.size.to_logical::<f64>(1.0);
    (at.x, at.y, size.width, size.height)
}

#[test]
fn a_page_fills_its_window_wherever_it_stands() {
    let size = LogicalSize::new(1200.0, 800.0);
    assert_eq!(rect(fill(size, 0.0, 0.0)), (0.0, 0.0, 1200.0, 800.0));
    // A tab out of sight keeps the window's size.
    assert_eq!(
        rect(fill(size, -1300.0, 34.0)),
        (-1300.0, 34.0, 1200.0, 800.0)
    );
}

#[test]
fn a_new_size_or_scale_lays_the_pages_out_again() {
    assert!(relayout(&WindowEvent::Resized(PhysicalSize::new(800, 600))));
    let mut size = PhysicalSize::new(1600, 1200);
    let scaled = WindowEvent::ScaleFactorChanged {
        scale_factor: 2.0,
        new_inner_size: &mut size,
    };
    assert!(relayout(&scaled));
    assert!(!relayout(&WindowEvent::Moved(PhysicalPosition::new(1, 2))));
    assert!(!relayout(&WindowEvent::Focused(true)));
}

#[test]
fn glass_is_for_a_shown_window_on_macos() {
    assert_eq!(look(true, Transparency::Allowed), Look::Hidden);
    assert_eq!(look(true, Transparency::Reduced), Look::Hidden);
    let shown = if cfg!(target_os = "macos") {
        Look::Glass
    } else {
        Look::Opaque
    };
    assert_eq!(look(false, Transparency::Allowed), shown);
    // Reduced transparency: an ordinary window.
    assert_eq!(look(false, Transparency::Reduced), Look::Opaque);
}

#[test]
fn each_appearance_has_its_own_material_and_coat() {
    // HUD in the dark appearance; sidebar, which keeps the desktop's colour, in the light.
    assert_eq!(material(true), 13);
    assert_eq!(material(false), 7);
    use super::super::settings::Transparency as Level;
    let levels = [Level::Off, Level::Low, Level::Standard, Level::High];
    let ladder = |dark| levels.map(|level| coat(dark, level));
    let (dark, light) = (ladder(true), ladder(false));
    assert!(dark.iter().all(|(colour, _)| *colour == DARK));
    assert!(light.iter().all(|(colour, _)| *colour == LIGHT));
    for steps in [dark, light] {
        // Off is opaque, and each level clearer than the one before.
        assert!((steps[0].1 - 1.0).abs() < f64::EPSILON);
        assert!(
            steps.windows(2).all(|pair| pair[1].1 < pair[0].1),
            "{steps:?}"
        );
    }
    // White hides a light blur sooner than black a dark one: never more of it at a level, and
    // the light standard is nearly the bare material.
    assert!(
        dark.iter()
            .zip(light)
            .all(|(dark, light)| light.1 <= dark.1)
    );
    assert!(light[2].1 <= 0.15 && (0.3..=0.5).contains(&dark[2].1));
    // A neutral coat, near-black or near-white, so the material never reads grey.
    let neutral = |[red, green, blue]: Rgb| red == green && green == blue;
    assert!(neutral(DARK) && DARK[0] < 0x10 && neutral(LIGHT) && LIGHT[0] > 0xf0);
}

#[test]
fn the_glass_keeps_the_chosen_level_in_every_window_state() {
    use super::super::settings::Transparency as Level;
    for chosen in [Level::Off, Level::Low, Level::Standard, Level::High] {
        assert_eq!(glass_level(chosen, false), chosen);
        assert_eq!(glass_level(chosen, true), Level::Off);
    }
}

#[test]
fn the_theme_is_the_persons_else_the_systems() {
    use super::super::settings::Theme as Chosen;
    assert_eq!(themed(Chosen::Light), Some(Theme::Light));
    assert_eq!(themed(Chosen::Dark), Some(Theme::Dark));
    assert_eq!(themed(Chosen::System), None);
}

#[test]
fn a_page_is_dressed_in_the_frames_stylesheet() {
    let page = "<!doctype html>\n<html>\n<head>\n<title>t</title></head><body></body></html>";
    // Transparency as the person left it on: the machine's own setting is the next test's.
    let glass = dressed_as(page, Look::Glass, false);
    assert!(glass.contains("<html data-glass>"), "{glass}");
    assert!(
        glass.contains(&format!("<head><style>{CSS}</style>")),
        "{glass}"
    );
    let opaque = dressed_as(page, Look::Opaque, false);
    assert!(opaque.contains("<html>") && opaque.contains(CSS));
    assert_eq!(opaque.matches("<style>").count(), 1);
    assert!(CSS.contains("prefers-reduced-transparency"));
}

#[test]
fn a_page_is_told_when_the_person_reduces_transparency() {
    let page = "<!doctype html>\n<html>\n<head>\n<title>t</title></head><body></body></html>";
    assert!(dressed_as(page, Look::Glass, true).contains("<html data-reduced data-glass>"));
    assert!(dressed_as(page, Look::Opaque, true).contains("<html data-reduced>"));
    assert!(!dressed_as(page, Look::Glass, false).contains("<html data-reduced"));
    assert!(CSS.contains("html[data-reduced]"));
}
