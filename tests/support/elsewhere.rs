//! The tabs keep to local addresses, through real windows: a page's external frame loads where
//! it is, the agent's own click or redirect to an external address opens nowhere, and a shown
//! page that goes away by itself sends the person's browser the address, its tab staying put.
//! 192.0.2.1 is a documentation address: external to the rule, and never reached.

use std::time::{Duration, Instant};

use llx_desk::browser::{Browser, Driver, WebviewDriver, Window, sent_elsewhere};
use llx_desk::view::Action;

fn click(driver: &WebviewDriver, role: &str, name: &str) {
    let page = driver.observe().unwrap();
    let reference = (page.elements.iter())
        .find(|element| element.role == role && element.name == name)
        .unwrap_or_else(|| panic!("no {role} \"{name}\" in {:#?}", page.elements))
        .reference
        .clone();
    driver
        .act(&page.epoch, &Action::Click { reference })
        .unwrap_or_else(|error| panic!("click {name}: {error}"));
}

/// The page still at `path` after `wait`, the default browser sent nothing new.
fn stays(driver: &WebviewDriver, origin: &str, path: &str, wait: Duration, sent: usize) {
    std::thread::sleep(wait);
    let url = driver.observe().unwrap().url;
    assert_eq!(url, format!("{origin}/{path}"));
    assert_eq!(sent_elsewhere().len(), sent, "{:?}", sent_elsewhere());
}

pub(crate) fn scenario(driver: &WebviewDriver, origin: &str) {
    let before = sent_elsewhere().len();
    let start = driver.observe().unwrap().url;
    driver
        .navigate(&format!("{origin}/elsewhere.html"))
        .unwrap();
    // The external frame is the page's to embed: it neither moves the tab nor opens anything.
    stays(
        driver,
        origin,
        "elsewhere.html",
        Duration::from_millis(500),
        before,
    );

    // The agent's click on an external link: the tab stays, and the person's browser is left be.
    click(driver, "link", "External link");
    stays(
        driver,
        origin,
        "elsewhere.html",
        Duration::from_millis(1200),
        before,
    );

    // A local address the server redirects outside: the agent's navigation goes nowhere outside.
    let redirected = driver.navigate(&format!("{origin}/away/redirect"));
    std::thread::sleep(Duration::from_millis(1200));
    assert_eq!(
        sent_elsewhere().len(),
        before,
        "{redirected:?} {:?}",
        sent_elsewhere()
    );
    let url = driver.observe().unwrap().url;
    assert!(!url.contains("192.0.2.1"), "{redirected:?} {url}");
    // The scenarios after this one start where it found the tab.
    driver.navigate(&start).unwrap();
}

/// A shown window's page that goes away by itself, well after the agent's click settled: the
/// person's page going there. The default browser gets the address, and the tab stays.
/// A headless window's page never sends the person's browser anything.
pub(crate) fn shown(browser: &Browser, window: &Window, origin: &str) {
    let before = sent_elsewhere().len();
    let shown = Window {
        url: format!("{origin}/elsewhere.html"),
        headless: false,
        ..window.clone()
    };
    let driver = browser.open(shown).expect("a shown window opens");
    driver.wait_loaded(Duration::from_secs(10)).unwrap();
    click(&driver, "button", "Go later");
    let deadline = Instant::now() + Duration::from_secs(5);
    while sent_elsewhere().len() == before && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(100));
    }
    assert_eq!(
        sent_elsewhere()[before..],
        ["http://192.0.2.1/later".to_string()]
    );
    stays(
        &driver,
        origin,
        "elsewhere.html",
        Duration::ZERO,
        before + 1,
    );
    driver.close();
}
