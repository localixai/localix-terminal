//! Whose child a process is: the terminal a program was started from is the one whose own program
//! it descends from, however many shells and runtimes are in between. So a call of the tool is
//! known by the terminal it came from, with nothing set in its environment (an agent may start its
//! tools with a bare one).

/// How far up a process's parents are followed: past any chain of shells an agent starts.
const DEPTH: usize = 64;

/// Process `pid`'s parent (macOS: its BSD info).
#[cfg(target_os = "macos")]
pub fn parent(pid: libc::pid_t) -> Option<libc::pid_t> {
    let mut info = std::mem::MaybeUninit::<libc::proc_bsdinfo>::zeroed();
    let size = libc::c_int::try_from(size_of::<libc::proc_bsdinfo>()).ok()?;
    // SAFETY: proc_pidinfo writes at most `size` bytes into `info`, and says how many.
    let wrote = unsafe {
        libc::proc_pidinfo(
            pid,
            libc::PROC_PIDTBSDINFO,
            0,
            info.as_mut_ptr().cast(),
            size,
        )
    };
    if wrote != size {
        return None;
    }
    // SAFETY: proc_pidinfo filled the whole of `info`, as `wrote` says.
    let info = unsafe { info.assume_init() };
    libc::pid_t::try_from(info.pbi_ppid)
        .ok()
        .filter(|parent| *parent > 0)
}

/// Process `pid`'s parent (Linux: the field after its name and state in `/proc`; the name is in
/// brackets and may hold any character, so the fields are read after its last one).
#[cfg(not(target_os = "macos"))]
pub fn parent(pid: libc::pid_t) -> Option<libc::pid_t> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let after = stat.rsplit_once(')')?.1;
    let parent: libc::pid_t = after.split_whitespace().nth(1)?.parse().ok()?;
    (parent > 0).then_some(parent)
}

/// The one of `among` that process `pid` is, or descends from: the nearest.
pub fn ancestor(pid: libc::pid_t, among: &[libc::pid_t]) -> Option<libc::pid_t> {
    let mut at = pid;
    for _ in 0..DEPTH {
        if among.contains(&at) {
            return Some(at);
        }
        at = parent(at)?;
    }
    None
}

#[cfg(test)]
#[path = "lineage_tests.rs"]
mod tests;
