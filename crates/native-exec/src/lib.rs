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
pub mod try_commit_result;
pub use try_commit_result::TryCommitResult;

#[cfg(target_family = "wasm")]
compile_error!("The `native_exec` crate cannot be compiled for wasm.");
