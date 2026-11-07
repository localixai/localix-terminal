//! Tests of `served`.

#[path = "served/http_tests.rs"]
mod http;

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use super::*;
use crate::testing::Scratch;

/// `GET path` of `address`'s server, as a browser asks: its status line and its whole answer.
fn get(address: &str, path: &str) -> (String, String) {
    get_from(address, path, None)
}

/// [`get`] with the `Referer` a browser sends for a page's own subresources.
fn get_from(address: &str, path: &str, referer: Option<&str>) -> (String, String) {
    let rest = address.strip_prefix("http://").unwrap();
    let (host, _) = rest.split_once('/').unwrap();
    let mut stream = TcpStream::connect(host).unwrap();
    let referer = referer.map_or_else(String::new, |r| format!("Referer: {r}\r\n"));
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: {host}\r\n{referer}\r\n"
    )
    .unwrap();
    let mut answer = String::new();
    stream.read_to_string(&mut answer).unwrap();
    let status = answer.lines().next().unwrap_or_default().to_string();
    (status, answer)
}

/// The path part of `address`.
fn path(address: &str) -> String {
    let (_, rest) = address.split_once("/desk-").unwrap();
    format!("/desk-{rest}")
}

/// The path part of `address`, its directory's own prefix with `name` after it.
fn beside(address: &str, name: &str) -> String {
    let path = path(address);
    let (dir, _) = path.rsplit_once('/').unwrap();
    format!("{dir}/{name}")
}

fn site() -> Scratch {
    let scratch = Scratch::new();
    scratch.file(
        "demo/shop/index.html",
        "<link rel=stylesheet href=style.css><h1>Shop</h1>",
    );
    scratch.file("demo/shop/style.css", "h1 { color: red }");
    scratch.file("demo/shop/img/logo.svg", "<svg/>");
    scratch.file("demo/shop/.env", "KEY=secret");
    scratch.file("demo/notes.txt", "outside the page's directory");
    scratch
}

#[test]
fn a_local_page_is_opened_by_its_path_with_its_directory_beside_it() {
    let scratch = site();
    let served = Served::default();
    let address = served
        .address(scratch.path(), scratch.path(), "demo/shop/index.html")
        .unwrap()
        .unwrap();
    assert!(address.starts_with("http://127.0.0.1:"), "{address}");
    assert!(address.ends_with("/index.html"), "{address}");
    let (status, page) = get(&address, &path(&address));
    assert_eq!(status, "HTTP/1.1 200 OK");
    assert!(
        page.contains("text/html") && page.contains("<h1>Shop</h1>"),
        "{page}"
    );
    let (status, style) = get(&address, &beside(&address, "style.css"));
    assert_eq!(status, "HTTP/1.1 200 OK");
    assert!(
        style.contains("text/css") && style.contains("color: red"),
        "{style}"
    );
    let (status, logo) = get(&address, &beside(&address, "img/logo.svg"));
    assert_eq!(status, "HTTP/1.1 200 OK");
    assert!(logo.contains("image/svg+xml"), "{logo}");
    // Edited between two loads, the page shows the edit.
    assert!(page.contains("no-store"), "{page}");
}

#[test]
fn a_directory_opens_its_index_and_an_address_or_a_file_url_are_read_as_such() {
    let scratch = site();
    let served = Served::default();
    let address = served
        .address(scratch.path(), scratch.path(), "./demo/shop")
        .unwrap()
        .unwrap();
    assert!(address.ends_with("/index.html"), "{address}");
    let file = format!(
        "file://{}",
        scratch.path().join("demo/shop/index.html").display()
    );
    assert!(
        served
            .address(scratch.path(), scratch.path(), &file)
            .unwrap()
            .is_some()
    );
    // A web address is not a file here: the browser reads it.
    for web in ["example.com/shop", "https://example.com", "localhost:3000"] {
        assert_eq!(
            served.address(scratch.path(), scratch.path(), web).unwrap(),
            None,
            "{web}"
        );
    }
    // A path that names no file says so, rather than opening a site of that name.
    let Err(why) = served.address(scratch.path(), scratch.path(), "./demo/missing.html") else {
        panic!("a missing file was opened");
    };
    assert!(why.contains("no file"), "{why}");
}

#[test]
fn nothing_outside_the_page_s_directory_nor_hidden_nor_off_limits_is_served() {
    let scratch = site();
    scratch.file("demo/shop/private/key.pem", "secret");
    let served = Served::default();
    let address = served
        .address(scratch.path(), scratch.path(), "demo/shop/index.html")
        .unwrap()
        .unwrap();
    for refused in [
        "../notes.txt",
        "%2e%2e/notes.txt",
        ".env",
        "private/../../notes.txt",
    ] {
        let (status, answer) = get(&address, &beside(&address, refused));
        assert!(
            status.contains(" 404 ") || status.contains(" 403 "),
            "{refused}: {status}"
        );
        assert!(
            !answer.contains("secret") && !answer.contains("outside"),
            "{refused}"
        );
    }
    // Another directory's prefix serves nothing.
    let (status, _) = get(&address, "/desk-0000/index.html");
    assert!(status.contains(" 404 "), "{status}");
}

#[test]
fn each_directory_has_a_prefix_of_its_own_and_the_same_one_keeps_it() {
    let scratch = site();
    scratch.file("other/index.html", "<h1>Other</h1>");
    let served = Served::default();
    let shop = served
        .address(scratch.path(), scratch.path(), "demo/shop/index.html")
        .unwrap()
        .unwrap();
    let again = served
        .address(scratch.path(), scratch.path(), "demo/shop/")
        .unwrap()
        .unwrap();
    let other = served
        .address(scratch.path(), scratch.path(), "other/index.html")
        .unwrap()
        .unwrap();
    assert_eq!(shop, again);
    assert_ne!(beside(&shop, ""), beside(&other, ""));
    // The shop's prefix never reaches the other directory's files.
    let (status, _) = get(
        &shop,
        &format!("{}../../other/index.html", beside(&shop, "")),
    );
    assert!(!status.contains(" 200 "), "{status}");
}

/// A page outside the workspace (a download, a system file) would hand its whole directory to
/// whatever script it holds: only the project's own pages open from disk.
#[test]
fn only_a_page_inside_the_workspace_opens_from_disk() {
    let scratch = site();
    let elsewhere = Scratch::new();
    let page = elsewhere.file("evil.html", "<script>fetch('notes.txt')</script>");
    let served = Served::default();
    for outside in [
        page.display().to_string(),
        format!("file://{}", page.display()),
    ] {
        let Err(why) = served.address(scratch.path(), scratch.path(), &outside) else {
            panic!("{outside} was opened");
        };
        assert!(why.contains("not inside"), "{why}");
    }
}

/// A browser opens connections ahead of the requests it will send on them: one that has sent
/// nothing yet must not hold up a page asked for on another, or every click waits for it.
#[test]
fn a_connection_that_sends_nothing_holds_up_no_other() {
    let scratch = site();
    let served = Served::default();
    let address = served
        .address(scratch.path(), scratch.path(), "demo/shop/index.html")
        .unwrap()
        .unwrap();
    let rest = address.strip_prefix("http://").unwrap();
    let (host, _) = rest.split_once('/').unwrap();
    let idle: Vec<TcpStream> = (0..3).map(|_| TcpStream::connect(host).unwrap()).collect();
    let started = std::time::Instant::now();
    let (status, _) = get(&address, &path(&address));
    assert_eq!(status, "HTTP/1.1 200 OK");
    assert!(
        started.elapsed() < Duration::from_millis(500),
        "the page waited {:?} behind idle connections",
        started.elapsed()
    );
    drop(idle);
}

#[test]
fn a_root_absolute_request_is_answered_from_the_root_its_page_came_from() {
    let scratch = site();
    scratch.file("demo/other/index.html", "<h1>Other</h1>");
    scratch.file("demo/other/style.css", "h1 { color: blue }");
    scratch.file("demo/shop/assets/app.js", "shop()");
    let served = Served::default();
    let shop = served
        .address(scratch.path(), scratch.path(), "demo/shop/index.html")
        .unwrap()
        .unwrap();
    let other = served
        .address(scratch.path(), scratch.path(), "demo/other/index.html")
        .unwrap()
        .unwrap();
    // A built site names `/style.css`, no prefix: its Referer says which directory it means.
    let (status, css) = get_from(&shop, "/style.css", Some(&shop));
    assert_eq!(status, "HTTP/1.1 200 OK");
    assert!(css.contains("red"), "{css}");
    let (_, css) = get_from(&other, "/style.css", Some(&other));
    assert!(css.contains("blue"), "{css}");
    let (status, _) = get_from(&shop, "/assets/app.js", Some(&shop));
    assert_eq!(status, "HTTP/1.1 200 OK");
    // With two roots and no Referer, no guess is made.
    let (status, _) = get_from(&shop, "/style.css", None);
    assert_eq!(status, "HTTP/1.1 404 Not Found");
    // Every guard holds: not outside the root, not a dot-file.
    for path in ["/%2E%2E/notes.txt", "/.env", "/img/%2E%2E/%2E%2E/notes.txt"] {
        let (status, body) = get_from(&shop, path, Some(&shop));
        assert!(!status.contains("200"), "{path}: {body}");
    }
}

#[test]
fn with_one_root_served_a_root_absolute_request_needs_no_referer() {
    let scratch = site();
    let served = Served::default();
    let shop = served
        .address(scratch.path(), scratch.path(), "demo/shop/index.html")
        .unwrap()
        .unwrap();
    let (status, css) = get_from(&shop, "/style.css", None);
    assert_eq!(status, "HTTP/1.1 200 OK");
    assert!(css.contains("red"), "{css}");
}

#[test]
fn a_link_inside_the_directory_to_a_dot_file_is_not_followed() {
    let scratch = site();
    scratch.file("demo/shop/.git/config", "url = https://token@example.com");
    let dir = scratch.path().join("demo/shop");
    std::os::unix::fs::symlink(".git/config", dir.join("leak")).unwrap();
    std::os::unix::fs::symlink(".env", dir.join("env.txt")).unwrap();
    std::os::unix::fs::symlink("style.css", dir.join("again.css")).unwrap();
    let served = Served::default();
    let address = served
        .address(scratch.path(), scratch.path(), "demo/shop/index.html")
        .unwrap()
        .unwrap();
    for name in ["leak", "env.txt"] {
        let (status, answer) = get(&address, &beside(&address, name));
        assert!(status.contains(" 403 "), "{name}: {status}");
        assert!(
            !answer.contains("token") && !answer.contains("secret"),
            "{name}"
        );
    }
    // A link to an ordinary file in the directory still works.
    let (status, css) = get(&address, &beside(&address, "again.css"));
    assert_eq!(status, "HTTP/1.1 200 OK");
    assert!(css.contains("red"), "{css}");
}

#[test]
fn a_file_url_may_spell_spaces_and_a_page_may_carry_a_query_or_a_hash() {
    let scratch = site();
    scratch.file("My Site/index.html", "<h1>Spaced</h1>");
    scratch.file("demo/odd#name.html", "<h1>Odd</h1>");
    let served = Served::default();
    let open = |url: &str| {
        served
            .address(scratch.path(), scratch.path(), url)
            .unwrap()
            .unwrap()
    };
    let spaced = open(&format!(
        "file://{}/My%20Site/index.html",
        scratch.path().display()
    ));
    assert!(spaced.ends_with("/index.html"), "{spaced}");
    assert!(get(&spaced, &path(&spaced)).1.contains("Spaced"));
    let hashed = open("demo/shop/index.html#/cart");
    assert!(hashed.ends_with("/index.html#/cart"), "{hashed}");
    let queried = open("demo/shop?step=2");
    assert!(queried.ends_with("/index.html?step=2"), "{queried}");
    // The page itself is found without them, and a file name that holds a `#` is a file first.
    let (status, page) = get(&queried, path(&queried).split('?').next().unwrap());
    assert_eq!(status, "HTTP/1.1 200 OK");
    assert!(page.contains("<h1>Shop</h1>"), "{page}");
    assert!(open("demo/odd#name.html").contains("odd%23name.html"));
    // A tail never makes a file of what is none, nor takes a page out of the project.
    let Err(why) = served.address(scratch.path(), scratch.path(), "./demo/none.html?x=1") else {
        panic!("a missing file was opened");
    };
    assert!(why.contains("no file"), "{why}");
}
