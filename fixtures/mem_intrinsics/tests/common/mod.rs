//! A byte-at-a-time model of each `MemIntrinsics` method. It shares the
//! contract's pattern and digest, and none of its copy or fill code.

#![allow(unused)]
// The loops are the model: clippy's replacement would call the routines
// under test.
#![allow(clippy::manual_memcpy)]

use mem_intrinsics::{BUF_LEN, GUARD, digest, pattern};

fn patterned() -> Vec<u8> {
    (0..BUF_LEN).map(pattern).collect()
}

/// What `MemIntrinsics::copy` returns.
pub fn copy(len: u32, src: u32, dst: u32) -> u64 {
    let (len, src, dst) = (len as usize, src as usize, dst as usize);
    let from = patterned();
    let mut to = vec![GUARD; BUF_LEN];
    for i in 0..len {
        to[dst + i] = from[src + i];
    }
    digest(&to)
}

/// What `MemIntrinsics::shift` returns: every byte read is the byte the
/// buffer held before the move.
pub fn shift(len: u32, src: u32, dst: u32) -> u64 {
    let (len, src, dst) = (len as usize, src as usize, dst as usize);
    let before = patterned();
    let mut after = before.clone();
    for i in 0..len {
        after[dst + i] = before[src + i];
    }
    digest(&after)
}

/// What `MemIntrinsics::fill` returns.
pub fn fill(len: u32, dst: u32, byte: u32) -> u64 {
    let (len, dst) = (len as usize, dst as usize);
    let mut buf = vec![GUARD; BUF_LEN];
    for slot in &mut buf[dst..dst + len] {
        *slot = byte as u8;
    }
    digest(&buf)
}

/// Lengths that cross every boundary the runtime routines branch on: empty,
/// bytes only, each word count below a 32-byte block, and several blocks
/// with each remainder.
pub fn lengths() -> impl Iterator<Item = u32> {
    (0..=100).chain([127, 128, 129, 255, 256, 257, 400])
}
