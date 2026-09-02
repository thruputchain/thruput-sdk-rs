//! The block a transaction executes in -- its height, timepoint, and
//! network -- and arithmetic on [`BlockHeight`].
//!
//! [`BlockCount`] is to [`BlockHeight`] what
//! [`Duration`](crate::time::Duration) is to [`TimePoint`]:
//! `height + BlockCount::new(100)` is a later height, and
//! [`later.blocks_since(&earlier)`](BlockHeightExt::blocks_since) the count
//! between them. Like the time arithmetic, it panics on overflow in every
//! profile rather than wrapping.

use core::iter::Sum;
use core::ops::{Add, AddAssign, Deref, Div, Mul, Neg, Sub, SubAssign};

use crate::exn::syscall_throw;
use crate::sys::{self, SyscallNo};
use crate::xdr::{ConstXDRSize, ReadXDR, WriteXDR};
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

/// A number of blocks: the difference between two [`BlockHeight`]s.
///
/// Signed, as heights are, so `earlier - later` is negative rather than a
/// panic. On the wire and in storage it is one `i64`, so it can be a
/// `#[contract_type]` field, a method argument, or a
/// [`Value`](crate::storage::Value).
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    ReadXDR,
    WriteXDR,
    ConstXDRSize,
)]
#[repr(transparent)]
pub struct BlockCount {
    inner: i64,
}

impl BlockCount {
    /// No blocks.
    pub const ZERO: BlockCount = BlockCount::new(0);

    /// A count of `blocks` blocks.
    pub const fn new(blocks: i64) -> BlockCount {
        BlockCount { inner: blocks }
    }

    /// The count as a plain integer.
    pub const fn into_inner(self) -> i64 {
        self.inner
    }

    /// `self + rhs`, or `None` on overflow.
    pub const fn checked_add(self, rhs: BlockCount) -> Option<BlockCount> {
        match self.inner.checked_add(rhs.inner) {
            Some(inner) => Some(BlockCount { inner }),
            None => None,
        }
    }

    /// `self - rhs`, or `None` on overflow.
    pub const fn checked_sub(self, rhs: BlockCount) -> Option<BlockCount> {
        match self.inner.checked_sub(rhs.inner) {
            Some(inner) => Some(BlockCount { inner }),
            None => None,
        }
    }

    /// `self * rhs`, or `None` on overflow.
    pub const fn checked_mul(self, rhs: i64) -> Option<BlockCount> {
        match self.inner.checked_mul(rhs) {
            Some(inner) => Some(BlockCount { inner }),
            None => None,
        }
    }
}

impl From<i64> for BlockCount {
    fn from(blocks: i64) -> Self {
        BlockCount::new(blocks)
    }
}

impl From<BlockCount> for i64 {
    fn from(count: BlockCount) -> Self {
        count.inner
    }
}

impl Deref for BlockCount {
    type Target = i64;

    fn deref(&self) -> &i64 {
        &self.inner
    }
}

// The spec sees the plain integer, exactly as it sees `BlockHeight`.
impl crate::__contract_spec::ContractTypeRefSpec for BlockCount {
    const INFO: crate::__contract_spec::ContractTypeInfo<'static> =
        crate::__contract_spec::ContractTypeInfo {
            ty: crate::__contract_spec::ContractTypeView::I64,
        };
}

impl Add for BlockCount {
    type Output = BlockCount;

    fn add(self, rhs: BlockCount) -> BlockCount {
        match self.checked_add(rhs) {
            Some(sum) => sum,
            None => panic!("overflow when adding block counts"),
        }
    }
}

impl AddAssign for BlockCount {
    fn add_assign(&mut self, rhs: BlockCount) {
        *self = *self + rhs;
    }
}

impl Sub for BlockCount {
    type Output = BlockCount;

    fn sub(self, rhs: BlockCount) -> BlockCount {
        match self.checked_sub(rhs) {
            Some(difference) => difference,
            None => panic!("overflow when subtracting block counts"),
        }
    }
}

impl SubAssign for BlockCount {
    fn sub_assign(&mut self, rhs: BlockCount) {
        *self = *self - rhs;
    }
}

impl Mul<i64> for BlockCount {
    type Output = BlockCount;

    fn mul(self, rhs: i64) -> BlockCount {
        match self.checked_mul(rhs) {
            Some(product) => product,
            None => panic!("overflow when multiplying block count by scalar"),
        }
    }
}

impl Mul<BlockCount> for i64 {
    type Output = BlockCount;

    fn mul(self, rhs: BlockCount) -> BlockCount {
        rhs * self
    }
}

impl Div<i64> for BlockCount {
    type Output = BlockCount;

    fn div(self, rhs: i64) -> BlockCount {
        match self.inner.checked_div(rhs) {
            Some(inner) => BlockCount { inner },
            None => panic!("overflow when dividing block count by scalar"),
        }
    }
}

impl Neg for BlockCount {
    type Output = BlockCount;

    fn neg(self) -> BlockCount {
        match self.inner.checked_neg() {
            Some(inner) => BlockCount { inner },
            None => panic!("overflow when negating block count"),
        }
    }
}

impl Sum for BlockCount {
    fn sum<I: Iterator<Item = BlockCount>>(iter: I) -> BlockCount {
        iter.fold(BlockCount::ZERO, Add::add)
    }
}

impl<'a> Sum<&'a BlockCount> for BlockCount {
    fn sum<I: Iterator<Item = &'a BlockCount>>(iter: I) -> BlockCount {
        iter.copied().sum()
    }
}

mod sealed {
    pub trait Sealed {}
    impl Sealed for super::BlockHeight {}
}

/// Checked arithmetic on [`BlockHeight`], alongside the `+` and `-`
/// operators.
///
/// The operator forms panic where these return `None`. This trait is sealed:
/// it exists to give the wire type methods, not to be implemented elsewhere.
pub trait BlockHeightExt: sealed::Sealed {
    /// `self + count`, or `None` on overflow.
    fn checked_add(&self, count: BlockCount) -> Option<BlockHeight>;

    /// `self - count`, or `None` on overflow.
    fn checked_sub(&self, count: BlockCount) -> Option<BlockHeight>;

    /// The blocks from `earlier` to `self`, negative if `earlier` is in fact
    /// later.
    ///
    /// Panics if the difference overflows `i64`.
    fn blocks_since(&self, earlier: &BlockHeight) -> BlockCount;
}

impl BlockHeightExt for BlockHeight {
    fn checked_add(&self, count: BlockCount) -> Option<BlockHeight> {
        (**self).checked_add(count.inner).map(BlockHeight::new)
    }

    fn checked_sub(&self, count: BlockCount) -> Option<BlockHeight> {
        (**self).checked_sub(count.inner).map(BlockHeight::new)
    }

    fn blocks_since(&self, earlier: &BlockHeight) -> BlockCount {
        match (**self).checked_sub(**earlier) {
            Some(blocks) => BlockCount::new(blocks),
            None => panic!("overflow when subtracting block heights"),
        }
    }
}

impl Add<BlockCount> for &BlockHeight {
    type Output = BlockHeight;

    fn add(self, rhs: BlockCount) -> BlockHeight {
        match self.checked_add(rhs) {
            Some(later) => later,
            None => panic!("overflow when adding block count to height"),
        }
    }
}

impl Add<BlockCount> for BlockHeight {
    type Output = BlockHeight;

    fn add(self, rhs: BlockCount) -> BlockHeight {
        &self + rhs
    }
}

impl AddAssign<BlockCount> for BlockHeight {
    fn add_assign(&mut self, rhs: BlockCount) {
        *self = &*self + rhs;
    }
}

impl Sub<BlockCount> for &BlockHeight {
    type Output = BlockHeight;

    fn sub(self, rhs: BlockCount) -> BlockHeight {
        match self.checked_sub(rhs) {
            Some(earlier) => earlier,
            None => {
                panic!("overflow when subtracting block count from height")
            }
        }
    }
}

impl Sub<BlockCount> for BlockHeight {
    type Output = BlockHeight;

    fn sub(self, rhs: BlockCount) -> BlockHeight {
        &self - rhs
    }
}

impl SubAssign<BlockCount> for BlockHeight {
    fn sub_assign(&mut self, rhs: BlockCount) {
        *self = &*self - rhs;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::internal::encode_xdr;

    fn height(h: i64) -> BlockHeight {
        BlockHeight::new(h)
    }

    #[test]
    fn height_plus_and_minus_count() {
        let now = height(1_000);
        let expiry = &now + BlockCount::new(100);
        assert_eq!(expiry, height(1_100));
        assert!(now < expiry);
        assert_eq!(&expiry - BlockCount::new(100), now);
        assert_eq!(expiry.clone() - BlockCount::new(100), now);

        let mut moving = now.clone();
        moving += BlockCount::new(10);
        moving -= BlockCount::new(3);
        assert_eq!(moving, height(1_007));
    }

    #[test]
    fn height_difference_is_a_signed_count() {
        let earlier = height(100);
        let later = height(250);
        assert_eq!(later.blocks_since(&earlier), BlockCount::new(150));
        assert_eq!(earlier.blocks_since(&later), BlockCount::new(-150));
        assert_eq!(*earlier.blocks_since(&later), -150);
    }

    #[test]
    fn height_checked_forms() {
        let top = height(i64::MAX);
        assert_eq!(top.checked_add(BlockCount::new(1)), None);
        assert_eq!(
            top.checked_sub(BlockCount::new(1)),
            Some(height(i64::MAX - 1))
        );
        assert_eq!(height(i64::MIN).checked_sub(BlockCount::new(1)), None);
    }

    #[test]
    #[should_panic(expected = "overflow when adding block count to height")]
    fn height_add_overflow_panics() {
        let _ = height(i64::MAX) + BlockCount::new(1);
    }

    #[test]
    #[should_panic(expected = "overflow when subtracting block heights")]
    fn height_difference_overflow_panics() {
        let _ = height(i64::MAX).blocks_since(&height(-1));
    }

    #[test]
    fn count_arithmetic() {
        let c = BlockCount::new(30);
        assert_eq!(c + BlockCount::new(12), BlockCount::new(42));
        assert_eq!(c - BlockCount::new(42), BlockCount::new(-12));
        assert_eq!(c * 2, BlockCount::new(60));
        assert_eq!(2 * c, BlockCount::new(60));
        assert_eq!(c / 4, BlockCount::new(7));
        assert_eq!(-c, BlockCount::new(-30));
        assert_eq!(c.into_inner(), 30);
        assert_eq!(i64::from(c), 30);
        assert_eq!(BlockCount::from(30), c);
        let total: BlockCount = [c, c].iter().sum();
        assert_eq!(total, BlockCount::new(60));
        assert_eq!(BlockCount::new(i64::MAX).checked_add(c), None);
        assert_eq!(BlockCount::new(i64::MIN).checked_sub(c), None);
        assert_eq!(BlockCount::new(i64::MAX).checked_mul(2), None);
    }

    #[test]
    #[should_panic(expected = "overflow when adding block counts")]
    fn count_add_overflow_panics() {
        let _ = BlockCount::new(i64::MAX) + BlockCount::new(1);
    }

    #[test]
    fn encodes_as_one_i64_like_height() {
        let c = BlockCount::new(-7);
        let encoded = encode_xdr(&c).unwrap();
        assert_eq!(&encoded[..], &encode_xdr(&height(-7)).unwrap()[..]);

        let mut reader = xdrs::cxs::buf_reader::BufReader::new(&encoded[..]);
        assert_eq!(BlockCount::read_xdr(&mut reader).unwrap(), c);
    }
}
