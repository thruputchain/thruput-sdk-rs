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

    /// Fills buffer `a` (`len_a` bytes), then buffer `b` (`len_b` bytes)
    /// above it, then grows `a` by `extra` bytes. `a` is no longer the most
    /// recent allocation, so growing it copies it.
    ///
    /// Checks every byte of both buffers and traps on a mismatch. Returns
    /// `a`'s final length.
    pub fn grow_below(len_a: u32, len_b: u32, extra: u32) -> u32 {
        let mut a = filled(len_a, 3);
        let b = filled(len_b, 5);
        a.reserve_exact(extra as usize);
        for i in len_a..len_a + extra {
            a.push(pattern(i, 3));
        }
        let (a, b) = (black_box(a), black_box(b));
        check(&a, 3);
        check(&b, 5);
        a.len() as u32
    }

    /// Fills a `len`-byte buffer, asks to grow it by `extra` bytes with
    /// `try_reserve_exact`, and returns whether that failed. With `pinned`,
    /// a small allocation sits above the buffer, so growth takes the copying
    /// path; without it, growth is attempted in place.
    ///
    /// Either way, the buffer must stay usable. Traps if its contents
    /// change, if it cannot be rewritten, or if a later allocation overlaps
    /// it.
    pub fn grow_fail(len: u32, extra: u32, pinned: bool) -> bool {
        let mut buf = filled(len, 7);
        let pin = pinned.then(|| filled(16, 11));
        let failed = buf.try_reserve_exact(extra as usize).is_err();
        let mut buf = black_box(buf);
        check(&buf, 7);

        // Rewrite the buffer, then allocate after it: the new allocation
        // must not overlap the buffer.
        for (i, byte) in buf.iter_mut().enumerate() {
            *byte = pattern(i as u32, 13);
        }
        let after = black_box(filled(64, 17));
        check(&buf, 13);
        check(&after, 17);
        if let Some(pin) = pin {
            check(&black_box(pin), 11);
        }
        failed
    }
}

/// Byte `i` of a buffer filled with `seed`. Distinct seeds give buffers
/// distinct contents, so a copy from the wrong place shows.
fn pattern(i: u32, seed: u8) -> u8 {
    (i as u8)
        .wrapping_mul(seed)
        .wrapping_add(seed ^ (i >> 8) as u8)
}

fn filled(len: u32, seed: u8) -> Vec<u8> {
    let mut buf = Vec::with_capacity(len as usize);
    for i in 0..len {
        buf.push(pattern(i, seed));
    }
    buf
}

/// Traps unless `buf` holds `pattern(i, seed)` at every index.
fn check(buf: &[u8], seed: u8) {
    for (i, &byte) in buf.iter().enumerate() {
        if byte != pattern(i as u32, seed) {
            panic!("buffer contents changed");
        }
    }
}
