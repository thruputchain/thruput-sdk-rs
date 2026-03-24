//! Development utilities.
//!
//! *Requires feature `dev`.*

mod contract_test;
pub use contract_test::*;

mod test_setup;

pub use core::convert::Infallible;
pub use groundhog_native_exec::{
    frame_builder::{
        ContinuableFrame, FrameAction, FrameBuilder, FrameHandle,
    },
    frame_result::FrameResult,
    mock_contract::MockContract,
    mock_transaction_handler::MockTransactionHandler,
    test_environment::{TestEnvironment, TestEnvironmentHandle},
    test_helpers::Named,
    transact_result::TransactResult,
};
pub use hogexec_internal::ffi_enums::*;

pub use groundhog_storage_cost::*;

mod panic;

pub fn dev_init() {
    panic::init();
}
