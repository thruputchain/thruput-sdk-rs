use crate::transaction::kTxStatGasRemaining;
use crate::{GasAmount, contract_panic, sys};

pub fn gas_remaining() -> GasAmount {
    let mut n: u64 = 0;

    sys::raw_GetTransactionStatistics(kTxStatGasRemaining, &mut n)
        .unwrap_or_else(|e| {
            contract_panic!("GetTransactionStatistics failed: {}", e)
        });

    n
}
