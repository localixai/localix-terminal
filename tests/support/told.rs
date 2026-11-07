//! What a page tells besides what it shows, through a real window: its dialogs (Cancel unless the
//! tab is armed), a toast gone before the next look, a field the form refused, a field that takes
//! keys only, a date written as people write it, a poll that is no work, a cover that fades.

use std::time::{Duration, Instant};

use llx_desk::browser::{Armed, Driver, Notice, Observation, WebviewDriver};
use llx_desk::view::Action;

fn reference(page: &Observation, role: &str, name: &str) -> String {
    page.elements
        .iter()
        .find(|element| element.role == role && element.name == name)
        .unwrap_or_else(|| panic!("no {role} \"{name}\" in {:#?}", page.elements))
        .reference
        .clone()
}

fn click(driver: &WebviewDriver, name: &str) {
    let page = driver.observe().unwrap();
    let target = reference(&page, "button", name);
    driver
        .act(&page.epoch, &Action::Click { reference: target })
        .unwrap_or_else(|error| panic!("click {name}: {error}"));
}

fn type_into(driver: &WebviewDriver, name: &str, text: &str) -> String {
    let page = driver.observe().unwrap();
    let target = reference(&page, "textbox", name);
    let typed = Action::Type {
        reference: target,
        text: text.into(),
    };
    driver.act(&page.epoch, &typed).unwrap();
    let page = driver.observe().unwrap();
    let field = page.elements.iter().find(|e| e.name == name).unwrap();
    field.value.clone().unwrap_or_default()
}

fn title(driver: &WebviewDriver) -> String {
    driver.observe().unwrap().title
}

/// What the tab told since `mark`, as words.
fn told(driver: &WebviewDriver, mark: u64) -> Vec<String> {
    driver
        .told_since(mark)
        .iter()
        .map(Notice::to_string)
        .collect()
}

pub(crate) fn scenario(driver: &WebviewDriver, origin: &str) {
    driver.navigate(&format!("{origin}/told.html")).unwrap();

    // A control in a frame that is part of the page's flow is read in the outline with its ref,
    // as the page's own are: the agent reads nothing else.
    let page = driver.observe().unwrap();
    let inner = reference(&page, "button", "Frame confirm");
    assert!(
        page.outline
            .contains(&format!("button \"Frame confirm\" [ref={inner}]")),
        "{}",
        page.outline
    );

    // A same-origin frame's confirm is answered Cancel and told, before any call armed an answer.
    let mark = driver.told_mark();
    driver
        .evaluate("document.getElementById('inner').contentDocument.querySelector('button').click(); 'ok'")
        .unwrap();
    assert_eq!(title(driver), "frame kept");
    assert_eq!(
        told(driver, mark),
        ["a confirm: \"Leave the frame?\", answered Cancel"]
    );

    // Words announced in the very click a caller's script makes are heard, with no earlier action
    // in this document to have started listening.
    let mark = driver.told_mark();
    let clicked = driver.evaluate(
        "[...document.querySelectorAll('button')].find((b) => b.textContent === 'Add to cart').click(); 'ok'",
    );
    assert_eq!(clicked.unwrap(), "ok");
    assert_eq!(told(driver, mark), ["announced: \"Fern added to cart\""]);
    // A list marked live, its controls in it, is content: a change to it is no announcement.
    let mark = driver.told_mark();
    driver
        .evaluate("document.querySelector('#feed button').click(); 'ok'")
        .unwrap();
    assert_eq!(told(driver, mark), Vec::<String>::new());

    // The same words shown again by a later action are told again.
    let mark = driver.told_mark();
    driver.evaluate(
        "[...document.querySelectorAll('button')].find((b) => b.textContent === 'Add to cart').click(); 'ok'",
    ).unwrap();
    assert_eq!(told(driver, mark), ["announced: \"Fern added to cart\""]);

    // A cover still fading out is waited for, not refused.
    click(driver, "Under the cover");
    assert_eq!(title(driver), "late clicked");

    // A confirm is answered Cancel, and told; armed, OK.
    let mark = driver.told_mark();
    click(driver, "Delete row");
    assert_eq!(title(driver), "kept");
    assert_eq!(
        told(driver, mark),
        ["a confirm: \"Delete the row?\", answered Cancel"]
    );
    driver.arm(Some(Armed { text: None }));
    click(driver, "Delete row");
    assert_eq!(title(driver), "deleted");
    driver.arm(Some(Armed {
        text: Some("Ada".into()),
    }));
    click(driver, "Name");
    assert_eq!(title(driver), "named:Ada");
    driver.arm(None);
    click(driver, "Name");
    assert_eq!(title(driver), "named:null");

    // An alert never blocks, and is told; so is a toast gone before the next look.
    let mark = driver.told_mark();
    click(driver, "Greet");
    assert_eq!(title(driver), "greeted");
    click(driver, "Save");
    let heard = told(driver, mark);
    assert_eq!(
        heard,
        ["an alert: \"Hi there\"", "announced: \"Saved\""],
        "{heard:?}"
    );

    // A form sent with a required field empty: the field reads as refused, with why.
    let page = driver.observe().unwrap();
    let email = reference(&page, "textbox", "Email");
    driver
        .act(
            &page.epoch,
            &Action::PressEnter {
                reference: Some(email),
            },
        )
        .unwrap();
    let page = driver.observe().unwrap();
    let field = page.elements.iter().find(|e| e.name == "Email").unwrap();
    assert!(
        field.states.iter().any(|s| s.starts_with("invalid")),
        "{:?}",
        field.states
    );
    assert_ne!(page.title, "signed up");

    // The form's fields and its button are known as one form; a field outside it as none.
    let page = driver.observe().unwrap();
    let form_of = |name: &str| {
        (page.elements.iter())
            .find(|e| e.name == name)
            .and_then(|e| e.form.clone())
    };
    assert!(form_of("Email").is_some());
    assert_eq!(form_of("Email"), form_of("Sign up"));
    assert_eq!(form_of("Phone"), None);

    // A value of signs only that the field refused is told, not taken as kept.
    let mark = driver.told_mark();
    assert_eq!(type_into(driver, "Code", "!!!!"), "");
    let heard = told(driver, mark);
    assert!(
        heard.iter().any(|notice| notice.contains("Code")),
        "{heard:?}"
    );

    // A field that takes keys only gets the value typed key by key; a date as people write it.
    assert_eq!(type_into(driver, "Phone", "5551234"), "(555) 1234");
    assert_eq!(type_into(driver, "Day", "Nov 5, 2026"), "2026-11-05");
    assert_eq!(type_into(driver, "Day", "05.11.2026"), "2026-11-05");

    // A poll every 200 ms is the page living: it does not keep the page busy.
    let deadline = Instant::now() + Duration::from_secs(3);
    while driver.observe().unwrap().busy {
        assert!(Instant::now() < deadline, "a poll keeps the page busy");
        std::thread::sleep(Duration::from_millis(100));
    }
    // The next scenario starts where the window opened.
    driver.navigate(&format!("{origin}/form.html")).unwrap();
}
