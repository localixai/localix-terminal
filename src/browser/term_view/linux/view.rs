//! The terminal's view in the window (Linux): a GTK drawing area of its own (`TermView`) in the
//! window's box, where the tab's pages are, shown while its tab is and hidden otherwise. Its
//! size gives the terminal's grid; it takes the keys, the pointer and an input method's text
//! (`events.rs`, `pointer.rs`) and tells the terminal through its `Surface`; it draws when the
//! terminal says it changed (`paint.rs`), and while it is out of sight it draws nothing and
//! keeps no picture. To a screen reader it is a terminal by its role and name.

use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Mutex, MutexGuard, Weak};

use gtk::glib::Propagation;
use gtk::glib::clone::Downgrade;
use gtk::glib::thread_guard::ThreadGuard;
use gtk::prelude::{
    AtkObjectExt, BoxExt, Cast, ContainerExt, GtkWindowExt, IMContextExt, WidgetExt,
    WidgetExtManual,
};
use gtk::{atk, gdk};
use tao::dpi::LogicalSize;

pub(crate) use super::state::State;
use super::{events, pointer};
use crate::surface::{Count, FindStep, Surface};

/// A handle on the view: its drawing area, its input method and what it keeps. Each signal makes
/// one of the area it is called for; only the tab's `TermView` takes the area out of its window.
#[derive(Clone)]
pub(crate) struct View {
    pub(super) area: gtk::DrawingArea,
    pub(super) im: gtk::IMMulticontext,
    /// Shared by the view's signals on GTK's main thread, and reached from no other: the guard
    /// holds it to the thread that made it, so nothing of GTK's it holds is used from another.
    state: Arc<Mutex<ThreadGuard<State>>>,
}

/// The view's state, held for a moment on the thread that made it.
pub(super) struct Held<'a>(MutexGuard<'a, ThreadGuard<State>>);

impl Deref for Held<'_> {
    type Target = State;

    fn deref(&self) -> &State {
        self.0.get_ref()
    }
}

impl DerefMut for Held<'_> {
    fn deref_mut(&mut self) -> &mut State {
        self.0.get_mut()
    }
}

/// The view as its tab holds it: let go of, its area leaves the window.
pub(crate) struct TermView(View);

impl Deref for TermView {
    type Target = View;

    fn deref(&self) -> &View {
        &self.0
    }
}

impl Drop for TermView {
    /// The view out of its window for good, at once: the cursor's next turn and a selection's
    /// frame callback taken off, the window no longer heard, the input method given no window,
    /// its pictures and what the terminal drew from let go of, and the area destroyed, which
    /// takes it out of its box and drops every handler it holds (and the state they share).
    fn drop(&mut self) {
        let view = &self.0;
        view.unblink();
        pointer::undrag(view);
        view.unwatch();
        view.im.set_client_window(None);
        if let Some(mut state) = view.state() {
            state.over = super::state::Over::default();
            if let Some(painter) = state.painter.as_mut() {
                painter.forget();
            }
            state.surface.hidden();
        }
        // SAFETY: the tab's view is the area's last user: the handles its signals make live as
        // long as one call, and the others hold it weakly, so nothing uses it once destroyed.
        unsafe { view.area.destroy() };
    }
}

impl TermView {
    /// A view of `surface` in `native`'s box, out of sight until it is placed, which opens the
    /// links the person Super-clicks with `opens`; none where the window has no box.
    pub(crate) fn open(
        native: &tao::window::Window,
        surface: Arc<dyn Surface>,
        opens: Box<dyn Fn(&str)>,
    ) -> Option<Self> {
        use tao::platform::unix::WindowExtUnix;
        let vbox = native.default_vbox()?;
        let area = gtk::DrawingArea::new();
        area.set_widget_name(super::probe::NAME);
        area.set_can_focus(true);
        // Shown only as its tab is: a window shown whole leaves it as it is.
        area.set_no_show_all(true);
        area.add_events(
            gdk::EventMask::KEY_PRESS_MASK
                | gdk::EventMask::KEY_RELEASE_MASK
                | gdk::EventMask::BUTTON_PRESS_MASK
                | gdk::EventMask::BUTTON_RELEASE_MASK
                | gdk::EventMask::POINTER_MOTION_MASK
                | gdk::EventMask::SCROLL_MASK
                | gdk::EventMask::SMOOTH_SCROLL_MASK
                | gdk::EventMask::ENTER_NOTIFY_MASK
                | gdk::EventMask::LEAVE_NOTIFY_MASK
                | gdk::EventMask::FOCUS_CHANGE_MASK,
        );
        if let Some(reader) = area.accessible() {
            reader.set_role(atk::Role::Terminal);
            reader.set_name("Terminal");
        }
        let view = View {
            area,
            im: gtk::IMMulticontext::new(),
            state: Arc::new(Mutex::new(ThreadGuard::new(State::new(surface, opens)))),
        };
        view.connect();
        vbox.pack_start(&view.area, true, true, 0);
        view.watch(native.gtk_window());
        view.restyle();
        Some(Self(view))
    }
}

impl View {
    /// What the view keeps, held for a moment; none while it is held already (a call that came
    /// back into the view from GTK while it worked), which tells such a call to leave it alone.
    pub(super) fn state(&self) -> Option<Held<'_>> {
        self.state.try_lock().ok().map(Held)
    }

    /// What makes a handle on the view of the area a signal is called for.
    pub(super) fn maker(&self) -> impl Fn(&gtk::DrawingArea) -> Self + Clone + 'static {
        let (state, im) = (Arc::clone(&self.state), self.im.clone());
        move |area: &gtk::DrawingArea| View {
            area: area.clone(),
            im: im.clone(),
            state: Arc::clone(&state),
        }
    }

    /// The view's signals, each carried out on a handle of the area it is called for.
    fn connect(&self) {
        let (area, view) = (&self.area, self.maker());
        let made = view.clone();
        area.connect_draw(move |area, cr| {
            made(area).paint(cr);
            Propagation::Stop
        });
        let made = view.clone();
        area.connect_size_allocate(move |area, _| made(area).fit());
        let made = view.clone();
        area.connect_scale_factor_notify(move |area| made(area).restyle());
        area.connect_style_updated(move |area| view(area).restyle());
        let im = self.im.clone();
        area.connect_realize(move |area| im.set_client_window(area.window().as_ref()));
        let im = self.im.clone();
        area.connect_unrealize(move |_| im.set_client_window(None));
        events::connect(self);
        pointer::connect(self);
    }

    /// A handle on the view of `area` while it is there, for what is called without it (the
    /// input method's signals, a window's, a timeout).
    pub(super) fn weak(&self) -> impl Fn() -> Option<Self> + 'static {
        let (area, im, state) = (
            self.area.downgrade(),
            self.im.clone(),
            Arc::downgrade(&self.state),
        );
        move || {
            Some(View {
                area: area.upgrade()?,
                im: im.clone(),
                state: Weak::upgrade(&state)?,
            })
        }
    }

    /// The view hears its window become the active one, or stop being it (and no longer the
    /// window it heard before).
    fn watch(&self, window: &gtk::ApplicationWindow) {
        self.unwatch();
        let view = self.weak();
        let handler = window.connect_is_active_notify(move |window| {
            let Some(view) = view() else {
                return;
            };
            let toplevel = view.area.toplevel();
            if toplevel.is_some_and(|top| top == *window.upcast_ref::<gtk::Widget>()) {
                events::window_key_changed(&view);
            }
        });
        if let Some(mut state) = self.state() {
            state.watching = Some((window.downgrade(), handler));
        }
    }

    /// The window the view heard no longer heard.
    fn unwatch(&self) {
        let watching = self.state().and_then(|mut state| state.watching.take());
        if let Some((window, handler)) = watching
            && let Some(window) = window.upgrade()
        {
            gtk::glib::ObjectExt::disconnect(&window, handler);
        }
    }

    /// Whether the view's window is the active one.
    pub(super) fn key_window(&self) -> bool {
        (self.area.toplevel())
            .and_then(|top| top.downcast::<gtk::Window>().ok())
            .is_some_and(|window| window.is_active())
    }

    /// The view in its tab's pane: a GTK box lays it out itself, so it is only shown or hidden;
    /// out of sight, it draws nothing and lets go of its pictures.
    pub(crate) fn place(&self, _x: f64, _top: f64, _size: LogicalSize<f64>, shown: bool) {
        self.show(shown);
    }

    /// A divider of its split held or let go: a GTK box lays the view out itself, filling the
    /// window's, so no divider moves it.
    #[expect(
        clippy::unused_self,
        reason = "the macOS view's hold, which GTK's own layout makes nothing here"
    )]
    pub(crate) fn hold(&self, _on: bool) {}

    /// The keys to this view, in its window.
    pub(crate) fn focus(&self) {
        self.area.grab_focus();
    }

    /// Whether the view holds its window's keys.
    pub(crate) fn has_keys(&self) -> bool {
        self.area.is_focus()
    }

    /// The view moved into `native`'s box, as it is: its terminal goes on.
    pub(crate) fn reparent(&self, native: &tao::window::Window) {
        use tao::platform::unix::WindowExtUnix;
        let parent = self.area.parent();
        if let Some(parent) = parent.and_then(|parent| parent.downcast::<gtk::Container>().ok()) {
            parent.remove(&self.area);
        }
        if let Some(vbox) = native.default_vbox() {
            vbox.pack_start(&self.area, true, true, 0);
        }
        self.watch(native.gtk_window());
    }

    /// Under the window's dialog (`on`), the view takes no click, no key and no pointer.
    pub(crate) fn hush(&self, on: bool) {
        if let Some(mut state) = self.state() {
            state.hushed = on;
        }
    }

    /// The terminal the view shows, if the view can be told anything now.
    pub(super) fn surface(&self) -> Option<Arc<dyn Surface>> {
        let state = self.state()?;
        (!state.hushed).then(|| Arc::clone(&state.surface))
    }

    /// A step of a find in the terminal's text, its matches drawn: where it is and how many.
    pub(crate) fn find(&self, step: &FindStep) -> Count {
        let Some(surface) = self.surface() else {
            return Count::default();
        };
        let count = surface.find(step);
        // A recount moves nothing and lights nothing new: the view draws as output comes.
        if *step != FindStep::Again {
            self.redraw(false);
        }
        count
    }

    /// What the program asked to put on the clipboard since this was last asked, and whether
    /// it rang the bell: the window decides where either goes.
    pub(crate) fn told(&self) -> (Option<String>, bool) {
        let surface = self.state().map(|state| Arc::clone(&state.surface));
        surface.map_or((None, false), |surface| (surface.copied(), surface.rang()))
    }
}

/// A signal's answer: handled here, or not, for GTK to go on with.
pub(super) const fn handled(done: bool) -> Propagation {
    match done {
        true => Propagation::Stop,
        false => Propagation::Proceed,
    }
}
