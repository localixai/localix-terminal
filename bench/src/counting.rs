//! What the kernel counts for a process: the instructions it retired and the cycles it ran
//! (`proc_pid_rusage` on macOS), the fast cores asked for, and a child's peak resident memory
//! read as it is waited for (`wait4`).

use std::io;
use std::process::Child;

/// What this process has run so far: the instructions its threads retired and the CPU cycles
/// they ran, as the kernel counts them for it. The instructions of the same work are the same on
/// every run, on any core: a change of a tenth of a percent is a change of the code. Elsewhere
/// than macOS there is no such count, and both are zero.
#[cfg(target_os = "macos")]
pub fn counters() -> (u64, u64) {
    /// `struct rusage_info_v4` of `<sys/resource.h>`: a UUID, then 35 counters, of which
    /// `ri_instructions` is the 30th and `ri_cycles` the 31st.
    #[repr(C)]
    struct Usage {
        uuid: [u8; 16],
        counters: [u64; 35],
    }
    unsafe extern "C" {
        fn proc_pid_rusage(pid: i32, flavor: i32, buffer: *mut Usage) -> i32;
    }
    let mut usage = Usage {
        uuid: [0; 16],
        counters: [0; 35],
    };
    // SAFETY: a buffer of `rusage_info_v4`'s layout, for the flavor that fills it (4).
    let read = unsafe { proc_pid_rusage(libc::getpid(), 4, &raw mut usage) };
    match read {
        0 => (usage.counters[29], usage.counters[30]),
        _ => (0, 0),
    }
}

#[cfg(not(target_os = "macos"))]
pub fn counters() -> (u64, u64) {
    (0, 0)
}

/// This thread asks for the fast cores (QoS user-interactive on macOS): an efficiency core runs
/// the same work slower, so a run that moved between them would not compare.
pub fn fast_cores() {
    #[cfg(target_os = "macos")]
    {
        unsafe extern "C" {
            fn pthread_set_qos_class_self_np(qos: u32, relative: i32) -> i32;
        }
        // SAFETY: QOS_CLASS_USER_INTERACTIVE (0x21) for the calling thread.
        unsafe { pthread_set_qos_class_self_np(0x21, 0) };
    }
}

/// Waits for `child` and returns whether it succeeded and the most memory it ever held resident,
/// in bytes, as the kernel kept it for that process alone.
pub fn wait_with_peak(child: &Child) -> io::Result<(bool, u64)> {
    let pid = i32::try_from(child.id()).map_err(io::Error::other)?;
    let mut status = 0;
    // SAFETY: a zeroed `rusage` is a valid one for `wait4` to fill.
    let mut usage: libc::rusage = unsafe { std::mem::zeroed() };
    // SAFETY: a child of this process not yet waited for; both out-pointers live through the call.
    let waited = unsafe { libc::wait4(pid, &raw mut status, 0, &raw mut usage) };
    if waited != pid {
        return Err(io::Error::last_os_error());
    }
    let ok = libc::WIFEXITED(status) && libc::WEXITSTATUS(status) == 0;
    // `ru_maxrss` is in bytes on macOS and in kibibytes on Linux.
    let peak = u64::try_from(usage.ru_maxrss).unwrap_or(0);
    let peak = match cfg!(target_os = "macos") {
        true => peak,
        false => peak * 1024,
    };
    Ok((ok, peak))
}
