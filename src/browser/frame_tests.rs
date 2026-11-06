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
    // HUD in the dark appearance; under-window background, which reads light, in the light.
    assert_eq!(material(true), 13);
    assert_eq!(material(false), 21);
    let (dark_coat, dark_alpha) = coat(true);
    let (light_coat, light_alpha) = coat(false);
    assert_eq!((dark_coat, light_coat), (DARK, LIGHT));
    // Mostly glass, as Safari's bar: at most a quarter coat, lighter where the text is dark.
    assert!(dark_alpha < light_alpha && dark_alpha >= 0.4 && light_alpha <= 0.7);
    // A neutral coat, near-black or near-white, so the material never reads grey.
    let neutral = |[red, green, blue]: Rgb| red == green && green == blue;
    assert!(neutral(DARK) && DARK[0] < 0x10 && neutral(LIGHT) && LIGHT[0] > 0xf0);
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
    let glass = dress(page, Look::Glass);
    assert!(glass.contains("<html data-glass>"), "{glass}");
    assert!(
        glass.contains(&format!("<head><style>{CSS}</style>")),
        "{glass}"
    );
    let opaque = dress(page, Look::Opaque);
    assert!(opaque.contains("<html>") && opaque.contains(CSS));
    assert_eq!(opaque.matches("<style>").count(), 1);
    assert!(CSS.contains("prefers-reduced-transparency"));
}
