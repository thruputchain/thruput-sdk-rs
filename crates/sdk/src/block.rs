use crate::exn::syscall_throw;
use crate::sys::{self, SyscallNo};
use groundhog_xdr::types::{BlockHeight, NetworkHash, SHA256Hash, TimePoint};

/// The height of the block the current transaction executes in.
///
/// Throws a syscall exception (aborting the frame) if the host call fails.
pub fn get_block_height() -> BlockHeight {
    let mut block: BlockHeight = 0i64.into();
    sys::raw_GetBlockHeight(&mut block)
        .unwrap_or_else(|e| syscall_throw(SyscallNo::kGetBlockHeight, e));
    block
}

/// The hash of the network name -- the identifier separating this network's
/// blocks from every other network's. Constant for the life of the network.
///
/// Transaction signatures already commit to this value, so a contract can bind
/// its own off-chain signed messages to this network the same way, and a
/// message signed for one network cannot be replayed against a contract at the
/// same address on another.
///
/// Throws a syscall exception (aborting the frame) if the host call fails.
pub fn get_network_hash() -> NetworkHash {
    let mut network_hash: NetworkHash = SHA256Hash::default().into();
    sys::raw_GetNetworkHash(&mut network_hash)
        .unwrap_or_else(|e| syscall_throw(SyscallNo::kGetNetworkHash, e));
    network_hash
}

/// The timepoint of the block the current transaction executes in, as Unix
/// nanoseconds.
///
/// Fixed by the coordinator before execution begins and carried in the block's
/// header, so every node executing this block sees the same value. It is not
/// the time this call is made.
///
/// Throws a syscall exception (aborting the frame) if the host call fails.
pub fn get_block_timepoint() -> TimePoint {
    let mut timepoint: TimePoint = 0u64.into();
    sys::raw_GetBlockTimepoint(&mut timepoint)
        .unwrap_or_else(|e| syscall_throw(SyscallNo::kGetBlockTimepoint, e));
    timepoint
}
