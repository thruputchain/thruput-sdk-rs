#![allow(unexpected_cfgs)]

// Backtrace holds pointers into the with_debug_context stack frames; letting
// user code capture and store one would be UB from safe code. It must not be
// reachable outside the sdk crate.
fn main() {
    let bt = sdk::Backtrace::capture();
    let _ = std::format!("{}", bt);
}
