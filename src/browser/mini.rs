//! The mini (only macOS builds one, where `presence` has it): a small floating panel in a screen corner over every
//! Space, with a live picture of the tab shown, its project and id, a spinner while the agent
//! acts, and a highlight when it shows a tab. Its page (mini.html) is data in and one word out,
//! `expand` (its button); a press anywhere else is the grip's (mini_grip.rs on macOS: a drag, or
//! a click that expands). It never takes the keyboard or navigates.

use std::time::Duration;

use base64::Engine;
use serde_json::json;
use tao::dpi::{LogicalPosition, LogicalSize};
use tao::event_loop::{EventLoopProxy, EventLoopWindowTarget};
use tao::window::{Window, WindowBuilder, WindowId};
use wry::{WebView, WebViewBuilder};

use super::capture::Shot;
use super::frame;
use super::host::Cmd;
use super::placement::MINI;
use super::platform::{adopt_mini, grip_mini, hide_mini, keep_tab_rendering, show_mini};
use super::tab_view::{build, own_page};

/// The mini's page.
const PAGE: &str = include_str!("mini.html");

pub(super) struct Mini {
    window: Window,
    webview: WebView,
}

/// What the mini's page said, as a command.
fn heard(said: &str) -> Option<Cmd> {
    (said == "expand").then_some(Cmd::Opened)
}

impl Mini {
    /// The person's settings: its theme and its page's fonts.
    pub(super) fn restyle(&self, settings: &super::settings::Settings) {
        self.window.set_theme(frame::themed(settings.theme));
        let _ = self
            .webview
            .evaluate_script(&super::settings::look_script(settings));
    }

    /// The mini at `at` (its top left), out of sight until [`Mini::show`].
    pub(super) fn new(
        target: &EventLoopWindowTarget<Cmd>,
        proxy: &EventLoopProxy<Cmd>,
        at: (f64, f64),
    ) -> Result<Self, String> {
        let window = WindowBuilder::new()
            .with_title("llx-desk")
            .with_decorations(false)
            .with_transparent(true)
            .with_resizable(false)
            .with_focused(false)
            .with_visible(false)
            .with_inner_size(LogicalSize::new(MINI.0, MINI.1))
            .with_position(LogicalPosition::new(at.0, at.1))
            .build(target)
            .map_err(|error| error.to_string())?;
        // A panel before its page is built (see `Frame::build`).
        adopt_mini(&window);
        let said = proxy.clone();
        let builder = own_page(WebViewBuilder::new())
            .with_html(PAGE)
            // The person's fonts, before the page's own scripts (settings.rs).
            .with_initialization_script(super::settings::look_script(&super::settings::now()))
            .with_transparent(true)
            .with_bounds(frame::fill(LogicalSize::new(MINI.0, MINI.1), 0.0, 0.0))
            .with_ipc_handler(move |request| {
                if let Some(command) = heard(request.body()) {
                    let _ = said.send_event(command);
                }
            });
        let webview = build(builder, &window).map_err(|error| error.to_string())?;
        keep_tab_rendering(&webview);
        grip_mini(&window, proxy);
        Ok(Self { window, webview })
    }

    pub(super) fn id(&self) -> WindowId {
        self.window.id()
    }

    pub(super) fn show(&self) {
        show_mini(&self.window);
    }

    pub(super) fn hide(&self) {
        hide_mini(&self.window);
    }

    pub(super) fn page(&self) -> &WebView {
        &self.webview
    }

    /// Where it is now, its top left in points.
    pub(super) fn at(&self) -> Option<(f64, f64)> {
        let at = self.window.outer_position().ok()?;
        let at = at.to_logical::<f64>(self.window.scale_factor());
        Some((at.x, at.y))
    }

    /// Show `shot`, the tab's picture.
    pub(super) fn picture(&self, shot: &Shot) {
        let data = base64::engine::general_purpose::STANDARD.encode(&shot.jpeg);
        self.call(&format!("P(\"data:image/jpeg;base64,{data}\")"));
    }

    /// Name the tab shown: its project and its id.
    pub(super) fn tab(&self, id: &str, group: &str) {
        self.call(&format!("T({})", json!({"id": id, "group": group})));
    }

    pub(super) fn busy(&self, on: bool) {
        self.call(&format!("B({on})"));
    }

    pub(super) fn blink(&self, long: Duration) {
        self.call(&format!("K({})", long.as_millis()));
    }

    fn call(&self, script: &str) {
        let _ = self.webview.evaluate_script(script);
    }
}

#[cfg(test)]
#[path = "mini_tests.rs"]
mod tests;
