use crate::mock_contract::MockContractInner;

use core::ffi::c_void;

// TODO(geoff): I cannot figure out how to prevent rust from moving objects
// without just converting back to raw pointers.
// Pin<Box<T>> with PhantomPinned still wound up getting moved, somehow.
// It would be nice to avoid this later of indirection, as that would make passing
// (or at least seem to make) passing closures in as parameters to test functions
// much easier.
pub struct TestEnvironment {
    db: *mut c_void,
    contract_context: *mut c_void,
    mock_contract_index: u32,
    contracts: Vec<*mut MockContractInner>, // owns these contracts
}
