use crate::exn::syscall_throw;
use crate::sys::SyscallNo;
use crate::transaction::TransactionStatistic;
use crate::{GasAmount, sys};

/// The gas remaining in the current transaction.
///
/// Throws a syscall exception (aborting the frame) if the host call fails.
pub fn gas_remaining() -> GasAmount {
    let mut n: u64 = 0;

    sys::raw_GetTransactionStatistics(
        TransactionStatistic::GasRemaining,
        &mut n,
    )
    .unwrap_or_else(|e| {
        syscall_throw(SyscallNo::kGetTransactionStatistics, e)
    });

    n
}
