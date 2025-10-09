use crate::contract_panic;
use crate::sys;
use crate::transaction::kTxStatGasRemaining;

pub fn gas_remaining() -> u64 {
    let mut n: u64 = 0;

    sys::raw_GetTransactionStatistics(kTxStatGasRemaining, &mut n)
        .unwrap_or_else(|e| {
            contract_panic!("GetTransactionStatistics failed: {}", e)
        });

    n
}
