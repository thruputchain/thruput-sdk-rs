//! # thruput-sys
//!
//! Low-level FFI bindings to the Thruput test executor, the native library
//! that `thruput-native-exec` runs contracts on. The bindings are generated
//! from the executor's C headers with bindgen; the build script fetches and
//! links the library itself.
//!
//! This crate is an implementation detail of `thruput-native-exec`. Use that
//! crate, or `thruput-sdk`'s `dev` feature, instead.

// suppress spurious warnings from bindgen
#![allow(non_camel_case_types)]
#![allow(non_upper_case_globals)]
#![allow(non_snake_case)]
#![allow(dead_code)]

// Private module containing all bindgen-generated bindings
mod bindings {
    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}

// Export all non-weird bindgen symbols (excludes __ prefixed items, _opaque_* structs, etc.)

// Constants
#[rustfmt::skip]
pub use bindings::{
    TEST_FFI_VERSION_MAJOR,
    TEST_FFI_VERSION_MINOR,
    TEST_FFI_VERSION_PATCH,
};

// Types
#[rustfmt::skip]
pub use bindings::{
    ForeignInvokeResult,
    ForeignTracedInvokeResult,
    ForeignEntryPoint,
    ForeignHostFnResult,
    ForeignTryCommitResult,
    CreateDBResult,
    OpenRuntimeContextResult,
    FFISerializedSignedTransaction,
    OpenSyscallHandlerResult,
    FFISourceAuthData,
    FFISerializedAuthClaimData,
    FFISerializedInvocation,
    VerifyAuthResult,
    VerifyAuthResultFromDBResult,
    FFIDataBuffer,
    GetCostModelResult,
    GetNetworkConfigResult,
    GetSnapshotStorageItemResult,
    VerifyContractResult,
    FFISerializedAddressAndKey,
    FFIRv32StackUsage,
    GetRv32StackUsageResult,
};

// Functions
#[rustfmt::skip]
pub use bindings::{
    create_db,
    free_db,
    get_cost_model,
    get_network_config,
    get_snapshot_storage_item,
    advance_new_block,
    open_runtime_context,
    close_runtime_context,
    register_new_contract,
    setup_create_contract,
    setup_deploy_contract,
    setup_grant_contract_storage_tokens,
    open_syscall_handler,
    open_mocked_syscall_handler,
    open_mock_invocation_frame,
    finish_mock_invocation_frame,
    push_context,
    pop_context,
    close_syscall_handler,
    mock_syscall_entrypoint,
    root_invoke,
    try_commit_transaction,
    abort_transaction,
    enable_syscall_logging,
    verify_signed_transaction_auth,
    verify_signed_transaction_auth_from_db,
    do_verify_contract,
    get_rv32_stack_usage,
};

mod impls;
pub use impls::FFIErrorCode;
pub use impls::FFILogLevel;
