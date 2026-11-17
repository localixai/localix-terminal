//! The pointer in the view, told to the terminal: to the program when it takes the mouse, else a
//! selection by the core's gestures, scrolled on at the screen's pace while it is dragged past an
//! edge (a frame clock callback that exists only then); the wheel and the touchpad in whole rows;
//! and the menu a right click opens. Links and the scrollbar are `hover.rs`.

use std::sync::Arc;

use gtk::gdk;
use gtk::glib::{ControlFlow, Propagation};
use gtk::prelude::{WidgetExt, WidgetExtManual};

use super::super::acts::Picked;
use super::view::{View, handled};
use super::{acts, events, hover, keymap};
use crate::surface::{Button, Edge, Mouse, MouseAction, Routed};

/// What the pointer did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Pointer {
    Down,
    Dragged,
    Up,
    Moved,
}

/// The pointer as an event tells it: where (the view's points from its top left), the
/// modifiers held, the button (GDK's: 1 left, 2 middle, 3 right; 0 none) and its time (ms).
#[derive(Debug, Clone, Copy)]
pub(super) struct Point {
    pub(super) at: (f64, f64),
    pub(super) state: u32,
    pub(super) button: u32,
    pub(super) time: u32,
}

/// The view's pointer, wheel and crossing heard.
pub(super) fn connect(view: &View) {
    let (area, make) = (&view.area, view.maker());
    let made = make.clone();
    area.connect_button_press_event(move |area, event| {
        let view = made(area);
        let point = point(
            event.position(),
            event.state(),
            event.button(),
            event.time(),
        );
        // GTK's own double and triple presses: the core counts the single ones itself.
        if event.event_type() != gdk::EventType::ButtonPress {
            return Propagation::Stop;
        }
        handled(match point.button {
            3 => right_down(&view, point),
            _ => mouse(&view, point, Pointer::Down),
        })
    });
    let made = make.clone();
    area.connect_button_release_event(move |area, event| {
        let point = point(
            event.position(),
            event.state(),
            event.button(),
            event.time(),
        );
        handled(mouse(&made(area), point, Pointer::Up))
    });
    let made = make.clone();
    area.connect_motion_notify_event(move |area, event| {
        let held = event.state().intersects(
            gdk::ModifierType::BUTTON1_MASK
                | gdk::ModifierType::BUTTON2_MASK
                | gdk::ModifierType::BUTTON3_MASK,
        );
        let point = point(
            event.position(),
            event.state(),
            u32::from(held),
            event.time(),
        );
        let pointer = if held {
            Pointer::Dragged
        } else {
            Pointer::Moved
        };
        handled(mouse(&made(area), point, pointer))
    });
    let made = make.clone();
    area.connect_scroll_event(move |area, event| {
        // GDK counts down from the top; the terminal counts a turn up as more than 0.
        let direction = event.direction();
        let (dy, precise) = if direction == gdk::ScrollDirection::Smooth {
            (-event.delta().1, true)
        } else if direction == gdk::ScrollDirection::Up {
            (1.0, false)
        } else if direction == gdk::ScrollDirection::Down {
            (-1.0, false)
        } else {
            (0.0, false)
        };
        let point = point(event.position(), event.state(), 0, event.time());
        wheel(&made(area), point, dy, precise);
        Propagation::Stop
    });
    let made = make.clone();
    area.connect_enter_notify_event(move |area, _| {
        hover::pointer_in(&made(area), true);
        Propagation::Proceed
    });
    area.connect_leave_notify_event(move |area, _| {
        hover::pointer_in(&make(area), false);
        Propagation::Proceed
    });
}

fn point(at: (f64, f64), state: gdk::ModifierType, button: u32, time: u32) -> Point {
    Point {
        at,
        state: state.bits(),
        button,
        time,
    }
}

/// The pointer as the terminal reads it: in pixels, its button, and the modifiers held.
fn read(view: &View, point: Point, pointer: Pointer) -> Mouse {
    let scale = f64::from(view.area.scale_factor().max(1));
    let sides = view.state().map_or(0, |state| state.sides);
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a place in a view, in pixels"
    )]
    let (x, y) = ((point.at.0 * scale) as f32, (point.at.1 * scale) as f32);
    let button = match (pointer, point.button) {
        (Pointer::Moved, _) | (_, 0) => None,
        (_, 2) => Some(Button::Middle),
        (_, 3) => Some(Button::Right),
        _ => Some(Button::Left),
    };
    Mouse {
        x,
        y,
        button,
        action: match pointer {
            Pointer::Down => MouseAction::Press,
            Pointer::Up => MouseAction::Release,
            Pointer::Dragged | Pointer::Moved => MouseAction::Motion,
        },
        mods: keymap::mods(point.state, sides),
        time_ns: u64::from(point.time) * 1_000_000,
    }
}

/// The pointer `pointer` at `point`: whether the view took it.
pub(super) fn mouse(view: &View, point: Point, pointer: Pointer) -> bool {
    let Some(surface) = view.surface() else {
        return false;
    };
    if pointer == Pointer::Down {
        view.focus();
    }
    let mods = read(view, point, pointer).mods;
    // A Super-click on a link opens it; the scrollbar takes what is pressed on it.
    let theirs = match pointer {
        Pointer::Down => hover::open(view, point.at, mods) || hover::press(view, point.at),
        Pointer::Dragged => hover::drag(view, point.at),
        Pointer::Up => hover::release(view),
        Pointer::Moved => {
            hover::hover(view, point.at, mods);
            hover::moved(view, point.at);
            false
        }
    };
    if theirs {
        return true;
    }
    let taken = view.state().is_some_and(|state| state.frame.mouse_taken);
    // A pointer that only moves is the program's alone: no selection follows it.
    if pointer == Pointer::Moved && !taken {
        return false;
    }
    let Some(geometry) = view.geometry() else {
        return false;
    };
    let mouse = read(view, point, pointer);
    let routed = surface.mouse(&mouse, &geometry);
    match (routed, pointer) {
        (Routed::Selection { scroll: Some(edge) }, Pointer::Dragged) => drag(view, edge, mouse),
        _ => undrag(view),
    }
    view.redraw(false);
    true
}

/// A selection dragged past `edge`: the view scrolls on a row a frame of the screen while it
/// stays, from the area's frame clock.
fn drag(view: &View, edge: Edge, mouse: Mouse) {
    if let Some(mut state) = view.state()
        && let Some(dragging) = state.dragging.as_mut()
    {
        dragging.0 = edge;
        dragging.2 = mouse;
        return;
    }
    let make = view.maker();
    let tick = view.area.add_tick_callback(move |area, _| {
        let view = make(area);
        match drag_frame(&view) {
            true => ControlFlow::Continue,
            false => ControlFlow::Break,
        }
    });
    match view.state() {
        Some(mut state) => state.dragging = Some((edge, tick, mouse)),
        None => tick.remove(),
    }
}

pub(super) fn undrag(view: &View) {
    let dragging = view.state().and_then(|mut state| state.dragging.take());
    if let Some((_, tick, _)) = dragging {
        tick.remove();
    }
}

/// A frame while a selection is dragged past an edge: a row further, the selection taken to
/// where the pointer is. Whether it goes on.
fn drag_frame(view: &View) -> bool {
    let dragging = view.state().and_then(|state| {
        let (edge, _, mouse) = state.dragging.as_ref()?;
        Some((*edge, *mouse, Arc::clone(&state.surface)))
    });
    let (Some((edge, mouse, surface)), Some(geometry)) = (dragging, view.geometry()) else {
        return false;
    };
    let rows = if edge == Edge::Top { 1 } else { -1 };
    let still = Mouse {
        button: None,
        ..mouse
    };
    surface.wheel(rows, &still, &geometry);
    let goes_on = surface.mouse(&mouse, &geometry) != (Routed::Selection { scroll: None });
    if !goes_on {
        // The callback ends by its answer; its id is let go of without removing it again.
        drop(view.state().and_then(|mut state| state.dragging.take()));
    }
    view.redraw(false);
    goes_on
}

/// The wheel (`dy` steps, up when more than 0) or the touchpad (`precise`: rows, in parts):
/// whole rows, a touchpad's fine turns gathered until they make one.
pub(super) fn wheel(view: &View, point: Point, dy: f64, precise: bool) {
    let Some(surface) = view.surface() else {
        return;
    };
    let rows = {
        let Some(mut state) = view.state() else {
            return;
        };
        // A fine turn is in rows already: one row is one "point" here.
        acts::wheel_rows(&mut state.wheel, dy, precise, 1.0)
    };
    if rows == 0 {
        return;
    }
    let Some(geometry) = view.geometry() else {
        return;
    };
    surface.wheel(rows, &read(view, point, Pointer::Moved), &geometry);
    view.redraw(false);
}

/// The terminal's menu where the person right-clicked: Copy while something is selected, Paste,
/// Select All, and Clear on the main screen (a full-screen program draws all of its own). A
/// program that takes the mouse is told the click instead.
fn right_down(view: &View, point: Point) -> bool {
    let Some(surface) = view.surface() else {
        return false;
    };
    let taken = view.state().is_some_and(|state| state.frame.mouse_taken);
    if taken {
        return mouse(view, point, Pointer::Down);
    }
    view.focus();
    let main = view.state().is_some_and(|state| !state.frame.alternate);
    let (entries, picks) = acts::menu(surface.copy().is_some(), main);
    let area: &gtk::Widget = view.area.as_ref();
    let picked = crate::browser::menu_gtk::show_in(area, point.at, &entries);
    match picked.and_then(|place| picks.get(place)) {
        Some(Picked::Copy) => events::copy(view),
        Some(Picked::Paste) => events::paste(view),
        Some(Picked::SelectAll) => events::select_all(view),
        Some(Picked::Clear) => {
            surface.clear();
            view.redraw(false);
        }
        None => {}
    }
    true
}
