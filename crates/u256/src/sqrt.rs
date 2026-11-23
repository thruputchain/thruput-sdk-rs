//! Integer square root.

use crate::U256;

impl U256 {
    /// The largest `r` such that `r * r <= self`: the runtime's
    /// experimental `U256Sqrt` host operation, which needs syscall API 1.2.
    ///
    /// # Panics
    ///
    /// If the host operation fails.
    pub fn integer_sqrt(self) -> Self {
        match thruput_xdr::syscalls::U256Sqrt(crate::host::bytes(&self)) {
            Ok(root) => Self::from_u128(root),
            Err(_) => panic!("U256Sqrt failed"),
        }
    }
}
