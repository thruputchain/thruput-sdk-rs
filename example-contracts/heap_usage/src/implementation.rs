extern crate alloc;

use alloc::vec::Vec;
use sdk::*;

#[derive(Default)]
pub struct HeapUsage;

#[contract(client = HeapUsageClient)]
impl HeapUsage {
    /// Sum `0..count` through a heap-allocated `Vec<u64>`.
    ///
    /// The `Vec` costs `8 * count` bytes on the guest bump allocator;
    /// the native test asserts that shows up in the frame's alloc stats.
    pub fn sum_range(count: u32) -> u64 {
        let values: Vec<u64> = (0..count as u64).collect();
        values.iter().sum()
    }
}
