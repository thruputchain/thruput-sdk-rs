//! Mock-execution utilities.

#[cfg(feature = "mock")]
mod contract_test;

#[cfg(feature = "mock")]
mod mock_public {
    pub use super::contract_test::*;

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
}

#[cfg(feature = "mock")]
pub use mock_public::*;

#[cfg(feature = "mock")]
#[doc(hidden)]
pub mod mock_registry;

#[cfg(feature = "mock")]
pub use mock_registry::register_mock_package;

#[cfg(feature = "mock")]
#[doc(hidden)]
pub extern crate linkme as __linkme;

#[cfg(feature = "mock")]
#[macro_export]
macro_rules! declare_mock_contract {
    () => {
        #[doc(hidden)]
        pub fn register_mock_contract(
            env: &::sdk::TestEnvironmentHandle,
        ) -> ::sdk::MockContract {
            ::sdk::register_mock_package(env, env!("CARGO_PKG_NAME"))
        }
    };
}

#[cfg(not(feature = "mock"))]
#[macro_export]
macro_rules! declare_mock_contract {
    () => {};
}
