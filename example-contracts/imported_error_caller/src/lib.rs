#![cfg_attr(not(feature = "dev"), no_std)]

sdk::declare_mock_contract!();

use sdk::*;

pub struct ImportedErrorCaller;

#[contract(client = ImportedErrorCallerClient)]
impl ImportedErrorCaller {
    pub fn classify_failure(target: &ContractAddress) -> u32 {
        match imported_error_callee::ImportedErrorCalleeClient::new(target)
            .try_throw_imported_error()
        {
            Err(e) => match_exn!(e, {
                imported_error_callee::ImportedError::Denied => 1,
                _ => 0,
            }),
            Ok(()) => 0,
        }
    }
}
