//! The app's menu bar (macOS): the app's own menu (About, Settings, Hide, Quit), Edit, and a
//! menu for each group of the shortcuts (shortcuts.rs: Tabs, Page, Window), each item doing what
//! its click on the strip does, so a key, a menu item and a button never do different things.
//! Made again when the person changes a shortcut. Quitting from the Dock or the system comes
//! here too (`quit_through`): the desk asks, then ends the app itself.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

use objc2::rc::{Allocated, Retained};
use objc2::runtime::{AnyClass, AnyObject, Bool, ClassBuilder, Sel};
use objc2::{class, msg_send, sel};
use tao::event_loop::EventLoopProxy;

use super::appkit::{self, text};
use super::host::Cmd;
use super::shortcuts::{self, Chord, Shortcut};

/// The group whose shortcuts are the app's own menu's.
const APP: &str = "LLX Desk";
/// What the app's menu says of quitting, which it holds last.
const QUIT: &str = "quit";

/// Each Edit item: its title, its action (sent to the first responder) and its key; an
/// upper-case key takes Shift too.
fn edits() -> [(&'static str, Sel, &'static str); 6] {
    [
        ("Undo", sel!(undo:), "z"),
        ("Redo", sel!(redo:), "Z"),
        ("Cut", sel!(cut:), "x"),
        ("Copy", sel!(copy:), "c"),
        ("Paste", sel!(paste:), "v"),
        ("Select All", sel!(selectAll:), "a"),
    ]
}

/// NSEvent's modifier flags.
const SHIFT: usize = 1 << 17;
const CONTROL: usize = 1 << 18;
const OPTION: usize = 1 << 19;
const COMMAND: usize = 1 << 20;

fn menu(title: &str) -> Retained<AnyObject> {
    // SAFETY: `alloc` answers an NSMenu to initialize.
    let made: Allocated<AnyObject> = unsafe { msg_send![class!(NSMenu), alloc] };
    // SAFETY: `initWithTitle:` takes an NSString and answers the menu, retained.
    unsafe { msg_send![made, initWithTitle: &*text(title)] }
}

fn item(title: &str, action: Option<Sel>, key: &str) -> Retained<AnyObject> {
    // SAFETY: `alloc` answers an NSMenuItem to initialize.
    let made: Allocated<AnyObject> = unsafe { msg_send![class!(NSMenuItem), alloc] };
    // SAFETY: `initWithTitle:action:keyEquivalent:` takes two NSStrings and a selector, which may
    // be nil, and answers the item, retained.
    unsafe {
        msg_send![
            made,
            initWithTitle: &*text(title),
            action: action,
            keyEquivalent: &*text(key)
        ]
    }
}

/// An item with its key's modifiers given (`mask`, NSEvent's flags).
fn masked(title: &str, action: Sel, key: &str, mask: usize) -> Retained<AnyObject> {
    let entry = item(title, Some(action), key);
    // SAFETY: an NSMenuItem takes a modifier mask, an NSUInteger.
    let _: () = unsafe { msg_send![&*entry, setKeyEquivalentModifierMask: mask] };
    entry
}

fn add(to: &AnyObject, item: &AnyObject) {
    // SAFETY: an NSMenu takes an NSMenuItem.
    let _: () = unsafe { msg_send![to, addItem: item] };
}

fn separator(to: &AnyObject) {
    // SAFETY: NSMenuItem's class method answers a separator item.
    let line: Retained<AnyObject> = unsafe { msg_send![class!(NSMenuItem), separatorItem] };
    add(to, &line);
}

/// A menu of the bar, holding `items`.
fn holding(title: &str, items: &AnyObject) -> Retained<AnyObject> {
    let holder = item(title, None, "");
    // SAFETY: an NSMenuItem takes an NSMenu as its submenu.
    let _: () = unsafe { msg_send![&*holder, setSubmenu: items] };
    holder
}

/// Where a shortcut picked goes, and what each item (by its tag) does.
static FIRING: Mutex<Option<(EventLoopProxy<Cmd>, Vec<String>)>> = Mutex::new(None);

extern "C" fn fire(_this: &AnyObject, _sel: Sel, sender: *mut AnyObject) {
    // SAFETY: the sender is the NSMenuItem whose key was pressed, alive while its action runs.
    let Some(item) = (unsafe { sender.as_ref() }) else {
        return;
    };
    // SAFETY: an NSMenuItem answers its tag, an NSInteger.
    let tag: isize = unsafe { msg_send![item, tag] };
    let firing = crate::lock(&FIRING);
    if let Some((proxy, does)) = firing.as_ref()
        && let Some(does) = usize::try_from(tag).ok().and_then(|at| does.get(at))
    {
        let _ = proxy.send_event(Cmd::Clicked(does.clone()));
    }
}

/// Whether a web page is shown now: what only a page can do is greyed while none is.
static PAGE: AtomicBool = AtomicBool::new(false);

/// A web page is shown now (`shown`), or a terminal, the board, the settings.
pub(super) fn page_shown(shown: bool) {
    PAGE.store(shown, Ordering::Relaxed);
}

/// Whether an item may be picked now: what needs a page, only while one is shown.
extern "C" fn validate(_this: &AnyObject, _sel: Sel, item: *mut AnyObject) -> Bool {
    // SAFETY: AppKit hands the NSMenuItem it asks about, alive for the call.
    let Some(item) = (unsafe { item.as_ref() }) else {
        return Bool::YES;
    };
    // SAFETY: an NSMenuItem answers its tag, an NSInteger.
    let tag: isize = unsafe { msg_send![item, tag] };
    let firing = crate::lock(&FIRING);
    let does = firing
        .as_ref()
        .and_then(|(_, does)| does.get(usize::try_from(tag).ok()?));
    let page_only = does.is_some_and(|does| shortcuts::of_page(does));
    Bool::new(!page_only || PAGE.load(Ordering::Relaxed))
}

/// What the shortcut items call: one object for the process.
fn firer() -> Option<&'static AnyObject> {
    static TARGET: OnceLock<Option<usize>> = OnceLock::new();
    let at = (*TARGET.get_or_init(|| {
        let mut class = ClassBuilder::new(c"LlxShortcut", AnyClass::get(c"NSObject")?)?;
        // SAFETY: an action's signature: the receiver, the selector, the sender.
        unsafe { class.add_method(sel!(fire:), fire as extern "C" fn(_, _, _)) };
        // SAFETY: as above, answering a BOOL: AppKit asks it before an item shows or fires.
        unsafe {
            class.add_method(
                sel!(validateMenuItem:),
                validate as extern "C" fn(_, _, _) -> _,
            );
        };
        let class = class.register();
        // SAFETY: `new` on an NSObject subclass answers a retained instance.
        let object: Retained<AnyObject> = unsafe { msg_send![class, new] };
        // Kept for the process: a menu item holds its target weakly.
        Some(Retained::into_raw(object) as usize)
    }))?;
    // SAFETY: the object was leaked above, so it lives as long as the process.
    Some(unsafe { &*(at as *const AnyObject) })
}

/// The menus' shortcut items, each firing its click (by its tag, an index of `does`).
struct Items<'a> {
    does: &'a mut Vec<String>,
}

impl Items<'_> {
    /// `shortcut`'s items into `menu`: one, or a row of places (`digit`) one for each; none when
    /// the person took its chord off.
    fn add(&mut self, menu: &AnyObject, shortcut: &Shortcut, chord: Option<&Chord>) {
        let Some(chord) = chord else {
            // No key, still a menu item: what it does stays a click away.
            if !shortcut.keys.contains("digit") {
                self.entry(menu, shortcut.title, "", 0, shortcut.does.to_string());
            }
            return;
        };
        for (chord, place) in chord.each() {
            let (key, shift) = chord.menu_key();
            let title = place.map_or_else(
                || shortcut.title.to_string(),
                |place| format!("Show Tab {place}"),
            );
            let mask = [
                (chord.cmd, COMMAND),
                (chord.ctrl, CONTROL),
                (chord.alt, OPTION),
                (shift, SHIFT),
            ]
            .iter()
            .filter(|(held, _)| *held)
            .fold(0, |mask, (_, bit)| mask | bit);
            let does = match place {
                Some(place) => format!("{} {place}", shortcut.does),
                None => shortcut.does.to_string(),
            };
            self.entry(menu, &title, &key, mask, does);
        }
    }

    fn entry(&mut self, menu: &AnyObject, title: &str, key: &str, mask: usize, does: String) {
        let entry = masked(title, sel!(fire:), key, mask);
        let tag = isize::try_from(self.does.len()).unwrap_or(isize::MAX);
        // SAFETY: an NSMenuItem takes an NSInteger tag.
        let _: () = unsafe { msg_send![&*entry, setTag: tag] };
        if let Some(target) = firer() {
            // SAFETY: an NSMenuItem takes a target object, which it holds weakly; this one lives
            // for the process.
            let _: () = unsafe { msg_send![&*entry, setTarget: target] };
        }
        add(menu, &entry);
        self.does.push(does);
    }
}

/// The app's own menu: About, its shortcuts (Settings), Hide, and Quit last.
fn app_menu(items: &mut Items<'_>, mine: &[(&Shortcut, Option<Chord>)]) -> Retained<AnyObject> {
    let app = menu(APP);
    add(
        &app,
        &item(
            "About LLX Desk",
            Some(sel!(orderFrontStandardAboutPanel:)),
            "",
        ),
    );
    separator(&app);
    for (shortcut, chord) in mine.iter().filter(|(s, _)| s.does != QUIT) {
        items.add(&app, shortcut, chord.as_ref());
    }
    separator(&app);
    add(&app, &item("Hide LLX Desk", Some(sel!(hide:)), "h"));
    add(
        &app,
        &masked(
            "Hide Others",
            sel!(hideOtherApplications:),
            "h",
            COMMAND | OPTION,
        ),
    );
    add(
        &app,
        &item("Show All", Some(sel!(unhideAllApplications:)), ""),
    );
    separator(&app);
    for (shortcut, chord) in mine.iter().filter(|(s, _)| s.does == QUIT) {
        items.add(&app, shortcut, chord.as_ref());
    }
    app
}

/// Give the app its menu bar, as the settings say now. Again after the person changes a
/// shortcut. Runs on the main thread, where the event loop is.
pub(super) fn install(proxy: &EventLoopProxy<Cmd>) {
    let settings = super::settings::now();
    let resolved = shortcuts::resolved(&settings.shortcuts);
    let mut does = vec![];
    let mut items = Items { does: &mut does };
    let bar = menu("");
    let mine: Vec<_> = resolved
        .iter()
        .filter(|(s, _)| s.group == APP)
        .map(|(s, c)| (*s, c.clone()))
        .collect();
    add(&bar, &holding(APP, &app_menu(&mut items, &mine)));
    let edit = menu("Edit");
    for (title, action, key) in edits() {
        add(&edit, &item(title, Some(action), key));
    }
    add(&bar, &holding("Edit", &edit));
    let app = appkit::app();
    for group in shortcuts::groups()
        .into_iter()
        .filter(|group| *group != APP)
    {
        let held = menu(group);
        for (shortcut, chord) in resolved.iter().filter(|(s, _)| s.group == group) {
            items.add(&held, shortcut, chord.as_ref());
        }
        add(&bar, &holding(group, &held));
        if group == "Window" {
            // SAFETY: the app takes an NSMenu as the one it lists its windows in.
            let _: () = unsafe { msg_send![&*app, setWindowsMenu: &*held] };
        }
    }
    *crate::lock(&FIRING) = Some((proxy.clone(), does));
    // SAFETY: the app takes an NSMenu as its main menu, and retains it.
    let _: () = unsafe { msg_send![&*app, setMainMenu: &*bar] };
}

/// NSApplicationTerminateReply: the app does not end now.
const TERMINATE_CANCEL: usize = 0;

/// The system asks whether the app may end (the Dock's Quit, logging out): not now. The person
/// is asked as with the app's own Quit, and the desk ends it once they are sure.
extern "C" fn should_terminate(_this: &AnyObject, _sel: Sel, _app: *mut AnyObject) -> usize {
    let firing = crate::lock(&FIRING);
    if let Some((proxy, _)) = firing.as_ref() {
        let _ = proxy.send_event(Cmd::Clicked(QUIT.into()));
    }
    TERMINATE_CANCEL
}

/// Quitting from the Dock or the system comes to the desk, as the app's Quit does: the app's
/// delegate (tao's) is taught `applicationShouldTerminate:`, which it does not answer itself.
pub(super) fn quit_through(proxy: &EventLoopProxy<Cmd>) {
    {
        let mut firing = crate::lock(&FIRING);
        if firing.is_none() {
            *firing = Some((proxy.clone(), vec![]));
        }
    }
    let app = appkit::app();
    // SAFETY: the app answers its delegate, or nil.
    let delegate: *mut AnyObject = unsafe { msg_send![&*app, delegate] };
    // SAFETY: checked for nil; the delegate lives as long as the app.
    let Some(delegate) = (unsafe { delegate.as_ref() }) else {
        return;
    };
    let class = std::ptr::from_ref(delegate.class()).cast_mut();
    type Method = extern "C" fn(&AnyObject, Sel, *mut AnyObject) -> usize;
    let method: Method = should_terminate;
    // SAFETY: a function pointer as the runtime's untyped one; the runtime calls it with the
    // selector's own arguments, which `Method` has.
    let imp = unsafe { std::mem::transmute::<Method, objc2::runtime::Imp>(method) };
    // SAFETY: the method has the selector's signature (`NSUInteger (id, SEL, NSApplication *)`,
    // its types `Q@:@`); `class_addMethod` adds it only when the class has none of its own.
    let _ = unsafe {
        objc2::ffi::class_addMethod(
            class,
            sel!(applicationShouldTerminate:),
            imp,
            c"Q@:@".as_ptr(),
        )
    };
}
