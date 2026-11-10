//! The browser's own commands on its tabs in the window: a script, a load, a picture, a size,
//! and a tab opened, switched to or closed.

use std::sync::Arc;

use tao::event_loop::EventLoopProxy;

use super::super::capture::{Clip, capture};
use super::super::deck::{Deck, Shown};
use super::super::strip::Key;
use super::{Cmd, Shared, WINDOW_CLOSED};

/// The browser is closed: a script or a load waiting on it fails now.
pub(super) fn closed(command: &Cmd, shared: &Shared) {
    if let Cmd::Eval { id, .. } = command {
        shared.answer(*id, Err(WINDOW_CLOSED.into()));
    } else if let Cmd::Navigate { reply, .. } = command {
        let _ = reply.send(Err(WINDOW_CLOSED.into()));
    }
}

/// `url` loaded in `webview`, tab `tab`'s (the one acted on with none); the caller told at once
/// when the tab is gone, so it never waits for a load that cannot start.
pub(super) fn load(
    webview: Option<&wry::WebView>,
    url: &str,
    tab: Option<u32>,
    reply: &std::sync::mpsc::SyncSender<Result<(), String>>,
) {
    // An address the system cannot read never reaches WebKit: its load would panic here.
    let loaded = match (webview, crate::browser::guard::loadable(url)) {
        (None, _) => Err(match tab {
            Some(id) => format!("tab {id} is closed: list shows the tabs open"),
            None => WINDOW_CLOSED.into(),
        }),
        (Some(_), None) => Err(format!("`{url}` is not an address")),
        (Some(webview), Some(url)) => webview.load_url(&url).map_err(|why| why.to_string()),
    };
    let _ = reply.send(loaded);
}

/// `script` run in tab `tab`'s `webview`, its call `id` answered by that page alone; a tab closed
/// answers at once.
fn eval(
    (tab, webview): (u32, Option<&wry::WebView>),
    id: u64,
    script: &str,
    failed: &'static str,
    shared: &Arc<Shared>,
) {
    let Some(webview) = webview else {
        return shared.answer(id, Err(format!("{failed}: the tab is closed")));
    };
    shared.calls.aimed(id, tab);
    let answer = Arc::clone(shared);
    let sent = webview.evaluate_script_with_callback(script, move |started| {
        if started.trim() != "true" {
            answer.answer(id, Err(failed.into()));
        }
    });
    if let Err(error) = sent {
        shared.answer(id, Err(format!("{failed}: {error}")));
    }
}

/// Tab `tab` (the one acted on with none) takes `frame`, and the window and the strip show it.
fn reframe(
    open: &mut Deck,
    tab: Option<u32>,
    frame: Option<crate::browser::sizes::Frame>,
    shared: &Shared,
) {
    if let Some(tabs) = open.web.as_mut() {
        tabs.set_frame(tab, frame);
    }
    open.layout();
    open.refresh(shared);
}

/// `clip`, in CSS pixels, in the points of a page drawn at `zoom` (a size drawn smaller to fit).
fn zoomed(clip: Option<Clip>, zoom: f64) -> Option<Clip> {
    clip.map(|clip| Clip {
        x: clip.x * zoom,
        y: clip.y * zoom,
        width: clip.width * zoom,
        height: clip.height * zoom,
    })
}

/// A command of the browser's own, on its tabs in the window.
pub(super) fn on_browser(
    command: Cmd,
    deck: &mut Option<Deck>,
    shared: &Arc<Shared>,
    proxy: &EventLoopProxy<Cmd>,
) {
    // A driver exists only once the browser opened: with none now, it was closed.
    let Some(open) = deck.as_mut() else {
        return closed(&command, shared);
    };
    // A comment is the person's only while they comment in this window: no page can say they did.
    if matches!(command, Cmd::Noted(_)) && !open.is_commenting() {
        return;
    }
    let Some(tabs) = open.web.as_mut() else {
        return closed(&command, shared);
    };
    match command {
        Cmd::Eval {
            id,
            script,
            failed,
            tab,
            ..
        } => {
            let to = tab.unwrap_or_else(|| tabs.active_id());
            return eval((to, tabs.at(tab)), id, &script, failed, shared);
        }
        Cmd::Frame(tab, frame) => return reframe(open, tab, frame, shared),
        Cmd::Capture { clip, reply, tab } => match tabs.at(tab) {
            Some(webview) => return capture(webview, zoomed(clip, tabs.zoom(tab)), reply),
            None => {
                let _ = reply.send(Err("the tab is closed".into()));
                return;
            }
        },
        // The comment is of the tab shown in the window it was made in (this one: it is routed to
        // the window in use), not of whichever tab the app acted on last.
        Cmd::Noted(mut noted) => {
            noted.tab = tabs.active_id();
            let (reply, shot) = std::sync::mpsc::sync_channel(1);
            if let (Some(_), Some(webview)) = (noted.clip, tabs.at(Some(noted.tab))) {
                let clip = zoomed(noted.clip, tabs.zoom(Some(noted.tab)));
                capture(webview, clip, reply);
            }
            return crate::browser::comments::pictured(noted, shot, shared, proxy);
        }
        // What the agent does in the browser is shown: the window goes back to it, unless it
        // works in a tab of its own beside the one shown.
        Cmd::Navigate { url, tab, reply } => {
            load(tabs.at(tab), &url, tab, &reply);
            if tab.is_none() {
                open.show(Shown::Web);
            }
        }
        Cmd::Adopt => {
            if tabs.adopt(shared).is_some_and(|(_, shown)| shown) {
                open.show(Shown::Web);
            }
        }
        Cmd::NewTab { url, reply, opener } => new_tab(open, &url, reply, opener, shared, proxy),
        Cmd::SwitchTab { id, reply } => {
            let _ = reply.send(tabs.switch(id, shared));
            open.show(Shown::Web);
        }
        Cmd::CloseTab { id, reply } => {
            let closed = tabs.close(id, shared);
            let emptied = closed == Ok(false);
            let _ = reply.send(closed);
            if emptied {
                open.end_web(&[id], shared);
            }
        }
        Cmd::Open { .. }
        | Cmd::Close
        | Cmd::Panel(_)
        | Cmd::Paint(_)
        | Cmd::Unpanel(_)
        | Cmd::PanelMenu(..)
        | Cmd::Reveal(_)
        | Cmd::Titled(..)
        | Cmd::Clicked(_)
        | Cmd::SideClicked(_)
        | Cmd::NavClicked(_)
        | Cmd::Restyle
        | Cmd::Strip
        | Cmd::Offered(..)
        | Cmd::Go(_)
        | Cmd::Arranged(..)
        | Cmd::NewWindow(_)
        | Cmd::Dressed(_)
        | Cmd::Listing(_)
        | Cmd::Act(..)
        | Cmd::Presence(_)
        | Cmd::Pressed(..)
        | Cmd::Opened
        | Cmd::Presented(_)
        | Cmd::Confirm(..)
        | Cmd::Exit(_) => return,
    }
    open.refresh(shared);
}

/// A tab opened on `url` and shown: the agent's (`reply`), or a page's own new window, which past
/// the tab limit opens where the page is, as before tabs.
fn new_tab(
    open: &mut Deck,
    url: &str,
    reply: Option<std::sync::mpsc::SyncSender<Result<u32, String>>>,
    opener: Option<u32>,
    shared: &Arc<Shared>,
    proxy: &EventLoopProxy<Cmd>,
) {
    let Some(tabs) = open.web.as_mut() else {
        return;
    };
    // The project of the tab the app opens (one not shown opens out of sight there); a page's own
    // new window is of its opener's, and never takes the slot a call of the app set.
    let project = match opener {
        Some(opener) => crate::lock(&shared.groups)
            .get(&Key::Web(opener).name())
            .cloned(),
        None => crate::lock(&shared.scope).take_opening(),
    };
    let shown = (project.as_deref()).is_none_or(|project| {
        let mut scope = crate::lock(&shared.scope);
        scope.touched(project);
        scope.in_sight(project)
    });
    let added = match crate::browser::guard::loadable(url) {
        Some(url) => tabs.add(&open.frame.native, &url, project, shown, shared, proxy),
        None => Err(format!("`{url}` is not an address")),
    };
    match reply {
        Some(reply) => {
            let _ = reply.send(added);
        }
        None if added.is_err() => {
            if let Some(url) = crate::browser::guard::loadable(url) {
                let _ = tabs.active().load_url(&url);
            }
        }
        None => {}
    }
    if shown {
        open.show(Shown::Web);
    }
}
