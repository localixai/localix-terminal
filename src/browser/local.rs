//! Which addresses a tab's page may be: this machine and the local network. The host is read as
//! the address spells it (a name, an IPv4 or an IPv6 literal), never looked up: a name resolved to
//! a local address now may resolve elsewhere a moment later, so only the name itself counts.

use std::net::{Ipv4Addr, Ipv6Addr};

use url::{Host, Origin, Url};

/// The names that never leave this machine or its network: the loopback's own, and the ones set
/// aside for the local network, tests and private use.
const LOCAL_SUFFIXES: [&str; 5] = [".localhost", ".local", ".test", ".internal", ".home.arpa"];

/// Whether `url` is a local address a tab may show: http or https on a local host (any port),
/// `about:` (a blank page, a frame's own document), or a `blob:` made by a local page or by a
/// document with no host of its own.
pub(crate) fn local(url: &str) -> bool {
    let Ok(parsed) = Url::parse(url.trim()) else {
        return false;
    };
    match parsed.scheme() {
        "about" => true,
        "blob" => match parsed.origin() {
            Origin::Tuple(_, host, _) => local_host(&host),
            Origin::Opaque(_) => true,
        },
        "http" | "https" => parsed.host().is_some_and(|host| local_host(&host)),
        _ => false,
    }
}

/// Whether `host` is this machine or its network: `localhost` and the local names, or an IP
/// literal of the loopback, the unspecified address, a private or a link-local range.
pub(crate) fn local_host<S: AsRef<str>>(host: &Host<S>) -> bool {
    match host {
        Host::Domain(name) => local_name(name.as_ref()),
        Host::Ipv4(ip) => local_v4(*ip),
        Host::Ipv6(ip) => local_v6(*ip),
    }
}

/// A name, lowercase as the parser leaves it, a trailing dot (the root) let go.
fn local_name(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    let name = name.strip_suffix('.').unwrap_or(&name);
    name == "localhost" || LOCAL_SUFFIXES.iter().any(|suffix| name.ends_with(suffix))
}

fn local_v4(ip: Ipv4Addr) -> bool {
    ip.is_loopback() || ip.is_unspecified() || ip.is_private() || ip.is_link_local()
}

fn local_v6(ip: Ipv6Addr) -> bool {
    let first = ip.segments()[0];
    let link_local = first & 0xffc0 == 0xfe80;
    let unique_local = first & 0xfe00 == 0xfc00;
    ip.is_loopback()
        || ip.is_unspecified()
        || link_local
        || unique_local
        || ip.to_ipv4_mapped().is_some_and(local_v4)
}

#[cfg(test)]
#[path = "local_tests.rs"]
mod tests;
