//! HTTP host validation, byte ranges, HEAD and content types.

use super::*;

/// A request written as given (its own headers), answered whole as bytes.
fn raw(address: &str, request: &str) -> Vec<u8> {
    let rest = address.strip_prefix("http://").unwrap();
    let (host, _) = rest.split_once('/').unwrap();
    let mut stream = TcpStream::connect(host).unwrap();
    stream.write_all(request.as_bytes()).unwrap();
    let mut answer = Vec::new();
    stream.read_to_end(&mut answer).unwrap();
    answer
}

/// The text of `raw`'s answer, lossily, and its body after the head.
fn raw_text(address: &str, request: &str) -> (String, String) {
    let answer = String::from_utf8_lossy(&raw(address, request)).to_string();
    let (head, body) = answer.split_once("\r\n\r\n").unwrap_or((&answer, ""));
    (head.to_string(), body.to_string())
}

fn host_of(address: &str) -> &str {
    let rest = address.strip_prefix("http://").unwrap();
    rest.split_once('/').unwrap().0
}

#[test]
fn a_request_for_another_host_name_is_refused() {
    let scratch = site();
    let served = Served::default();
    let address = served
        .address(scratch.path(), scratch.path(), "demo/shop/index.html")
        .unwrap()
        .unwrap();
    let path = path(&address);
    let host = host_of(&address);
    let port = host.rsplit_once(':').unwrap().1;
    // A page that DNS points at this port names its own host: refused, with no Referer to help.
    for other in [
        format!("attacker.example:{port}"),
        "127.0.0.1:1".to_string(),
    ] {
        let (head, body) = raw_text(
            &address,
            &format!("GET /style.css HTTP/1.1\r\nHost: {other}\r\n\r\n"),
        );
        assert!(head.starts_with("HTTP/1.1 403"), "{other}: {head}");
        assert!(!body.contains("red"), "{other}");
    }
    let (head, _) = raw_text(&address, &format!("GET {path} HTTP/1.0\r\n\r\n"));
    assert!(head.starts_with("HTTP/1.1 403"), "{head}");
    let (head, _) = raw_text(
        &address,
        &format!("GET {path} HTTP/1.1\r\nHost: localhost:{port}\r\n\r\n"),
    );
    assert!(head.starts_with("HTTP/1.1 200"), "{head}");
}

#[test]
fn a_range_of_a_file_is_answered_as_a_part_of_it_and_a_head_sends_no_body() {
    let scratch = site();
    scratch.file("demo/shop/clip.mp4", "0123456789");
    let served = Served::default();
    let address = served
        .address(scratch.path(), scratch.path(), "demo/shop/index.html")
        .unwrap()
        .unwrap();
    let clip = beside(&address, "clip.mp4");
    let host = host_of(&address);
    let ask = |method: &str, range: &str| {
        raw_text(
            &address,
            &format!("{method} {clip} HTTP/1.1\r\nHost: {host}\r\n{range}\r\n"),
        )
    };
    let (head, body) = ask("GET", "");
    assert!(head.starts_with("HTTP/1.1 200 OK"), "{head}");
    assert!(
        head.contains("Accept-Ranges: bytes") && head.contains("video/mp4"),
        "{head}"
    );
    assert_eq!(body, "0123456789");
    for (range, part, within) in [
        ("bytes=2-4", "234", "bytes 2-4/10"),
        ("bytes=7-", "789", "bytes 7-9/10"),
        ("bytes=-3", "789", "bytes 7-9/10"),
        ("bytes=8-99", "89", "bytes 8-9/10"),
        ("bytes=0-0", "0", "bytes 0-0/10"),
    ] {
        let (head, body) = ask("GET", &format!("Range: {range}\r\n"));
        assert!(
            head.starts_with("HTTP/1.1 206 Partial Content"),
            "{range}: {head}"
        );
        assert!(
            head.contains(&format!("Content-Range: {within}")),
            "{range}: {head}"
        );
        assert!(
            head.contains(&format!("Content-Length: {}", part.len())),
            "{range}: {head}"
        );
        assert_eq!(body, part, "{range}");
    }
    let (head, _) = ask("GET", "Range: bytes=10-\r\n");
    assert!(head.starts_with("HTTP/1.1 416"), "{head}");
    assert!(head.contains("Content-Range: bytes */10"), "{head}");
    // What is no single byte range is answered whole.
    for range in ["bytes=0-1,4-5", "items=1-2", "bytes=5-2", "bytes=x-"] {
        let (head, body) = ask("GET", &format!("Range: {range}\r\n"));
        assert!(head.starts_with("HTTP/1.1 200 OK"), "{range}: {head}");
        assert_eq!(body, "0123456789", "{range}");
    }
    let (head, body) = ask("HEAD", "");
    assert!(
        head.starts_with("HTTP/1.1 200 OK") && head.contains("Content-Length: 10"),
        "{head}"
    );
    assert_eq!(body, "");
}

#[test]
fn media_documents_and_fonts_are_served_with_their_types() {
    let scratch = site();
    for (name, kind) in [
        ("demo.xhtml", "application/xhtml+xml"),
        ("feed.xml", "application/xml"),
        ("a.webm", "video/webm"),
        ("a.MOV", "video/quicktime"),
        ("a.m4v", "video/mp4"),
        ("a.mp3", "audio/mpeg"),
        ("a.aac", "audio/aac"),
        ("a.flac", "audio/flac"),
        ("a.avif", "image/avif"),
        ("a.bmp", "image/bmp"),
        ("a.tiff", "image/tiff"),
        ("a.heic", "image/heic"),
        ("a.ttf", "font/ttf"),
        ("a.pdf", "application/pdf"),
        ("a.csv", "text/csv"),
    ] {
        scratch.file(&format!("demo/shop/{name}"), "x");
        let served = Served::default();
        let address = served
            .address(scratch.path(), scratch.path(), "demo/shop/index.html")
            .unwrap()
            .unwrap();
        let (_, answer) = get(&address, &beside(&address, name));
        assert!(
            answer.contains(&format!("Content-Type: {kind}")),
            "{name}: {answer}"
        );
    }
}

#[test]
fn a_request_with_no_prefix_is_answered_only_to_the_page_that_was_served() {
    let scratch = site();
    let served = Served::default();
    let shop = served
        .address(scratch.path(), scratch.path(), "demo/shop/index.html")
        .unwrap()
        .unwrap();
    let host = host_of(&shop);
    let unprefixed = |cookie: &str| {
        raw_text(
            &shop,
            &format!("GET /style.css HTTP/1.1\r\nHost: {host}\r\n{cookie}\r\n"),
        )
    };
    // Another user of this machine reaches the loopback too: with no prefix, no Referer and no
    // cookie, it is told nothing, even with one directory served.
    let (head, body) = unprefixed("");
    assert!(head.starts_with("HTTP/1.1 404"), "{head}");
    assert!(!body.contains("red"));
    let (head, _) = unprefixed("Cookie: llx-desk-root=desk-0000\r\n");
    assert!(head.starts_with("HTTP/1.1 404"), "{head}");
    // The page itself, by its prefix, gives its browser the cookie, for the browser alone.
    let page = path(&shop);
    let (head, _) = raw_text(
        &shop,
        &format!("GET {page} HTTP/1.1\r\nHost: {host}\r\n\r\n"),
    );
    let prefix = page.trim_start_matches('/').split('/').next().unwrap();
    let cookie = format!("Set-Cookie: llx-desk-root={prefix}; HttpOnly; SameSite=Strict; Path=/");
    assert!(head.contains(&cookie), "{head}");
    // With it, a built site's root-absolute names (`/style.css`) are found.
    let (head, body) = unprefixed(&format!("Cookie: theme=dark; llx-desk-root={prefix}\r\n"));
    assert!(head.starts_with("HTTP/1.1 200"), "{head}");
    assert!(body.contains("red"), "{body}");
}
