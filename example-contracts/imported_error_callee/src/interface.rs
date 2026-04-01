use sdk::*;

#[contract(throws(crate::ImportedError))]
pub trait ImportedErrorCallee {
    fn throw_imported_error();
}
