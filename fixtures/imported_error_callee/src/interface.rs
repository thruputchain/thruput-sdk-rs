use thruput_sdk::*;

#[contract(throws(imported_error_type::ImportedError))]
pub trait ImportedErrorCallee {
    fn throw_imported_error();
}
