//! `throws(...)` accepts full paths, including two types that share a last
//! path segment. Those used to collide: the last segment named a variant of
//! the generated client error enum. With exceptions matched by selector
//! instead, the enum is gone and only the type identity matters.
#![allow(unexpected_cfgs)]

use thruput_sdk::*;

mod alpha {
    use thruput_sdk::*;

    #[contract_exception]
    pub enum AlphaError {
        Denied = 1,
    }

    #[contract_exception]
    pub enum E {
        One = 1,
    }
}

mod beta {
    use thruput_sdk::*;

    #[contract_exception]
    pub enum BetaError {
        Overflow = 1,
    }

    #[contract_exception]
    pub enum E {
        Two = 1,
    }
}

#[contract(throws(alpha::AlphaError, beta::BetaError, alpha::E, beta::E))]
pub trait ThrowsPathTrait {
    fn ping() -> u32;
}

/// Every declared type is matchable by its path, and so are the two that
/// share a last segment.
fn handle(e: &CallError) -> u32 {
    match_exn!(e, {
        alpha::AlphaError::Denied => 1,
        beta::BetaError::Overflow => 2,
        alpha::E::One => 3,
        beta::E::Two => 4,
        _ => 0,
    })
}

fn main() {
    // No host here, so exercise the fall-through rather than a real call.
    assert_eq!(handle(&CallError::Invalid), 0);
}
