#![allow(unexpected_cfgs)]

// A mock method body runs on whichever thread the coroutine resumed on, not
// the one that registered it. Recording calls through an `Rc` capture -- the
// obvious way to write a spy, and an easy reflex if you forget it leaves the
// registering thread -- would race the refcount across those threads, so
// `register_method` requires the body to be `Send + Sync`. `Arc` is the fix.
//
// The counter is atomic so that only the thread bounds fire here: an
// `Rc<RefCell<_>>` would also trip the pre-existing `UnwindSafe` bound, and
// this case would then still fail with the `Send + Sync` requirement removed.

use sdk::contract::ContractEntrypointID;
use sdk::TestEnvironment;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

fn main() {
    let env = TestEnvironment::no_cost();
    let calls: Rc<AtomicU32> = Rc::new(AtomicU32::new(0));

    env.register_mock(|c| {
        c.register_method(ContractEntrypointID::from_u64(1), move || {
            calls.fetch_add(1, Ordering::Relaxed);
        });
    });
}
