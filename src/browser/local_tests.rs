//! Tests of `local`.

use super::*;

#[test]
fn this_machine_and_the_local_network_are_local() {
    for url in [
        "http://localhost",
        "http://localhost:3000/x",
        "https://LOCALHOST:8443/",
        "http://localhost./",
        "http://app.localhost:5173",
        "http://printer.local",
        "http://Printer.Local./setup",
        "http://shop.test",
        "http://api.internal:8080",
        "http://nas.home.arpa",
        "http://127.0.0.1:4000",
        "http://127.8.9.10",
        "http://127.1/",
        "http://2130706433/",
        "http://0.0.0.0:3000",
        "http://10.0.0.7",
        "http://172.16.0.1",
        "http://172.31.255.254",
        "http://192.168.1.2:8000",
        "http://169.254.1.1",
        "http://[::1]:3000",
        "http://[::]/",
        "http://[fe80::1]",
        "http://[febf::1]",
        "http://[fc00::1]",
        "http://[fd12:3456::1]",
        "http://[::ffff:192.168.1.2]",
        "http://[::ffff:127.0.0.1]:80",
        "http://user:pass@localhost:3000/",
        // A name past ASCII, under a local name, is still under it once spelled in punycode.
        "http://bücher.local/",
        "about:blank",
        "about:srcdoc",
        "blob:http://localhost:3000/5c2a-11",
        "blob:null/5c2a-11",
    ] {
        assert!(local(url), "{url} is local");
    }
}

#[test]
fn the_rest_of_the_internet_is_not() {
    for url in [
        "https://example.com",
        "http://localhost.evil.com",
        "http://127.0.0.1.nip.io",
        "http://localhost@evil.com",
        "http://localhost:3000@evil.com/",
        "http://evil.com/?http://localhost",
        "http://evil.com#localhost",
        "http://notlocalhost",
        "http://local",
        "http://evil-local",
        "http://evil.localdomain",
        "http://evil.test.com",
        "http://8.8.8.8",
        "http://172.32.0.1",
        "http://172.15.255.255",
        "http://192.169.0.1",
        "http://11.0.0.1",
        "http://169.255.0.1",
        "http://[2001:db8::1]",
        "http://[fec0::1]",
        "http://[::ffff:8.8.8.8]",
        // A look-alike of localhost, with a Cyrillic "о", is another name.
        "http://lоcalhost/",
        "blob:https://example.com/5c2a-11",
    ] {
        assert!(!local(url), "{url} is not local");
    }
}

#[test]
fn only_pages_are_local_never_files_scripts_or_mail() {
    for url in [
        "file:///etc/passwd",
        "javascript:alert(1)",
        "data:text/html,hi",
        "mailto:a@localhost",
        "ftp://localhost/",
        "localhost:3000",
        "",
        "http://",
    ] {
        assert!(!local(url), "{url:?} is not a local page");
    }
}
