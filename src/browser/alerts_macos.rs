//! The system's side of what waits for the person (alerts.rs), on macOS: a notice in Notification
//! Center for each new thing while they are in another app, taken back by its event's number once
//! it no longer waits, a click on one said as a click in the strip; and the Dock's count of what
//! is new. Silent: a notice makes no sound. Only the app's bundle has them, as the system gives
//! notices to an app it can name.

use std::sync::{Mutex, OnceLock};

use block2::{Block, RcBlock};
use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject, Bool, ClassBuilder, Sel};
use objc2::{class, msg_send, sel};
use tao::event_loop::EventLoopProxy;

use super::alerts::{Change, Note};
use super::appkit::{app, literal, text};
use super::host::Cmd;

#[link(name = "UserNotifications", kind = "framework")]
unsafe extern "C" {}

/// `UNAuthorizationOptionAlert`: a notice shown, with no sound and no badge of the system's.
const ALERT: usize = 1 << 2;

/// Where a click on a notice is sent: the loop of the window.
static CLICKS: Mutex<Option<EventLoopProxy<Cmd>>> = Mutex::new(None);

/// Whether the person is in the app now.
pub(super) fn active() -> bool {
    // SAFETY: NSApplication answers whether it is the active app, a BOOL.
    let active: Bool = unsafe { msg_send![&*app(), isActive] };
    active.as_bool()
}

/// This run of the app, as its notices name it: a click on one is heard only in the run that
/// posted it, since an event's number means another event in the next.
fn run() -> &'static str {
    static RUN: OnceLock<String> = OnceLock::new();
    RUN.get_or_init(|| {
        let begun = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH);
        format!(
            "{}-{}",
            std::process::id(),
            begun.map_or(0, |at| at.as_nanos())
        )
    })
}

/// `change` carried out: the Dock's count, the notices posted and taken back.
pub(super) fn carry(change: &Change, proxy: &EventLoopProxy<Cmd>) {
    badge(change.badge);
    static BEGUN: OnceLock<()> = OnceLock::new();
    let begins = BEGUN.set(()).is_ok();
    if !begins && change.post.is_empty() && change.withdraw.is_empty() {
        return;
    }
    let Some(class) = AnyClass::get(c"UNUserNotificationCenter") else {
        return;
    };
    // The system raises where it cannot name the app (a program outside a bundle): alerts.rs
    // asks only in the bundle, and were it to raise all the same, nothing is posted.
    let asked = objc2::exception::catch(|| -> Retained<AnyObject> {
        // SAFETY: the class method answers the app's center, retained.
        unsafe { msg_send![class, currentNotificationCenter] }
    });
    let Ok(center) = asked else { return };
    if begins {
        begin(&center, proxy);
    }
    if !change.withdraw.is_empty() {
        let gone = ids(&change.withdraw);
        // SAFETY: both take an NSArray of NSString identifiers and return nothing.
        let _: () =
            unsafe { msg_send![&*center, removeDeliveredNotificationsWithIdentifiers: &*gone] };
        // SAFETY: as above.
        let _: () = unsafe {
            msg_send![&*center, removePendingNotificationRequestsWithIdentifiers: &*gone]
        };
    }
    if change.post.is_empty() {
        return;
    }
    static ASKED: OnceLock<()> = OnceLock::new();
    if ASKED.set(()).is_ok() {
        ask(center, change.post.clone());
    } else {
        for note in &change.post {
            post(&center, note);
        }
    }
}

/// The Dock's icon counts `count` things; none, it says nothing.
fn badge(count: usize) {
    let label = (count > 0).then(|| text(&count.to_string()));
    // SAFETY: NSApplication answers its Dock tile.
    let tile: Retained<AnyObject> = unsafe { msg_send![&*app(), dockTile] };
    // SAFETY: `setBadgeLabel:` takes an NSString or nil and returns nothing.
    let _: () = unsafe { msg_send![&*tile, setBadgeLabel: label.as_deref()] };
}

/// The app's first word to the center: the notices an earlier run left are taken back, as what
/// they were about is listed anew or gone, and clicks on notices go to `proxy` from here on.
fn begin(center: &AnyObject, proxy: &EventLoopProxy<Cmd>) {
    *crate::lock(&CLICKS) = Some(proxy.clone());
    if let Some(hears) = hears() {
        // SAFETY: `setDelegate:` takes an object and returns nothing; the center holds it
        // weakly, and this one lives as long as the process.
        let _: () = unsafe { msg_send![center, setDelegate: hears] };
    }
    // SAFETY: it takes nothing and returns nothing.
    let _: () = unsafe { msg_send![center, removeAllDeliveredNotifications] };
}

/// The person asked, once, whether the app may show notices, with the first of them at hand:
/// `first` are posted when they say yes. Their answer is the system's to keep, and a notice
/// refused is simply not shown.
fn ask(center: Retained<AnyObject>, first: Vec<Note>) {
    let asked = center.clone();
    let answered = RcBlock::new(move |granted: Bool, _error: *mut AnyObject| {
        for note in first.iter().filter(|_| granted.as_bool()) {
            post(&center, note);
        }
    });
    // SAFETY: the options are an NSUInteger, the block `void (^)(BOOL, NSError *)`, which the
    // center copies and calls once.
    let _: () = unsafe {
        msg_send![&*asked, requestAuthorizationWithOptions: ALERT, completionHandler: &*answered]
    };
}

/// `note` posted at once, under its event's number, so it is taken back by it.
fn post(center: &AnyObject, note: &Note) {
    // SAFETY: `new` answers an empty content, retained.
    let content: Retained<AnyObject> =
        unsafe { msg_send![class!(UNMutableNotificationContent), new] };
    // SAFETY: `setTitle:` takes an NSString and returns nothing.
    let _: () = unsafe { msg_send![&*content, setTitle: &*text(&note.title)] };
    // SAFETY: `setBody:` takes an NSString and returns nothing.
    let _: () = unsafe { msg_send![&*content, setBody: &*text(&note.body)] };
    if let Some(key) = literal(c"click") {
        // SAFETY: a dictionary of one NSString under one NSString key, answered retained.
        let info: Retained<AnyObject> = unsafe {
            msg_send![class!(NSDictionary), dictionaryWithObject: &*text(&format!("{} {}", run(), note.click)), forKey: &*key]
        };
        // SAFETY: `setUserInfo:` takes an NSDictionary and returns nothing.
        let _: () = unsafe { msg_send![&*content, setUserInfo: &*info] };
    }
    let id = text(&note.id.to_string());
    // SAFETY: an identifier, a content, and no trigger (nil: delivered now); it answers a request.
    let request: Retained<AnyObject> = unsafe {
        msg_send![class!(UNNotificationRequest), requestWithIdentifier: &*id, content: &*content, trigger: None::<&AnyObject>]
    };
    // SAFETY: a request and no completion handler (nil); it returns nothing.
    let _: () = unsafe {
        msg_send![center, addNotificationRequest: &*request, withCompletionHandler: None::<&Block<dyn Fn(*mut AnyObject)>>]
    };
}

/// `ids` as an NSArray of the strings notices are known by.
fn ids(ids: &[u64]) -> Retained<AnyObject> {
    // SAFETY: `new` answers an empty mutable array, retained.
    let array: Retained<AnyObject> = unsafe { msg_send![class!(NSMutableArray), new] };
    for id in ids {
        // SAFETY: `addObject:` takes an object and returns nothing.
        let _: () = unsafe { msg_send![&*array, addObject: &*text(&id.to_string())] };
    }
    array
}

/// A notice was clicked: what it says is said to the window, as a click in the strip. The system
/// brings the app to the front itself.
extern "C" fn clicked(
    _this: &AnyObject,
    _sel: Sel,
    _center: &AnyObject,
    response: &AnyObject,
    done: &Block<dyn Fn()>,
) {
    if let (Some(said), Some(proxy)) = (said(response), crate::lock(&CLICKS).as_ref()) {
        let _ = proxy.send_event(Cmd::Clicked(said));
    }
    done.call(());
}

/// What the notice `response` answers was posted to say when clicked; none for a notice an
/// earlier run posted.
fn said(response: &AnyObject) -> Option<String> {
    let path = literal(c"notification.request.content.userInfo.click")?;
    // SAFETY: key-value coding walks the response to its notification, request, content and
    // user info (a dictionary), and answers what lies under the last key, or nil.
    let said: Option<Retained<AnyObject>> = unsafe { msg_send![response, valueForKeyPath: &*path] };
    let said = said?;
    // SAFETY: what was posted under the key is an NSString; its UTF-8 lives as long as it does.
    let utf8: *const std::ffi::c_char = unsafe { msg_send![&*said, UTF8String] };
    if utf8.is_null() {
        return None;
    }
    // SAFETY: non-null and NUL-terminated, alive while `said` is, and copied out here.
    let said = unsafe { std::ffi::CStr::from_ptr(utf8) }.to_string_lossy();
    // A notice of another run says nothing here.
    let (posted_by, click) = said.split_once(' ')?;
    (posted_by == run()).then(|| click.to_string())
}

/// What the center tells of a click: one object for the process.
fn hears() -> Option<&'static AnyObject> {
    static TARGET: OnceLock<Option<usize>> = OnceLock::new();
    let at = (*TARGET.get_or_init(|| {
        let mut class = ClassBuilder::new(c"LlxAlerts", AnyClass::get(c"NSObject")?)?;
        // SAFETY: the delegate method's signature: the receiver, the selector, the center, the
        // response, and a block of `void (^)(void)` to call when done.
        unsafe {
            class.add_method(
                sel!(userNotificationCenter:didReceiveNotificationResponse:withCompletionHandler:),
                clicked as extern "C" fn(_, _, _, _, _),
            );
        };
        let class = class.register();
        // SAFETY: `new` on an NSObject subclass answers a retained instance.
        let object: Retained<AnyObject> = unsafe { msg_send![class, new] };
        Some(Retained::into_raw(object) as usize)
    }))?;
    // SAFETY: the object was leaked above, so it lives as long as the process.
    Some(unsafe { &*(at as *const AnyObject) })
}
