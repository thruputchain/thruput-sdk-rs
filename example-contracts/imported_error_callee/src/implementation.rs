use crate::{ImportedError, ImportedErrorCallee};
use sdk::*;

pub struct ImportedErrorCalleeContract;

#[contract]
impl ImportedErrorCallee for ImportedErrorCalleeContract {
    fn throw_imported_error() {
        throw(ImportedError::Denied);
    }
}
