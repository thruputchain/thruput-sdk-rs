//! Integer square root.

use crate::U256;

impl U256 {
    /// The largest `r` such that `r * r <= self`. In a contract this is the
    /// runtime's experimental `U256Sqrt` host operation, which needs syscall
    /// API 1.2.
    ///
    /// # Panics
    ///
    /// In a contract, if the host operation fails.
    pub fn integer_sqrt(self) -> Self {
        Self::from_u128(crate::host::sqrt(&self))
    }
}
