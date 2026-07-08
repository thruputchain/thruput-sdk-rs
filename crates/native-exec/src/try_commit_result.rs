use groundhog_sys::FFIDataBuffer;
use groundhog_xdr::exec_result::ExecResult;

// Re-exported so callers matching on `TxNonDeterministicError` have the
// conflict types in scope without depending on groundhog-trace directly.
pub use groundhog_trace::db_conflict::{
    DbConflictCode, DbConflictDiagnostic, DbConflictResult,
};

/// Outcome of a two-phase commit (`try_commit_transaction`).
///
/// Mirrors the codes of [`ExecResult`], one variant per code, so a commit
/// outcome is matched exhaustively. Only `ExecResultTxNonDeterministicError`
/// carries a two-phase-commit conflict diagnostic, so the [`DbConflictResult`]
/// rides on that variant alone — there is no way to reach a conflict after a
/// successful (or otherwise-failed) commit.
///
/// [`ExecResult`]: groundhog_xdr::exec_result::ExecResult
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TryCommitResult {
    /// `ExecResultOK` — the commit succeeded.
    Ok,
    /// `ExecResultTxDeterministicError` — the root frame returned a
    /// deterministic error.
    TxDeterministicError,
    /// `ExecResultTxRootFrameReverted` — the root frame chose to revert.
    TxRootFrameReverted,
    /// `ExecResultTxNonDeterministicError` — nondeterminism during the
    /// post-execution two-phase commit (a storage conflict). Carries the
    /// conflict diagnostic; `DbConflictResult::kDbDiagnosticNone` when the
    /// database reported no per-conflict detail for this execution mode.
    TxNonDeterministicError(DbConflictResult),
    /// `ExecResultFailedGasSponsorship` — no longer produced; retained to
    /// mirror [`ExecResult`].
    FailedGasSponsorship,
    /// `ExecResultTxValidationFailure` — attached validation data was bad.
    TxValidationFailure,
    /// `ExecResultSystemError` — recoverable system error (e.g. broken
    /// connection).
    SystemError,
    /// `ExecResultUnrecoverable` — critical, unrecoverable executor failure.
    Unrecoverable,
}

impl TryCommitResult {
    /// Build from the raw `try_commit_transaction` FFI result. The conflict
    /// buffer is only decoded for the nondeterministic arm; every other code
    /// ignores it.
    pub(crate) fn from_ffi(
        result_code: i32,
        serialized_conflict: &FFIDataBuffer,
    ) -> Self {
        match result_code {
            x if x == ExecResult::ExecResultOK.0 => Self::Ok,
            x if x == ExecResult::ExecResultTxDeterministicError.0 => {
                Self::TxDeterministicError
            },
            x if x == ExecResult::ExecResultTxRootFrameReverted.0 => {
                Self::TxRootFrameReverted
            },
            x if x == ExecResult::ExecResultTxNonDeterministicError.0 => {
                Self::TxNonDeterministicError(decode_conflict(
                    serialized_conflict,
                ))
            },
            x if x == ExecResult::ExecResultFailedGasSponsorship.0 => {
                Self::FailedGasSponsorship
            },
            x if x == ExecResult::ExecResultTxValidationFailure.0 => {
                Self::TxValidationFailure
            },
            x if x == ExecResult::ExecResultSystemError.0 => Self::SystemError,
            x if x == ExecResult::ExecResultUnrecoverable.0 => {
                Self::Unrecoverable
            },
            other => panic!(
                "try_commit_transaction returned unknown ExecResult code: {other}"
            ),
        }
    }

    /// The [`ExecResult`] code this outcome corresponds to, discarding any
    /// conflict diagnostic.
    pub fn exec_result(&self) -> ExecResult {
        match self {
            Self::Ok => ExecResult::ExecResultOK,
            Self::TxDeterministicError => {
                ExecResult::ExecResultTxDeterministicError
            },
            Self::TxRootFrameReverted => {
                ExecResult::ExecResultTxRootFrameReverted
            },
            Self::TxNonDeterministicError(_) => {
                ExecResult::ExecResultTxNonDeterministicError
            },
            Self::FailedGasSponsorship => {
                ExecResult::ExecResultFailedGasSponsorship
            },
            Self::TxValidationFailure => {
                ExecResult::ExecResultTxValidationFailure
            },
            Self::SystemError => ExecResult::ExecResultSystemError,
            Self::Unrecoverable => ExecResult::ExecResultUnrecoverable,
        }
    }
}

/// Decode the XDR-serialized [`DbConflictResult`] attached to a
/// nondeterministic commit failure.
fn decode_conflict(serialized_conflict: &FFIDataBuffer) -> DbConflictResult {
    match serialized_conflict.as_slice() {
        Some(bytes) => xdrs::txs::decode(bytes).unwrap_or_else(|e| {
            panic!(
                "failed to decode DbConflictResult ({} bytes): {e}",
                bytes.len()
            )
        }),
        None => panic!(
            "try_commit_transaction reported a nondeterministic error but the \
             serialized conflict buffer was null (serialization failed)"
        ),
    }
}
