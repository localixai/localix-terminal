//! One engine on one workload, as the bench's `engine` is, for the engines built apart from
//! llxterm's core (rio-vt).

fn main() -> std::process::ExitCode {
    llxterm_bench::child::serve()
}
