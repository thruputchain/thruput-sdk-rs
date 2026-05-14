pub mod frame_builder;
pub mod frame_result;
pub mod host_fn_error;
pub mod invoke_error;
pub use host_fn_error::perform_mocked_syscall;
pub mod mock_contract;
pub mod mock_transaction_handler;
pub mod test_environment;
pub mod test_helpers;
pub mod transact_result;
pub mod transaction_handler;

#[cfg(target_family = "wasm")]
compile_error!("The `native_exec` crate cannot be compiled for wasm.");
