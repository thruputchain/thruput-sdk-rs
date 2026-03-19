use crate::transaction::TransactionStatistic;
use crate::{GasAmount, sys};

pub fn gas_remaining() -> GasAmount {
    let mut n: u64 = 0;

    sys::raw_GetTransactionStatistics(
        TransactionStatistic::GasRemaining,
        &mut n,
    )
    .unwrap_or_else(|e| panic!("GetTransactionStatistics failed: {}", e));

    n
}
