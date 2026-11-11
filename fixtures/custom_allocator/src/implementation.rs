use alloc::vec::Vec;
use core::hint::black_box;
use thruput_sdk::*;

pub struct CustomAllocator;

#[contract(client = CustomAllocatorClient)]
impl CustomAllocator {
    /// The guest heap size in bytes, or 0 in a native build.
    pub fn heap_bytes() -> u32 {
        #[cfg(target_arch = "riscv32")]
        {
            thruput_sdk::guest_heap().len() as u32
        }
        #[cfg(not(target_arch = "riscv32"))]
        {
            0
        }
    }

    /// Allocates, fills, and drops a `bytes`-long buffer `rounds` times.
    /// Each buffer is the most recent allocation when it is dropped.
    ///
    /// Returns the wrapping sum of every byte written.
    pub fn churn(rounds: u32, bytes: u32) -> u32 {
        let mut sum = 0u32;
        for round in 0..rounds {
            let mut buf = Vec::with_capacity(bytes as usize);
            for i in 0..bytes {
                buf.push(i.wrapping_add(round) as u8);
            }
            // Keep the allocation observable: LLVM may otherwise elide an
            // allocation whose contents it can compute.
            let buf = black_box(buf);
            sum = buf.iter().fold(sum, |s, &b| s.wrapping_add(b as u32));
        }
        sum
    }

    /// Grows a buffer one byte at a time to `len` bytes, reserving exactly
    /// one more byte before each push. Every push reallocates.
    ///
    /// Returns the wrapping sum of the bytes.
    pub fn grow(len: u32) -> u32 {
        let mut buf = Vec::new();
        for i in 0..len {
            buf.reserve_exact(1);
            buf.push(i as u8);
        }
        let buf = black_box(buf);
        buf.iter().fold(0u32, |s, &b| s.wrapping_add(b as u32))
    }
}
