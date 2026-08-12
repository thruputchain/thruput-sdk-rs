//! Integer square root.

use crate::U256;

impl U256 {
    /// The largest `r` such that `r * r <= self`.
    ///
    /// Digit-by-digit (binary restoring): shifts, comparisons and subtractions,
    /// no division. Newton's method costs a full 256-bit divide per iteration
    /// that LLVM inlines into the loop -- `uint`'s Newton-based version
    /// measured 12,732 bytes of rv32 text, for a square root.
    pub const fn integer_sqrt(self) -> Self {
        let len = self.bit_len();
        if len <= 1 {
            // 0 and 1 are their own square roots.
            return self;
        }

        let mut x = self;
        let mut res = Self::ZERO;

        // The largest power of four not exceeding `self`: `1 << 2k` for the
        // largest even `2k <= bit_len - 1`.
        let mut bit = Self::ONE.wrapping_shl((len - 1) & !1);

        while !bit.is_zero() {
            let t = res.wrapping_add(bit);
            if const_ge(x, t) {
                x = x.wrapping_sub(t);
                res = res.wrapping_shr(1).wrapping_add(bit);
            } else {
                res = res.wrapping_shr(1);
            }
            bit = bit.wrapping_shr(2);
        }

        res
    }
}

/// `a >= b`, usable from `const fn` (the `PartialOrd` impl is not const).
#[inline]
const fn const_ge(a: U256, b: U256) -> bool {
    let mut i = crate::LIMBS;
    while i > 0 {
        i -= 1;
        if a.limbs[i] != b.limbs[i] {
            return a.limbs[i] > b.limbs[i];
        }
    }
    true
}
