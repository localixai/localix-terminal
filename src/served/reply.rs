//! One request of a served page answered: its head read within a deadline, the file it names
//! found inside its directory, and sent with its type from the open file, a byte range of it when
//! asked (a `<video>` needs them), never read whole into memory. A request that names no root by
//! its prefix is answered only to a page that knows one: its Referer, or the cookie its first
//! answer set; another user of the machine reaches the loopback too, but never saw a prefix.

use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::net::{Shutdown, TcpStream};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use super::Root;
use super::kinds::kind;
use super::range::{Slice, slice};

/// How long a request's head may take to arrive in all: a client that sends nothing, or a byte
/// at a time, holds no other.
const READ: Duration = Duration::from_secs(5);
/// The most a request's head may be.
const HEAD: usize = 16 * 1024;
/// How long an answer may take to be written in all, however slowly its client reads.
const WRITE: Duration = Duration::from_secs(60);
/// The largest file served: what a page loads, not what a disk holds.
const FILE_MOST: u64 = 64 << 20;
/// What a file is sent in at a time.
const CHUNK: usize = 64 << 10;

/// What follows an answer's head.
enum Body {
    Text(&'static str),
    /// An open file, placed at the first byte to send.
    File(File),
}

/// An answer: its head's parts, and its body, of `length` bytes.
struct Reply {
    status: &'static str,
    kind: &'static str,
    length: u64,
    body: Body,
    /// A file: its bytes may be asked for by range.
    ranged: bool,
    /// What `Content-Range` says, of a part or of a range refused.
    within: Option<String>,
    /// The prefix of the root a file was found in by its prefix, kept by the browser as a cookie.
    root: Option<String>,
}

impl Reply {
    fn text(status: &'static str, words: &'static str) -> Self {
        Self {
            status,
            kind: "text/plain",
            length: words.len() as u64,
            body: Body::Text(words),
            ranged: false,
            within: None,
            root: None,
        }
    }
}

/// One request read and answered, then the connection closed.
pub(super) fn answer(mut stream: TcpStream, roots: &Mutex<HashMap<String, Root>>) {
    let began = Instant::now();
    let port = stream.local_addr().map(|local| local.port()).ok();
    let head = read_head(&mut stream, began);
    let head = String::from_utf8_lossy(&head);
    let mut words = head.lines().next().unwrap_or_default().split(' ');
    let (method, target) = (
        words.next().unwrap_or_default(),
        words.next().unwrap_or_default(),
    );
    let reply = match method {
        // A page of another name (DNS rebinding points one at this port) is not asked for.
        "GET" | "HEAD" if !ours(header(&head, "host"), port) => {
            Reply::text("403 Forbidden", "not served")
        }
        "GET" | "HEAD" => {
            let range = header(&head, "range").filter(|_| method == "GET");
            let from = Asker {
                referer: header(&head, "referer"),
                root: root_cookie(header(&head, "cookie")),
            };
            found(target, &from, range, roots)
        }
        _ => Reply::text("405 Method Not Allowed", "only GET"),
    };
    let _ = send(&mut stream, reply, method == "GET");
    let _ = stream.shutdown(Shutdown::Both);
}

/// The request's head, read until its blank line, within `READ` in all.
fn read_head(stream: &mut TcpStream, began: Instant) -> Vec<u8> {
    let mut head = Vec::new();
    let mut chunk = [0u8; 2048];
    while !head.windows(4).any(|end| end == b"\r\n\r\n") && head.len() < HEAD {
        let Some(left) = READ.checked_sub(began.elapsed()).filter(|d| !d.is_zero()) else {
            break;
        };
        let _ = stream.set_read_timeout(Some(left));
        match stream.read(&mut chunk) {
            Ok(0) | Err(_) => break,
            Ok(read) => head.extend_from_slice(chunk.get(..read).unwrap_or_default()),
        }
    }
    head
}

/// Whether `host` (the request's `Host`) is this server's own address.
fn ours(host: Option<&str>, port: Option<u16>) -> bool {
    let (Some(host), Some(port)) = (host, port) else {
        return false;
    };
    host == format!("127.0.0.1:{port}") || host.eq_ignore_ascii_case(&format!("localhost:{port}"))
}

/// The answer written: its head, then its body when the request was a GET, each write within
/// what is left of `WRITE`.
fn send(stream: &mut TcpStream, reply: Reply, body: bool) -> std::io::Result<()> {
    use std::fmt::Write as _;
    let ends = Instant::now() + WRITE;
    let within = |stream: &TcpStream| {
        let left = ends
            .checked_duration_since(Instant::now())
            .filter(|left| !left.is_zero())
            .ok_or(std::io::ErrorKind::TimedOut)?;
        stream.set_write_timeout(Some(left))
    };
    let mut head = format!(
        "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\n\
         Cache-Control: no-store\r\nConnection: close\r\n",
        reply.status, reply.kind, reply.length
    );
    if reply.ranged {
        head.push_str("Accept-Ranges: bytes\r\n");
    }
    if let Some(range) = &reply.within {
        let _ = write!(head, "Content-Range: {range}\r\n");
    }
    if let Some(root) = &reply.root {
        let _ = write!(
            head,
            "Set-Cookie: {COOKIE}={root}; HttpOnly; SameSite=Strict; Path=/\r\n"
        );
    }
    head.push_str("\r\n");
    within(stream)?;
    stream.write_all(head.as_bytes())?;
    if !body {
        return Ok(());
    }
    match reply.body {
        Body::Text(words) => {
            within(stream)?;
            stream.write_all(words.as_bytes())
        }
        Body::File(file) => {
            let mut file = file.take(reply.length);
            let mut chunk = vec![0u8; CHUNK];
            loop {
                let read = file.read(&mut chunk)?;
                if read == 0 {
                    return Ok(());
                }
                within(stream)?;
                stream.write_all(chunk.get(..read).unwrap_or_default())?;
            }
        }
    }
}

/// The value of the request's header `name`.
fn header<'a>(head: &'a str, name: &str) -> Option<&'a str> {
    head.lines().find_map(|line| {
        let (found, value) = line.split_once(':')?;
        found.eq_ignore_ascii_case(name).then(|| value.trim())
    })
}

/// The first segment of the path of `address`, an absolute address or a path.
fn first_segment(address: &str) -> &str {
    let path = address.split_once("://").map_or(address, |(_, rest)| {
        rest.find('/').map_or("", |at| rest.get(at..).unwrap_or(""))
    });
    (path.trim_start_matches('/'))
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default()
}

/// The cookie that names the root a browser's page was served from. A page's own request with no
/// prefix (a script's `fetch('/api.json')` under a policy that sends no Referer) carries it; a
/// request from anyone else on this machine, who can reach the loopback but never saw a prefix,
/// does not.
const COOKIE: &str = "llxterm-root";

/// The value of [`COOKIE`] in a request's `Cookie` header.
fn root_cookie(cookies: Option<&str>) -> Option<&str> {
    (cookies?.split(';')).find_map(|pair| pair.trim().strip_prefix(COOKIE)?.strip_prefix('='))
}

/// Where a request with no prefix says it comes from: the page that asked (`Referer`), and the
/// root its browser was served from ([`COOKIE`]).
struct Asker<'a> {
    referer: Option<&'a str>,
    root: Option<&'a str>,
}

/// What `target` names. A path with no prefix of a served root (a built site names
/// `/assets/app.css`) is read in the root its `referer` is of, else in the only root served when
/// the request's cookie names it: it knows a prefix, as its page did.
fn found(
    target: &str,
    from: &Asker<'_>,
    range: Option<&str>,
    roots: &Mutex<HashMap<String, Root>>,
) -> Reply {
    let missing = || Reply::text("404 Not Found", "not found");
    let refused = || Reply::text("403 Forbidden", "not served");
    let path = target.split(['?', '#']).next().unwrap_or_default();
    let mut parts = path.trim_start_matches('/').split('/').peekable();
    let roots = crate::lock(roots);
    let mut prefixed = None;
    let root = match parts.peek().and_then(|prefix| roots.get(*prefix)) {
        Some(root) => {
            prefixed = parts.next().map(str::to_string);
            Some(root.clone())
        }
        None => (from.referer)
            .and_then(|from| roots.get(first_segment(from)))
            .or_else(|| match roots.len() {
                1 => from.root.and_then(|root| roots.get(root)),
                _ => None,
            })
            .cloned(),
    };
    drop(roots);
    let Some(root) = root else {
        return missing();
    };
    let mut file = root.dir.clone();
    for part in parts.filter(|part| !part.is_empty()) {
        let Some(name) = decoded(part) else {
            return refused();
        };
        if name.starts_with('.') || name.contains(['/', '\\']) {
            return refused();
        }
        file.push(name);
    }
    if file.is_dir() {
        file.push("index.html");
    }
    let Ok(real) = std::fs::canonicalize(&file) else {
        return if file.exists() { refused() } else { missing() };
    };
    // Where a link leads is held to the same rules as the name asked for: inside the directory,
    // and no dot-file or dot-directory on the way (a link to `.git/config`, or to `.env`).
    let Ok(below) = real.strip_prefix(&root.dir) else {
        return refused();
    };
    if (below.components()).any(|part| part.as_os_str().to_string_lossy().starts_with('.')) {
        return refused();
    }
    let Ok(mut open) = File::open(&real) else {
        return missing();
    };
    let Some(length) = (open.metadata().ok())
        .filter(std::fs::Metadata::is_file)
        .map(|meta| meta.len())
    else {
        return missing();
    };
    if length > FILE_MOST {
        return Reply::text("413 Content Too Large", "larger than the desk serves");
    }
    let (status, start, length, within) = match slice(range, length) {
        Slice::Whole => ("200 OK", 0, length, None),
        Slice::Part(first, last) => (
            "206 Partial Content",
            first,
            last - first + 1,
            Some(format!("bytes {first}-{last}/{length}")),
        ),
        Slice::Refused => {
            let mut reply = Reply::text("416 Range Not Satisfiable", "not satisfiable");
            reply.ranged = true;
            reply.within = Some(format!("bytes */{length}"));
            return reply;
        }
    };
    if start > 0 && open.seek(SeekFrom::Start(start)).is_err() {
        return missing();
    }
    Reply {
        status,
        kind: kind(&file),
        length,
        body: Body::File(open),
        ranged: true,
        within,
        root: prefixed,
    }
}

/// A path's segment with its `%XX` escapes read; `None` when they are not UTF-8.
pub(super) fn decoded(part: &str) -> Option<String> {
    let bytes = part.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while let Some(&byte) = bytes.get(at) {
        if byte == b'%' {
            let hex = part.get(at + 1..at + 3)?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            at += 3;
        } else {
            out.push(byte);
            at += 1;
        }
    }
    String::from_utf8(out).ok()
}
