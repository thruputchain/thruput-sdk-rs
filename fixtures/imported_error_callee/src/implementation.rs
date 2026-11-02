use crate::ImportedErrorCallee;
use thruput_sdk::*;

pub struct ImportedErrorCalleeContract;

#[contract]
impl ImportedErrorCallee for ImportedErrorCalleeContract {
    fn throw_imported_error() {
        throw(imported_error_type::ImportedError::Denied);
    }
}
