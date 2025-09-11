use groundhog_xdr::transaction::kTxStatGasRemaining;
use crate::sys;
use crate::contract_panic;

pub fn gas_remaining() -> u64 {
    let mut n: u64 = 0;

    sys::raw_GetTransactionStatistics(kTxStatGasRemaining, &mut n).unwrap_or_else(|e|
        contract_panic!("GetTransactionStatistics failed: {}", e));

    n
}
