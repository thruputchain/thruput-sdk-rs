#![cfg_attr(not(feature = "dev"), no_std)]

sdk::declare_mock_package!();

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
                    _
                ),
            ) => 1,
            _ => 0,
        }
    }
}
