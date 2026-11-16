use core::hint::black_box;
use thruput_sdk::*;

pub struct MemIntrinsics;

/// Bytes in each scratch buffer.
pub const BUF_LEN: usize = 512;

/// What a destination byte holds before an operation writes it.
pub const GUARD: u8 = 0xA5;

/// The byte at `index` of a source buffer. Neighbouring bytes differ, and
/// the sequence does not repeat within a buffer, so a copy that lands at the
/// wrong offset changes the digest.
pub const fn pattern(index: usize) -> u8 {
    ((index as u32).wrapping_mul(0x9E37_79B1) >> 13) as u8
}

/// FNV-1a over `bytes`.
pub fn digest(bytes: &[u8]) -> u64 {
    let mut hash = 0xCBF2_9CE4_8422_2325u64;
    for &byte in bytes {
        hash = (hash ^ byte as u64).wrapping_mul(0x0000_0100_0000_01B3);
    }
    hash
}

fn patterned() -> [u8; BUF_LEN] {
    let mut buf = [0u8; BUF_LEN];
    let mut i = 0;
    while i < BUF_LEN {
        buf[i] = pattern(i);
        i += 1;
    }
    buf
}

// Each method does the same work whatever `len` is, apart from the one call
// under test: it fills and digests whole buffers. The difference in gas
// between two lengths is therefore the cost of the extra bytes alone.
//
// `black_box` keeps each length and offset opaque. LLVM then emits a call
// to the runtime routine, where a known length would become inline loads
// and stores.
#[contract(client = MemIntrinsicsClient)]
impl MemIntrinsics {
    /// Copies `len` bytes from offset `src` of a patterned buffer to offset
    /// `dst` of a guard-filled one, with `memcpy`. Returns the digest of the
    /// destination buffer.
    pub fn copy(len: u32, src: u32, dst: u32) -> u64 {
        let (len, src, dst) = (len as usize, src as usize, dst as usize);
        let from = black_box(patterned());
        let mut to = black_box([GUARD; BUF_LEN]);
        to[dst..dst + len].copy_from_slice(&from[src..src + black_box(len)]);
        digest(black_box(&to))
    }

    /// Moves `len` bytes from offset `src` to offset `dst` of one patterned
    /// buffer, with `memmove`. Returns the digest of the buffer.
    pub fn shift(len: u32, src: u32, dst: u32) -> u64 {
        let (len, src, dst) = (len as usize, src as usize, dst as usize);
        let mut buf = black_box(patterned());
        buf.copy_within(src..src + black_box(len), dst);
        digest(black_box(&buf))
    }

    /// Sets `len` bytes at offset `dst` of a guard-filled buffer to `byte`,
    /// with `memset`. Returns the digest of the buffer.
    pub fn fill(len: u32, dst: u32, byte: u32) -> u64 {
        let (len, dst) = (len as usize, dst as usize);
        let mut buf = black_box([GUARD; BUF_LEN]);
        buf[dst..dst + black_box(len)].fill(byte as u8);
        digest(black_box(&buf))
    }
}
