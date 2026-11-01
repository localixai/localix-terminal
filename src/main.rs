//! The `llx-desk` binary.

use std::process::ExitCode;

fn main() -> ExitCode {
    llx_desk::cli::run(std::env::args().skip(1).collect())
}
