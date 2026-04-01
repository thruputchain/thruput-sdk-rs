#![cfg_attr(not(feature = "dev"), no_std)]

use sdk::*;

pub struct ImportedErrorCaller;

#[contract(client = ImportedErrorCallerClient)]
impl ImportedErrorCaller {
    pub fn classify_failure(target: &ContractAddress) -> u32 {
        match imported_error_callee::ImportedErrorCalleeClient::new(target)
            .try_throw_imported_error()
        {
            Err(
                imported_error_callee::ImportedErrorCalleeClientError::ImportedError(
                    imported_error_callee::ImportedError::Denied,
                ),
            ) => 1,
            _ => 0,
        }
    }
}
