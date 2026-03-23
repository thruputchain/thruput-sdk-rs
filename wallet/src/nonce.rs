use std::ops::AddAssign;

/// A nonce for distinguishing multiple accounts using the same public key.
#[derive(Copy, Clone)]
pub struct PubkeyAccountNonce(u64);

impl PubkeyAccountNonce {
    pub fn initial() -> Self {
        Self(0)
    }
}

impl From<PubkeyAccountNonce> for u64 {
    fn from(n: PubkeyAccountNonce) -> u64 {
        n.0
    }
}

impl AddAssign<u64> for PubkeyAccountNonce {
    fn add_assign(&mut self, other: u64) {
        self.0 += other
    }
}

/// A nonce for use as a transaction uniquifier
#[derive(Copy, Clone)]
pub struct UniquifierNonce(u64);

impl UniquifierNonce {
    pub fn initial() -> Self {
        Self(0)
    }
}

impl From<UniquifierNonce> for u64 {
    fn from(n: UniquifierNonce) -> u64 {
        n.0
    }
}

impl AddAssign<u64> for UniquifierNonce {
    fn add_assign(&mut self, other: u64) {
        self.0 += other
    }
}
