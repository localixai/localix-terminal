//! The terminal's look against the window's token sheet: the text colour is `--fg` of
//! `frame.css` in light and dark, and contrast, selection, matches and the scrollbar take the
//! shares `theme.rs` names.

use super::{FG_DARK, FG_LIGHT, Paint, Theme, luminance, ratio};
use crate::surface::Rgb;

/// `--fg` as the token sheet sets it: first at the root, then for a dark scheme.
fn sheet_fg() -> (Rgb, Rgb) {
    let sheet = include_str!("../frame.css");
    let mut found = sheet.match_indices("--fg:#").map(|(at, token)| {
        let hex = sheet.get(at + token.len()..at + token.len() + 6).unwrap();
        Rgb::hex(u32::from_str_radix(hex, 16).unwrap())
    });
    (found.next().unwrap(), found.next().unwrap())
}

#[test]
fn the_text_colour_is_the_token_sheets_in_light_and_dark() {
    assert_eq!(sheet_fg(), (FG_LIGHT, FG_DARK));
    assert_eq!(Theme::new(false).palette().foreground, FG_LIGHT);
    assert_eq!(Theme::new(true).palette().foreground, FG_DARK);
}

#[test]
fn the_palette_is_the_dark_or_the_light_one() {
    let dark = Theme::new(true).palette();
    let light = Theme::new(false).palette();
    assert_eq!(dark.ansi[1], Rgb::hex(0xff7b72));
    assert_eq!(light.ansi[1], Rgb::hex(0xd73a49));
    assert_eq!(
        (dark.background, light.background),
        (Rgb::hex(0x1e1e1e), Rgb::hex(0xffffff))
    );
    assert_eq!(dark.cursor, FG_DARK);
}

#[test]
fn text_too_faint_for_its_ground_is_made_to_read() {
    let ground = Rgb::hex(0x1e1e1e);
    let dim = Rgb::hex(0x303030);
    let theme = Theme::new(true);
    let made = theme.readable(dim, ground);
    assert!(ratio(luminance(ground), luminance(made)) >= 4.5, "{made:?}");
    let light_ground = Rgb::hex(0xffffff);
    let pale = theme.readable(Rgb::hex(0xeeeeee), light_ground);
    assert!(
        ratio(luminance(light_ground), luminance(pale)) >= 4.5,
        "{pale:?}"
    );
    let fine = Rgb::hex(0xe6e6e6);
    assert_eq!(
        theme.readable(fine, ground),
        fine,
        "what reads stays as it is"
    );
}

#[test]
fn selection_matches_and_scrollbar_take_the_pages_shares() {
    let mut theme = Theme::new(true);
    assert_eq!(theme.selection(), Paint::share(theme.accent, 0.4));
    assert_eq!(theme.selection().alpha, 102);
    theme.active = false;
    assert_eq!(theme.selection(), Paint::share(FG_DARK, 0.18));
    assert_eq!(theme.scrollbar(false).alpha, 0x66);
    assert_eq!(theme.scrollbar(true).alpha, 0xe6);
    assert!(theme.found(true).rgb != theme.found(false).rgb);
}

#[test]
fn more_contrast_holds_text_to_seven_to_one() {
    let ground = Rgb::hex(0x1e1e1e);
    let grey = Rgb::hex(0x8b949e);
    let theme = Theme::new(true);
    assert_eq!(theme.readable(grey, ground), grey, "it reads at 4.5");
    let more = Theme {
        more: true,
        ..theme
    };
    let made = more.readable(grey, ground);
    assert!(ratio(luminance(ground), luminance(made)) >= 7.0, "{made:?}");
}

#[test]
fn matches_take_the_systems_grey_in_a_window_not_in_use() {
    let theme = Theme::new(true);
    let away = Theme {
        key: false,
        ..theme
    };
    assert_eq!(away.accent(), super::GREY);
    assert_ne!(away.found(true), theme.found(true));
    assert_eq!(
        away.found(false).rgb,
        Theme {
            accent: super::GREY,
            ..theme
        }
        .found(false)
        .rgb
    );
}
