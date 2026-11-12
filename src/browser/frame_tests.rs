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
    assert!(light[2].1 <= 0.15 && dark[2].1 >= 0.7);
    // A neutral coat, near-black or near-white, so the material never reads grey.
    let neutral = |[red, green, blue]: Rgb| red == green && green == blue;
    assert!(neutral(DARK) && DARK[0] < 0x10 && neutral(LIGHT) && LIGHT[0] > 0xf0);
}

/// A colour's relative luminance, as WCAG 2 defines it.
fn luminance(colour: [f64; 3]) -> f64 {
    let linear = |part: f64| match part / 255.0 {
        part if part <= 0.040_45 => part / 12.92,
        part => ((part + 0.055) / 1.055).powf(2.4),
    };
    let [red, green, blue] = colour.map(linear);
    0.2126 * red + 0.7152 * green + 0.0722 * blue
}

/// `top`, `alpha` of it, over `under`.
fn over(top: [f64; 3], alpha: f64, under: [f64; 3]) -> [f64; 3] {
    [0, 1, 2].map(|part| top[part] * alpha + under[part] * (1.0 - alpha))
}

#[test]
fn the_dark_glass_keeps_its_words_readable_over_a_bright_desktop() {
    use super::super::settings::Transparency as Level;
    // The dark material over a bright desktop, as a window showed it: far lighter than the
    // coat, which is all that darkens it for the words.
    const BRIGHT: [f64; 3] = [150.0, 127.0, 147.0];
    // The dark appearance's ink, and how much of it the main and the quiet words are.
    const INK: [f64; 3] = [230.0, 230.0, 230.0];
    const MAIN: f64 = 0.9;
    const QUIET: f64 = 0.6;
    assert!(CSS.contains("--ink:color-mix(in srgb,var(--fg) 90%,transparent);"));
    assert!(CSS.contains("--fg:#e6e6e6;\n--ink-2:color-mix(in srgb,var(--fg) 60%,transparent);"));
    let reads = |level: Level, ink: f64| {
        let (colour, alpha) = coat(true, level);
        let ground = over(colour.map(f64::from), alpha, BRIGHT);
        let (words, ground) = (luminance(over(INK, ink, ground)), luminance(ground));
        (words + 0.05) / (ground + 0.05)
    };
    // WCAG 2 (1.4.3): 4.5 to 1 for text. The quiet words keep it at the level the app starts
    // with and under it; at the clearest the person may choose, the main words still do, and
    // the quiet ones keep the 3 to 1 of a control's least part (1.4.11).
    for level in [Level::Off, Level::Low, Level::Standard] {
        assert!(
            reads(level, QUIET) >= 4.5,
            "{level:?}: {}",
            reads(level, QUIET)
        );
    }
    assert!(
        reads(Level::High, MAIN) >= 4.5,
        "{}",
        reads(Level::High, MAIN)
    );
    assert!(
        reads(Level::High, QUIET) >= 3.0,
        "{}",
        reads(Level::High, QUIET)
    );
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
