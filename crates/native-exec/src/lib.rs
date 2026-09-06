//! Native execution environment for testing Groundhog contracts in-process.
//!
//! The entry point is [`test_environment::TestEnvironment`], an in-memory
//! groundhog instance against which tests run mocked transactions and
//! assert on their results.
#![warn(missing_docs)]

/// Report a failure detected inside a `Drop` impl without risking a SIGABRT.
///
/// Panicking while the thread is *already* unwinding (e.g. a test assertion
/// just failed) aborts the whole process, replacing the original failure
/// message with an opaque SIGABRT. So when a panic is already in flight we log
/// to stderr instead; otherwise we panic normally, since a Drop-time invariant
/// violation with no prior panic is a real bug worth surfacing loudly.
#[track_caller]
pub(crate) fn drop_failure(args: core::fmt::Arguments<'_>) {
    if std::thread::panicking() {
        eprintln!(
            "[native-exec] error during Drop suppressed to preserve the \
             in-flight panic: {args}"
        );
    } else {
        panic!("{args}");
    }
}

/// Fluent builders for configuring and executing invocation frames.
pub mod frame_builder;
/// Per-frame results (status, buffers, trace, events) and their assertions.
pub mod frame_result;
/// The mocked syscall entrypoint bridging guest syscalls to the test FFI.
pub mod host_fn_error;
/// Panic payloads and the panic hook that classify frame unwinds.
pub mod invoke_error;
pub use host_fn_error::perform_mocked_syscall;
/// Mock contracts: native closures registered as contract entrypoints.
pub mod mock_contract;
/// The mocked transaction handler and its invocation-frame state machine.
pub mod mock_transaction_handler;
/// The chain's execution parameters, and a builder for non-default ones.
pub mod network_config;
/// The in-process groundhog test environment.
pub mod test_environment;
/// Optional builder for a test environment with non-default settings.
pub mod test_environment_builder;
/// Small conveniences for writing tests (deterministic named addresses).
pub mod test_helpers;
/// Whole-transaction results (root frame + commit outcome) and assertions.
pub mod transact_result;
/// Builder for configuring a mocked transaction before executing it.
pub mod transaction_builder;
/// Handler for executing full signed transactions against the environment.
pub mod transaction_handler;
/// Typed outcome of a two-phase transaction commit.
pub mod try_commit_result;
pub use try_commit_result::TryCommitResult;

#[cfg(target_arch = "riscv32")]
compile_error!("The `native_exec` crate cannot be compiled for rv32.");
