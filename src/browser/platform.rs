//! What differs by platform: where a headless window goes, keeping it rendering, the Dock, the user agent.

#[path = "platform_views.rs"]
mod views;

pub(super) use views::{Follow, follow};
pub(super) use views::{
    Pane, clear, keep_rendering, keep_tab_rendering, reparent, see_through, stack, to_front,
};

use tao::event_loop::EventLoopWindowTarget;
use tao::window::WindowBuilder;

/// With `DESK_WINDOWS=unseen` a shown window is everything it is, strip,
/// glass and panels, but on screen it is what a headless one is: clear, passing clicks through,
/// with no Dock icon, so a test run never covers what the person is watching (macOS). Its pages
/// remember nothing, anywhere (tab_view.rs).
pub(super) fn unseen() -> bool {
    std::env::var_os("DESK_WINDOWS").is_some_and(|value| value == "unseen")
}

/// The app's place in the system: a regular app, with its Dock icon and its menu bar, while it
/// runs as the app (inside its bundle) or shows a window. Otherwise (a program run from a
/// terminal with no window yet: a test, an app with nothing to show) it is a background app
/// macOS never activates: one started from the terminal the person types in would take their
/// keys at launch. Nothing llx-desk does activates it but the person's own acts (appkit.rs).
#[cfg(target_os = "macos")]
pub(super) fn dock(
    target: &EventLoopWindowTarget<super::host::Cmd>,
    shown: bool,
    proxy: &tao::event_loop::EventLoopProxy<super::host::Cmd>,
) {
    use tao::platform::macos::{ActivationPolicy, EventLoopWindowTargetExtMacOS};
    let regular = (shown || crate::bundle::app().is_some()) && !unseen();
    let policy = match regular {
        true => ActivationPolicy::Regular,
        false => ActivationPolicy::Prohibited,
    };
    target.set_activation_policy_at_runtime(policy);
    if regular {
        super::app_menu::install(proxy);
    }
}

#[cfg(not(target_os = "macos"))]
pub(super) use super::menu_gtk::{confirm, page_shown, quit_through};
/// Quitting from outside the app (the Dock, logging out) comes to the desk as its own Quit does
/// (macOS; elsewhere the window's close is the way out). A question to the person is asked as
/// their system asks it.
#[cfg(target_os = "macos")]
pub(super) use super::{
    app_menu::{page_shown, quit_through},
    appkit::confirm,
};

#[cfg(not(target_os = "macos"))]
pub(super) fn dock(
    _target: &EventLoopWindowTarget<super::host::Cmd>,
    _shown: bool,
    _proxy: &tao::event_loop::EventLoopProxy<super::host::Cmd>,
) {
}

/// The primary screen's room for windows: below the menu bar and above the Dock on macOS, else
/// the whole monitor. `None` when there is no monitor.
pub(super) fn work_area<T>(target: &EventLoopWindowTarget<T>) -> Option<super::placement::Area> {
    #[cfg(target_os = "macos")]
    if let Some(area) = super::nspanel::work_area() {
        return Some(area);
    }
    let monitor = target.primary_monitor()?;
    let scale = monitor.scale_factor();
    let (size, at) = (
        monitor.size().to_logical::<f64>(scale),
        monitor.position().to_logical::<f64>(scale),
    );
    Some(super::placement::Area {
        x: at.x,
        y: at.y,
        width: size.width,
        height: size.height,
    })
}

/// The mini's window as a floating panel on every Space, never taking the keyboard.
#[cfg(target_os = "macos")]
pub(super) fn adopt_mini(window: &tao::window::Window) {
    super::nspanel::adopt(window, super::nspanel::Role::Mini);
}

/// The deck's minimize button collapses it to the mini.
#[cfg(target_os = "macos")]
pub(super) fn collapse_button(
    window: &tao::window::Window,
    proxy: &tao::event_loop::EventLoopProxy<super::host::Cmd>,
) {
    super::nspanel::collapse_to(window, proxy);
}

/// Elsewhere there is no mini: the button minimizes as the system does.
#[cfg(not(target_os = "macos"))]
pub(super) fn collapse_button(
    _window: &tao::window::Window,
    _proxy: &tao::event_loop::EventLoopProxy<super::host::Cmd>,
) {
}

/// The mini's own grip over its page: a press moves it, a click expands it.
#[cfg(target_os = "macos")]
pub(super) fn grip_mini(
    window: &tao::window::Window,
    proxy: &tao::event_loop::EventLoopProxy<super::host::Cmd>,
) {
    super::mini_grip::grip(window, proxy);
}

#[cfg(target_os = "macos")]
pub(super) fn show_mini(window: &tao::window::Window) {
    super::nspanel::reveal(
        window,
        super::nspanel::Role::Mini,
        super::nspanel::Rendering::Switched,
    );
}

#[cfg(target_os = "macos")]
pub(super) fn hide_mini(window: &tao::window::Window) {
    super::nspanel::hide(window);
}

/// The person's moves between Spaces, told to the loop: the mini is put on every Space again.
#[cfg(target_os = "macos")]
pub(super) fn watch_spaces(proxy: &tao::event_loop::EventLoopProxy<super::host::Cmd>) {
    super::nspanel::watch_spaces(proxy);
}

/// The person's changes to Accessibility, Display (reduce transparency among them), told to the
/// loop: the windows are dressed again.
#[cfg(target_os = "macos")]
pub(super) fn watch_display(proxy: &tao::event_loop::EventLoopProxy<super::host::Cmd>) {
    super::nspanel::watch_display(proxy);
}

/// Elsewhere there is no setting to watch.
#[cfg(not(target_os = "macos"))]
pub(super) fn watch_display(_proxy: &tao::event_loop::EventLoopProxy<super::host::Cmd>) {}

/// The person's presses in the app's windows, told to the loop: a split window's half pressed
/// is the one in use.
#[cfg(target_os = "macos")]
pub(super) fn watch_presses(proxy: &tao::event_loop::EventLoopProxy<super::host::Cmd>) {
    super::presses::watch(proxy);
}

/// Elsewhere the window is not split.
#[cfg(not(target_os = "macos"))]
pub(super) fn watch_presses(_proxy: &tao::event_loop::EventLoopProxy<super::host::Cmd>) {}

/// Elsewhere there are no Spaces to watch.
#[cfg(not(target_os = "macos"))]
pub(super) fn watch_spaces(_proxy: &tao::event_loop::EventLoopProxy<super::host::Cmd>) {}

#[cfg(not(target_os = "macos"))]
pub(super) fn adopt_mini(_window: &tao::window::Window) {}

#[cfg(not(target_os = "macos"))]
pub(super) fn grip_mini(
    _window: &tao::window::Window,
    _proxy: &tao::event_loop::EventLoopProxy<super::host::Cmd>,
) {
}

#[cfg(not(target_os = "macos"))]
pub(super) fn show_mini(window: &tao::window::Window) {
    window.set_visible(true);
}

#[cfg(not(target_os = "macos"))]
pub(super) fn hide_mini(window: &tao::window::Window) {
    window.set_visible(false);
}

/// Where a headless window goes. macOS keeps it on screen and makes it invisible afterwards
/// ([`keep_rendering`]): WebKit marks an off-screen window hidden and stops its animation frames
/// and timers, so a page there would never settle. Elsewhere it goes off screen.
#[cfg(target_os = "macos")]
pub(super) fn conceal(builder: WindowBuilder) -> WindowBuilder {
    builder
}

#[cfg(not(target_os = "macos"))]
pub(super) fn conceal(builder: WindowBuilder) -> WindowBuilder {
    let size = super::viewport::Viewport::DESKTOP;
    builder.with_position(tao::dpi::LogicalPosition::new(
        -size.width * 4.0,
        -size.height * 4.0,
    ))
}

/// The person picks one of `items` from the system's menu at `at` of `view`, the one at `marked`
/// checked: the index picked.
pub(super) fn choose(
    view: &wry::WebView,
    at: (f64, f64),
    items: &[&str],
    marked: Option<usize>,
) -> Option<usize> {
    show(view, at, &super::choice::items(items, marked))
}

/// The person picks from `entries` in the system's menu at `at` of `view` (menu.rs): the place of
/// the item picked among them (`choice::flat`).
#[cfg(target_os = "macos")]
pub(super) fn show(
    view: &wry::WebView,
    at: (f64, f64),
    entries: &[super::choice::Entry],
) -> Option<usize> {
    super::menu::show(view, at, entries)
}

/// Elsewhere GTK's menu, at the pointer (the click that asked for it is under it).
#[cfg(not(target_os = "macos"))]
pub(super) fn show(
    view: &wry::WebView,
    _at: (f64, f64),
    entries: &[super::choice::Entry],
) -> Option<usize> {
    super::menu_gtk::show(view, entries)
}

/// The pasteboard's text (pasteboard.rs); elsewhere none yet.
#[cfg(target_os = "macos")]
pub(super) fn pasted() -> Option<String> {
    super::pasteboard::pasted()
}

#[cfg(not(target_os = "macos"))]
pub(super) fn pasted() -> Option<String> {
    None
}

/// `address` opened by the person's own app for it (their default browser); whether it was.
#[cfg(target_os = "macos")]
pub(super) use super::appkit::{open_outside, pick_folder, reveal};

/// A folder the person picks, as GTK asks; none when they cancel.
#[cfg(not(target_os = "macos"))]
pub(super) fn pick_folder() -> Option<std::path::PathBuf> {
    use gtk::prelude::{DialogExt, FileChooserExt, GtkWindowExt, WidgetExt};
    let none: Option<&gtk::Window> = None;
    let dialog = gtk::FileChooserDialog::with_buttons(
        Some("Open Folder"),
        none,
        gtk::FileChooserAction::SelectFolder,
        &[
            ("Cancel", gtk::ResponseType::Cancel),
            ("Open", gtk::ResponseType::Accept),
        ],
    );
    dialog.set_modal(true);
    let chose = dialog.run() == gtk::ResponseType::Accept;
    let folder = chose.then(|| dialog.filename()).flatten();
    dialog.close();
    folder
}

/// `address` opened by the person's own app for it (their default browser); whether it was.
#[cfg(not(target_os = "macos"))]
pub(super) fn open_outside(address: &str) -> bool {
    let none: Option<&gtk::gio::AppLaunchContext> = None;
    gtk::gio::AppInfo::launch_default_for_uri(address, none).is_ok()
}

/// What showing a folder in the system's file manager is called.
pub(super) const REVEAL: &str = if cfg!(target_os = "macos") {
    "Show in Finder"
} else {
    "Show in Files"
};

/// The folder at `path` shown in the person's file manager; whether it was.
#[cfg(not(target_os = "macos"))]
pub(super) fn reveal(path: &std::path::Path) -> bool {
    url::Url::from_file_path(path).is_ok_and(|folder| open_outside(folder.as_str()))
}

/// Put `text` on the pasteboard.
#[cfg(target_os = "macos")]
pub(super) fn copy(text: &str) {
    super::pasteboard::copy(text);
}

/// Put `text` on the clipboard (on the window's thread, where GTK runs).
#[cfg(not(target_os = "macos"))]
pub(super) fn copy(text: &str) {
    gtk::Clipboard::get(&gtk::gdk::SELECTION_CLIPBOARD).set_text(text);
}

/// The installed Safari's user agent. A plain WKWebView omits the `Version/… Safari/…` tokens,
/// and sites such as Google then serve a legacy page.
#[cfg(target_os = "macos")]
#[expect(
    clippy::unnecessary_wraps,
    reason = "one signature on every system: elsewhere there is none"
)]
pub(super) fn user_agent() -> Option<String> {
    /// Read once: Safari's version does not change in a run, and every tab asks.
    static AGENT: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    let agent = AGENT.get_or_init(|| {
        let plist = std::fs::read_to_string("/Applications/Safari.app/Contents/Info.plist").ok();
        let version = plist
            .as_deref()
            .and_then(|plist| plist.split("<key>CFBundleShortVersionString</key>").nth(1))
            .and_then(|rest| rest.split("<string>").nth(1))
            .and_then(|rest| rest.split("</string>").next())
            .filter(|version| !version.is_empty() && version.len() < 16)
            .unwrap_or("26.0");
        format!(
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 \
             (KHTML, like Gecko) Version/{version} Safari/605.1.15"
        )
    });
    Some(agent.clone())
}

#[cfg(not(target_os = "macos"))]
pub(super) fn user_agent() -> Option<String> {
    None
}
