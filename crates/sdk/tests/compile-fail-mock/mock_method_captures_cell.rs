#![allow(unexpected_cfgs)]

// `Sync`, not just `Send`, is the bound that matters. A `Cell` moved into the
// body makes the closure `Send` -- the capture could travel to another thread
// -- but not `Sync`, and the stored body is invoked through a shared reference
// from whichever thread resumed the coroutine. Two frames reaching the same
// `Cell` that way would race it, so this must not compile. `AtomicU32` is the
// fix for a counter.

use sdk::contract::ContractEntrypointID;
use sdk::TestEnvironment;
use std::cell::Cell;

fn main() {
    let env = TestEnvironment::no_cost();
    let calls: Cell<u32> = Cell::new(0);

    env.register_mock(|c| {
        c.register_method(ContractEntrypointID::from_u64(1), move || {
            calls.set(calls.get() + 1);
        });
    });
}
