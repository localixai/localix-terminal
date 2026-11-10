//! The frame both kinds of window are built in, the browser's and the panels': a native window
//! that never takes the keyboard, with its pages filling it. On macOS a shown window is glass:
//! the window is clear, and a native material behind its titlebar and pages samples the desktop.
//! A panel's page is clear too, over a coat of its ground, so the titlebar and the page are one
//! sheet; the browser's pages stay opaque, so what the agent sees in a screenshot never changes.
//! Elsewhere, and when the person reduces transparency as a window is built, the windows are
//! ordinary ones; one already glass turns solid while they do (`retint`).

use tao::dpi::{LogicalPosition, LogicalSize};
use tao::event::WindowEvent;
use tao::event_loop::EventLoopWindowTarget;
use tao::window::{Theme, Window, WindowBuilder};
use wry::WebView;

use super::host::Cmd;
use super::platform::{conceal, keep_rendering, keep_tab_rendering};

/// A colour as red, green and blue.
#[cfg(any(target_os = "macos", test))]
pub(super) type Rgb = [u8; 3];

/// The stylesheet a panel's page is dressed in: its ground, painted once, for each look.
pub(super) const CSS: &str = include_str!("frame.css");
/// The system's grounds, when a page has none of its own.
#[cfg(any(target_os = "macos", test))]
pub(super) const DARK: Rgb = [0x07, 0x07, 0x07];
#[cfg(any(target_os = "macos", test))]
pub(super) const LIGHT: Rgb = [0xf7, 0xf7, 0xf7];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Look {
    /// Clear, on a native material (macOS).
    Glass,
    Opaque,
    /// A headless window: invisible, and still rendering.
    Hidden,
}

/// Whether a window may be clear: the system has a material for it and the person has not
/// asked for less transparency.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Transparency {
    Allowed,
    Reduced,
}

/// How a window looks: hidden when headless, else glass where `transparency` allows it.
pub(super) fn look(headless: bool, transparency: Transparency) -> Look {
    let glass = transparency == Transparency::Allowed && cfg!(target_os = "macos");
    match (headless, glass) {
        (true, _) => Look::Hidden,
        (false, true) => Look::Glass,
        (false, false) => Look::Opaque,
    }
}

/// Whether the person has not asked for less transparency, on a system with glass.
pub(super) fn transparency() -> Transparency {
    #[cfg(target_os = "macos")]
    if !native::reduced() {
        return Transparency::Allowed;
    }
    Transparency::Reduced
}

/// Whether the person asked for less transparency, as the pages are told (`data-reduced`): the
/// pages' own `prefers-reduced-transparency` is not implemented by WebKit.
pub(super) fn reduced() -> bool {
    #[cfg(target_os = "macos")]
    return native::reduced();
    #[cfg(not(target_os = "macos"))]
    false
}

/// Whether the person runs a screen reader (VoiceOver), as a page is told (`reader` in its look).
pub(super) fn screen_reader() -> bool {
    #[cfg(target_os = "macos")]
    return native::screen_reader();
    #[cfg(not(target_os = "macos"))]
    false
}

/// A page's bounds: `size`, `x` points from the window's left edge and `top` from its top.
pub(super) fn fill(size: LogicalSize<f64>, x: f64, top: f64) -> wry::Rect {
    use wry::dpi::{LogicalPosition as At, LogicalSize as Size};
    wry::Rect {
        position: At::new(x, top).into(),
        size: Size::new(size.width, size.height).into(),
    }
}

/// Whether `event` changes the size the pages must fill.
pub(super) fn relayout(event: &WindowEvent) -> bool {
    matches!(
        event,
        WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. }
    )
}

/// The material for an appearance (NSVisualEffectMaterial): HUD, which is dark, in the dark
/// one; sidebar, as a Finder sidebar's, in the light one: it keeps the desktop's colour where the
/// milky under-window background washed it out.
#[cfg(any(target_os = "macos", test))]
pub(super) fn material(dark: bool) -> isize {
    if dark { 13 } else { 7 }
}

/// The coat over the glass in the dark appearance or the light at the person's transparency
/// `level`, and how much of it covers: neutral near-black or near-white (oklch 0.13 and 0.975),
/// never a page's colour. The material makes the text read, as the system means it to; the coat
/// only takes the desktop back for who wants less of it. White over a light blur hides the
/// desktop far sooner than black over a dark one, so the light ladder falls faster: its standard
/// is nearly the bare material.
#[cfg(any(target_os = "macos", test))]
pub(super) fn coat(dark: bool, level: super::settings::Transparency) -> (Rgb, f64) {
    use super::settings::Transparency as Level;
    let step = match level {
        Level::Off => 0,
        Level::Low => 1,
        Level::Standard => 2,
        Level::High => 3,
    };
    match dark {
        true => (DARK, [1.0, 0.7, 0.4, 0.1][step]),
        false => (LIGHT, [1.0, 0.35, 0.1, 0.0][step]),
    }
}

/// The person's chosen transparency, or none while the window must be `solid`: they reduce
/// transparency or increase contrast (a window's background is not to stay semi-transparent
/// then), or it is full screen, where no desktop lies behind it for the glass to sample (the
/// material would show a flat grey with the pages' ink over it).
#[cfg_attr(
    not(any(target_os = "macos", test)),
    expect(dead_code, reason = "the native glass is macOS's only")
)]
fn glass_level(
    chosen: super::settings::Transparency,
    solid: bool,
) -> super::settings::Transparency {
    if solid {
        super::settings::Transparency::Off
    } else {
        chosen
    }
}

/// The appearance a window is given: the person's, or none, which is the system's. Its pages
/// take it too (their `prefers-color-scheme`); nothing a page shows changes it.
pub(super) fn themed(theme: super::settings::Theme) -> Option<Theme> {
    use super::settings::Theme as Chosen;
    match theme {
        Chosen::Light => Some(Theme::Light),
        Chosen::Dark => Some(Theme::Dark),
        Chosen::System => None,
    }
}

/// `page` in the frame's stylesheet, marked as glass when it is and as reduced when the person
/// reduces transparency.
pub(super) fn dress(page: &str, look: Look) -> String {
    dressed_as(page, look, reduced())
}

/// `dress`, with whether transparency is reduced given.
fn dressed_as(page: &str, look: Look, reduced: bool) -> String {
    let page = page.replacen("<head>", &format!("<head><style>{CSS}</style>"), 1);
    let page = match look {
        Look::Glass => page.replacen("<html", "<html data-glass", 1),
        Look::Opaque | Look::Hidden => page,
    };
    match reduced {
        true => page.replacen("<html", "<html data-reduced", 1),
        false => page,
    }
}

/// A window as built here, and its glass.
pub(super) struct Frame {
    pub(super) native: Window,
    look: Look,
    #[cfg(target_os = "macos")]
    glass: Option<native::Glass>,
    /// How the window keeps drawing while it is concealed.
    #[cfg(target_os = "macos")]
    rendering: super::nspanel::Rendering,
    /// What the window remembers of full screen.
    #[cfg(target_os = "macos")]
    screen: std::sync::Mutex<native::Screen>,
}

impl Frame {
    /// A window titled `title` with pages of `size`, at `at` when given, in the person's theme.
    pub(super) fn build(
        target: &EventLoopWindowTarget<Cmd>,
        (title, size, at): (&str, LogicalSize<f64>, Option<LogicalPosition<f64>>),
        look: Look,
    ) -> Result<Self, String> {
        let theme = themed(super::settings::now().theme);
        let mut builder = WindowBuilder::new()
            .with_title(title)
            .with_inner_size(size)
            .with_focused(false)
            .with_theme(theme);
        if let Some(at) = at {
            builder = builder.with_position(at);
        }
        // A window the person sees is never made so small its bar and a page no longer fit.
        if look != Look::Hidden {
            builder = builder.with_min_inner_size(super::placement::LEAST);
        }
        // On macOS, order the window only after its Space and level are configured.
        #[cfg(target_os = "macos")]
        {
            builder = builder.with_visible(false);
        }
        builder = match look {
            // Shown only once concealed, so it never flashes on screen.
            Look::Hidden => conceal(builder.with_decorations(false).with_visible(false)),
            Look::Glass => native::clear(builder),
            Look::Opaque => builder,
        };
        let native = builder.build(target).map_err(|error| error.to_string())?;
        // A panel before any page is built in it: a WebView observes its window (KVO), and a
        // window whose class is swapped afterwards fails to remove the observer.
        #[cfg(target_os = "macos")]
        if look != Look::Hidden {
            super::nspanel::adopt(&native);
            native::toolbar(&native);
        }
        Ok(Self {
            native,
            look,
            #[cfg(target_os = "macos")]
            glass: None,
            #[cfg(target_os = "macos")]
            rendering: super::nspanel::Rendering::Switched,
            #[cfg(target_os = "macos")]
            screen: std::sync::Mutex::default(),
        })
    }

    /// Show the window with its first page, its glass, and, where it is shown to the person, as a
    /// panel (macOS) concealed: [`Frame::reveal`] opens it, when the person expands it.
    pub(super) fn show(&mut self, first: &WebView) {
        if self.look == Look::Hidden {
            keep_rendering(first);
            #[cfg(not(target_os = "macos"))]
            self.native.set_visible(true);
            return;
        }
        #[cfg(target_os = "macos")]
        {
            use super::nspanel::Rendering;
            self.rendering = match keep_tab_rendering(first) {
                true => Rendering::Switched,
                false => Rendering::Floating,
            };
            self.conceal();
            if self.look == Look::Glass {
                self.glass = native::Glass::under(&self.native);
                self.retint();
            }
        }
        #[cfg(not(target_os = "macos"))]
        let _ = keep_tab_rendering(first);
    }

    /// The window on screen, for the person to use (macOS: it comes over the Space they are in
    /// without taking the keyboard; elsewhere it is always shown).
    pub(super) fn reveal(&self) {
        // Elsewhere the window is an ordinary one, always as it is.
        #[cfg(not(target_os = "macos"))]
        let _ = &self.native;
        #[cfg(target_os = "macos")]
        {
            super::nspanel::reveal(&self.native, self.rendering);
            crate::lock(&self.screen).remember(&self.native);
        }
    }

    /// Full screen draws no toolbar over the strip (macOS), and leaving it gives the toolbar back.
    #[cfg_attr(
        not(target_os = "macos"),
        expect(
            clippy::unused_self,
            reason = "only macOS draws a toolbar to give back"
        )
    )]
    pub(super) fn follow_full_screen(&self) -> bool {
        #[cfg(target_os = "macos")]
        if self.look != Look::Hidden {
            let full = self.native.fullscreen().is_some();
            native::chrome(&self.native, full);
            // Leaving full screen, tao zooms the window it believed zoomed on the way in: the
            // frame the person had comes back instead of one that fills the screen.
            let was = crate::lock(&self.screen).step(&self.native, full);
            if was != full {
                self.retint();
                return !full;
            }
        }
        false
    }

    /// The window takes the person's keys: they just opened it themselves.
    pub(super) fn focus(&self) {
        #[cfg(not(target_os = "macos"))]
        let _ = &self.native;
        #[cfg(target_os = "macos")]
        super::nspanel::focus(&self.native);
    }

    /// Whether the window takes the keys now (it is the key window).
    pub(super) fn is_key(&self) -> bool {
        #[cfg(target_os = "macos")]
        return super::nspanel::is_key(&self.native);
        #[cfg(not(target_os = "macos"))]
        return self.native.is_focused();
    }

    /// The window out of sight, at the size it has, clear and click-through, still drawing.
    pub(super) fn conceal(&self) {
        // Elsewhere the window is an ordinary one, always as it is.
        #[cfg(not(target_os = "macos"))]
        let _ = &self.native;
        #[cfg(target_os = "macos")]
        super::nspanel::conceal(&self.native, self.rendering);
    }

    /// Tint the glass for the window's appearance now (the person's theme, or the system's).
    /// Solid, as the person's Off is, while they reduce transparency or increase contrast, and in
    /// full screen (`glass_level`).
    #[cfg_attr(
        not(target_os = "macos"),
        expect(clippy::unused_self, reason = "macOS only")
    )]
    pub(super) fn retint(&self) {
        #[cfg(target_os = "macos")]
        if let Some(glass) = &self.glass {
            let dark = self.native.theme() == Theme::Dark;
            let chosen = super::settings::now().transparency;
            let solid =
                native::reduced() || native::increased() || self.native.fullscreen().is_some();
            let (colour, alpha) = coat(dark, glass_level(chosen, solid));
            glass.tint(material(dark), colour, alpha);
        }
    }

    /// The window and its pages in the person's `theme`, the glass tinted again, its
    /// transparency with it.
    pub(super) fn restyle(&self, theme: super::settings::Theme) {
        self.native.set_theme(themed(theme));
        self.retint();
    }
}

/// For checks: the materials behind `window`, the first one's state, whether the window is
/// opaque, the shape its resizing keeps (width over height; 0 when none), and whether it is
/// coated.
pub(super) fn dressed(window: &Window) -> (usize, isize, bool, f64, bool) {
    #[cfg(target_os = "macos")]
    return native::dressed(window);
    #[cfg(not(target_os = "macos"))]
    {
        let _ = window;
        (0, -1, true, 0.0, false)
    }
}

#[cfg(target_os = "macos")]
#[path = "frame_macos.rs"]
mod native;
#[cfg(not(target_os = "macos"))]
mod native {
    pub(super) fn clear(builder: tao::window::WindowBuilder) -> tao::window::WindowBuilder {
        builder
    }
}

#[cfg(test)]
#[path = "frame_tests.rs"]
mod tests;
