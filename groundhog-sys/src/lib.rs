//! # groundhog-sys
//!
//! Low-level Rust FFI bindings to the `hogd` test harness runtime.
//!
//! This crate is an internal implementation detail exposing C bindings and constants needed for the
//! higher-level `groundhog` Rust crate. The contents are auto-generated from the `hogd` C headers via bindgen.

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
    ForeignRootInvokeResult,
    ForeignEntryPoint,
    ForeignHostFnResult,
    ForeignTryCommitResult,
    CreateDBResult,
    OpenRuntimeContextResult,
    FFISerializedSignedTransaction,
    OpenSyscallHandlerResult,
    GetEventLogLenResult,
    FFISourceAuthData,
    FFISerializedAuthData,
    FFIAuxiliaryAuthData,
    VerifyAuthResult,
    VerifyAuthResultFromDBResult,
    FFIDataBuffer,
    GetCostModelResult,
};

// Functions
#[rustfmt::skip]
pub use bindings::{
    create_db,
    free_db,
    get_cost_model,
    advance_new_block,
    open_runtime_context,
    close_runtime_context,
    register_new_contract,
    setup_create_contract,
    setup_deploy_contract,
    setup_grant_contract_storage_tokens,
    open_syscall_handler,
    close_syscall_handler,
    get_event_log,
    get_event_log_len,
    mock_syscall_entrypoint,
    root_invoke,
    try_commit_transaction,
    abort_transaction,
    enable_syscall_logging,
    verify_signed_transaction_auth,
    verify_signed_transaction_auth_from_db,
};

mod impls;
pub use impls::FFIErrorCode;
