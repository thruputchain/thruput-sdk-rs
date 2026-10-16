//! Guest dispatch primitives, shared by the rv32 `_start` dispatcher.
//!
//! `Entrypoint` is the registry element each `#[contract]` method emits into
//! the `groundhog_rv32_entrypoints` section via `#[used] #[link_section]`.
//! `lookup` is the pure scan used by the dispatcher; it is target-independent
//! so it can be unit-tested on the host.

/// One method's dispatch record: its selector and the wrapper to invoke.
///
/// `wrapper` runs the calldata-decode / invoke /
/// `thruput_sdk::internal::contract_return_encoded` path; it diverges (never
/// returns to the dispatcher) in practice.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Entrypoint {
    pub method_id: u64,
    pub wrapper: extern "C" fn(),
}

/// Return the wrapper registered for `method_id`, or `None` if unknown.
/// Linear scan; first match wins on a (vanishingly unlikely) id collision.
pub fn lookup(
    table: &[Entrypoint],
    method_id: u64,
) -> Option<extern "C" fn()> {
    table
        .iter()
        .find(|e| e.method_id == method_id)
        .map(|e| e.wrapper)
}

#[cfg(test)]
mod tests {
    use super::*;

    use core::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Mutex;

    static LAST: AtomicU32 = AtomicU32::new(0);
    static LOCK: Mutex<()> = Mutex::new(());

    extern "C" fn a() {
        LAST.store(0xA, Ordering::Relaxed);
    }
    extern "C" fn b() {
        LAST.store(0xB, Ordering::Relaxed);
    }
    extern "C" fn c() {
        LAST.store(0xC, Ordering::Relaxed);
    }

    /// Call `f` and report which wrapper ran.
    ///
    /// Identity is checked by invoking the returned pointer rather than
    /// comparing addresses: Rust does not guarantee that casting a function
    /// to a pointer twice yields equal values (functions may be merged or
    /// duplicated), and Miri models that non-determinism, so neither
    /// `as usize` nor `fn_addr_eq` is a valid identity test. Calling also
    /// covers strictly more -- that the dispatched pointer is callable.
    ///
    /// Serialized: the wrappers share one cell and cargo runs tests in
    /// parallel threads.
    fn observe(f: extern "C" fn()) -> u32 {
        let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        LAST.store(0, Ordering::Relaxed);
        f();
        LAST.load(Ordering::Relaxed)
    }

    fn table() -> [Entrypoint; 3] {
        [
            Entrypoint {
                method_id: 0x1111_2222_3333_4444,
                wrapper: a,
            },
            Entrypoint {
                method_id: 0x5555_6666_7777_8888,
                wrapper: b,
            },
            // duplicate id to prove "first match wins"
            Entrypoint {
                method_id: 0x5555_6666_7777_8888,
                wrapper: c,
            },
        ]
    }

    #[test]
    fn finds_known_method() {
        let t = table();
        let got = lookup(&t, 0x1111_2222_3333_4444).unwrap();
        assert_eq!(observe(got), 0xA);
    }

    #[test]
    fn first_match_wins_on_duplicate() {
        let t = table();
        let got = lookup(&t, 0x5555_6666_7777_8888).unwrap();
        assert_eq!(observe(got), 0xB);
    }

    #[test]
    fn unknown_method_is_none() {
        let t = table();
        assert!(lookup(&t, 0xDEAD_BEEF_DEAD_BEEF).is_none());
    }
}
