//! llxterm's core (`llxterm::terminal::Core`, libghostty-vt), fed as a session feeds it; and the
//! same with a view's frame (`Render::frame`) taken after every read, which is what the app pays
//! each time it wakes before anything is drawn: the rows that changed, copied out of the core's
//! render state.

use llxterm::surface::Frame;
use llxterm::terminal::{Core, Render};

use crate::{COLUMNS, Engine, ROWS, Screen};

/// A blank core of the bench's size; its history is the app's own (about 10,000 rows).
fn core() -> Core {
    match Core::new(ROWS, COLUMNS) {
        Ok(core) => core,
        Err(refused) => panic!("{refused}"),
    }
}

/// The core's screen, its cursor counted from 0 as every engine's is.
fn screen_of(core: &Core) -> Screen {
    let (row, column) = core.cursor();
    let cursor = (row.saturating_sub(1), column.saturating_sub(1));
    Screen::new(cursor, core.text().lines().map(str::to_string))
}

pub struct Ours {
    core: Core,
}

impl Ours {
    pub fn new() -> Self {
        Self { core: core() }
    }
}

impl Engine for Ours {
    fn feed(&mut self, bytes: &[u8]) {
        self.core.feed(bytes);
    }

    fn screen(&self) -> Screen {
        screen_of(&self.core)
    }
}

pub struct Framed {
    core: Core,
    render: Render,
    frame: Frame,
}

impl Framed {
    pub fn new() -> Self {
        let render = match Render::new() {
            Ok(render) => render,
            Err(refused) => panic!("{refused}"),
        };
        Self {
            core: core(),
            render,
            frame: Frame::default(),
        }
    }
}

impl Engine for Framed {
    fn feed(&mut self, bytes: &[u8]) {
        self.core.feed(bytes);
        self.render.frame(&self.core, &mut self.frame, false);
    }

    fn screen(&self) -> Screen {
        screen_of(&self.core)
    }
}
