use super::interface::*;
use thruput_sdk::*;

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
        let client = ExnCatchClient::new(&target).map_exceptions(|exn| {
            match_exn!(exn, {
                AlphaError::NotFound => 2,
                AlphaError::Overflow => 3,
                BetaError::Denied => 4,
                // Undeclared by this interface; still a peer here.
                Undeclared => 5,
                LibraryExn::NonNegativeIntUnderflow => 6,
                _ => 7,
            })
        });
        let outcome = match which {
            0 => client.try_succeed().map(|_| ()),
            1 => client.try_throw_alpha_not_found(),
            2 => client.try_throw_alpha_overflow(),
            3 => client.try_throw_beta_denied(),
            4 => client.try_throw_undeclared(),
            6 => client.try_fault(),
            7 => client.try_malformed_abort(),
            _ => client.try_throw_library(),
        };
        match outcome {
            Ok(()) => 0,
            Err(call::RawCallError::Fault(_)) => 1,
            Err(call::RawCallError::Exception(code)) => code,
            Err(_) => 7,
        }
    }

    fn classify_then_throw(target: ContractAddress, which: u32) {
        // Encode a new rejection only after the call scratch has been released.
        if Self::classify(target, which) != 0 {
            throw(AlphaError::Overflow);
        }
    }

    fn rethrow(target: ContractAddress) {
        let result: Result<(), call::RawCallError<core::convert::Infallible>> =
            ExnCatchClient::new(&target)
                .map_exceptions(|exn| exn.re_throw())
                .try_throw_undeclared();
        result.or_re_throw();
    }

    fn fault() {
        panic!("test fault");
    }

    fn malformed_abort() {
        contract_abort(&[0xff]);
    }
}
