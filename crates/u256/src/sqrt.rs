//! Integer square root.

use crate::U256;

impl U256 {
    /// The largest `r` such that `r * r <= self`.
    ///
    /// This is the digit-by-digit (binary restoring) method: walk `bit` down
    /// the even powers of two and at each step test whether the running
    /// remainder can absorb `res + bit`. Only shifts, comparisons and
    /// subtractions -- no division.
    ///
    /// Newton's method is the obvious alternative and is much worse here: its
    /// iteration step is `(x + n / x) >> 1`, a full 256-bit division, which
    /// LLVM then inlines into the sqrt loop. `uint`'s Newton-based
    /// `integer_sqrt` measured 12,732 bytes of rv32 text -- roughly 19% of the
    /// artifact format's 64 KiB code cap, for a square root.
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
        if a.0[i] != b.0[i] {
            return a.0[i] > b.0[i];
        }
    }
    true
}
