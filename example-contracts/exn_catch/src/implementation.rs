use super::interface::*;
use sdk::*;

#[derive(Default)]
pub struct Contract;

#[contract]
impl ExnCatch for Contract {
    fn succeed() -> u32 {
        42
    }

    fn throw_alpha_not_found() {
        throw(AlphaError::NotFound)
    }

    fn throw_alpha_overflow() {
        throw(AlphaError::Overflow)
    }

    fn throw_beta_denied() {
        throw(BetaError::Denied)
    }

    fn throw_undeclared() {
        throw(Undeclared)
    }

    fn throw_library() {
        throw(LibraryExn::NonNegativeIntUnderflow)
    }

    fn classify(target: ContractAddress, which: u32) -> u32 {
        let client = ExnCatchClient::new(&target);

        let outcome = match which {
            0 => client.try_succeed().map(|_| ()),
            1 => client.try_throw_alpha_not_found(),
            2 => client.try_throw_alpha_overflow(),
            3 => client.try_throw_beta_denied(),
            4 => client.try_throw_undeclared(),
            _ => client.try_throw_library(),
        };

        match outcome {
            Ok(()) => 0,
            // Carries no exception, so it stays an ordinary `match` arm.
            Err(CallError::Fault(_)) => 1,
            Err(e) => match_exn!(e, {
                AlphaError::NotFound => 2,
                AlphaError::Overflow => 3,
                BetaError::Denied => 4,
                // Undeclared by this interface; still a peer here.
                Undeclared => 5,
                LibraryExn::NonNegativeIntUnderflow => 6,
                _ => 7,
            }),
        }
    }
}
