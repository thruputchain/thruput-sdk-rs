//! Mock-execution utilities.

#[cfg(feature = "dev")]
mod contract_test;

#[cfg(feature = "dev")]
mod mock_public {
    pub use super::contract_test::*;

    pub use thruput_trace::ffi_enums::*;
    pub use thruput_native_exec::{
        frame_builder::{
            ContinuableFrame, FrameAction, FrameBuilder, FrameHandle,
        },
        frame_result::FrameResult,
        // `PanicLocation` and `PanicRecord` are named by
        // `FaultInfo::location` and `TransactResult::fault_panic`.
        invoke_error::{InvokeError, PanicLocation, PanicRecord},
        mock_contract::MockContract,
        mock_transaction_handler::{
            MockTransactionConfig, MockTransactionHandler,
        },
        // The optional non-default-settings lane: `TestEnvironment`'s own
        // constructors stay the ordinary way in.
        network_config,
        network_config::NetworkConfigBuilder,
        rv32_stack_usage::{Rv32StackUsage, SP_UNWRITTEN},
        test_environment::{
            TestEnvironment, TestEnvironmentHandle, test_network_hash,
        },
        test_environment_builder::TestEnvironmentBuilder,
        test_helpers::Named,
        transact_result::TransactResult,
        transaction_builder::TransactionBuilder,
        try_commit_result::{
            DbConflictCode, DbConflictDiagnostic, DbConflictResult,
            TryCommitResult,
        },
    };

    pub use thruput_storage_cost::*;
}

#[cfg(feature = "dev")]
pub use mock_public::*;

#[cfg(feature = "dev")]
#[doc(hidden)]
pub mod mock_registry;

#[cfg(feature = "dev")]
pub use mock_registry::register_mock_package;

#[cfg(feature = "dev")]
#[doc(hidden)]
pub extern crate linkme as __linkme;

#[cfg(feature = "dev")]
#[macro_export]
macro_rules! declare_mock_contract {
    () => {
        #[doc(hidden)]
        pub fn register_mock_contract(
            env: &$crate::TestEnvironmentHandle,
        ) -> $crate::MockContract {
            $crate::register_mock_package(env, env!("CARGO_PKG_NAME"))
        }
    };
}

#[cfg(not(feature = "dev"))]
#[macro_export]
macro_rules! declare_mock_contract {
    () => {};
}
