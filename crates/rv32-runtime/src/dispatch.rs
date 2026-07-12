//! Guest dispatch primitives, shared by the rv32 `_start` dispatcher.
//!
//! `Entrypoint` is the registry element each `#[contract]` method emits into
//! the `groundhog_rv32_entrypoints` section via `#[used] #[link_section]`.
//! `lookup` is the pure scan used by the dispatcher; it is target-independent
//! so it can be unit-tested on the host.

/// One method's dispatch record: its selector and the wrapper to invoke.
///
/// `wrapper` runs the calldata-decode / invoke / `sdk::internal::contract_return_encoded`
/// path; it diverges (never returns to the dispatcher) in practice.
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

    extern "C" fn a() {}
    extern "C" fn b() {}
    extern "C" fn c() {}

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
        assert_eq!(got as usize, a as *const () as usize);
    }

    #[test]
    fn first_match_wins_on_duplicate() {
        let t = table();
        let got = lookup(&t, 0x5555_6666_7777_8888).unwrap();
        assert_eq!(got as usize, b as *const () as usize);
    }

    #[test]
    fn unknown_method_is_none() {
        let t = table();
        assert!(lookup(&t, 0xDEAD_BEEF_DEAD_BEEF).is_none());
    }
}
