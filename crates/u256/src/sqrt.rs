//! Integer square root.
//!
//! A contract takes it from the runtime: `U256Sqrt` is a host operation with
//! a flat price, where the routine below costs a contract tens of thousands
//! of gas and about two kilobytes of text. Every other target computes it
//! here.

use crate::U256;

impl U256 {
    /// The largest `r` such that `r * r <= self`.
    ///
    /// On `riscv32`, which is to say in a contract, this is the runtime's
    /// experimental `U256Sqrt` host operation. It needs a runtime that
    /// implements syscall API 1.2. On any other target the root is computed
    /// in software. The two return the same root for every value.
    ///
    /// Not a `const fn`, unlike the rest of this type: a host operation
    /// cannot run at compile time.
    ///
    /// # Panics
    ///
    /// In a contract, if the host operation fails.
    pub fn integer_sqrt(self) -> Self {
        #[cfg(target_arch = "riscv32")]
        {
            host_sqrt(self)
        }
        #[cfg(not(target_arch = "riscv32"))]
        {
            software_sqrt(self)
        }
    }
}

/// The root, from the runtime.
#[cfg(target_arch = "riscv32")]
fn host_sqrt(value: U256) -> U256 {
    match thruput_xdr::syscalls::U256Sqrt(&crate::host::le_bytes(value)) {
        Ok(root) => U256::from_u128(root),
        Err(_) => panic!("U256Sqrt failed"),
    }
}

/// The root, digit by digit (binary restoring): shifts, comparisons and
/// subtractions, no division. It was written to be small in a contract, and
/// now runs only where no runtime is there to ask.
#[cfg(not(target_arch = "riscv32"))]
const fn software_sqrt(value: U256) -> U256 {
    let len = value.bit_len();
    if len <= 1 {
        // 0 and 1 are their own square roots.
        return value;
    }

    let mut x = value;
    let mut res = U256::ZERO;

    // The largest power of four not exceeding `value`: `1 << 2k` for the
    // largest even `2k <= bit_len - 1`.
    let mut bit = U256::ONE.wrapping_shl((len - 1) & !1);

    while !bit.is_zero() {
        let t = res.wrapping_add(bit);
        if x.const_ge(t) {
            x = x.wrapping_sub(t);
            res = res.wrapping_shr(1).wrapping_add(bit);
        } else {
            res = res.wrapping_shr(1);
        }
        bit = bit.wrapping_shr(2);
    }

    res
}
