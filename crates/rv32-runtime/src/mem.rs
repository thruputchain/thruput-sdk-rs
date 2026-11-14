//! Lean `memcpy`/`memmove`/`memset` for the hogd rv32 VM.
//!
//! compiler_builtins' generic versions are written for hardware where
//! unaligned word access traps: memcpy alone is ~500 B, ~340 B of which is a
//! shift-and-splice path for misaligned sources (plus a 16 B stack frame).
//! The hogd VM permits unaligned scalar loads/stores (the contract rustflags
//! set `-C target-feature=+unaligned-scalar-mem`), so the loops here use
//! `read_unaligned`/`write_unaligned` and need no alignment handling at all.
//! (Without `+unaligned-scalar-mem` — e.g. the publish-workflow rlib builds
//! that use prebuilt core and no contract rustflags — each unaligned word
//! access byte-expands: still correct, just not the fast path. The flag is
//! what makes these bodies a win; it is part of the canonical contract
//! rustflags.)
//!
//! These are strong definitions overriding compiler_builtins' weak symbols
//! at link time. Gas follows the instructions executed, so the forward copy
//! and the fill run eight words per iteration: 19 instructions per 32 bytes
//! to copy and 10 to fill, where one word per iteration takes 40 and 24.
//! The loop overhead is what unrolling removes; the loads and stores remain.
//! In the test executor a load costs 3 gas, a taken branch 2, and a store
//! or an addition 1, so a block costs 36 gas to copy and 11 to fill. The
//! `mem_intrinsics` fixture's rv32 test bounds both, and checks every
//! routine against a byte-at-a-time model.
//!
//! Sizes: `memcpy` 176 B, `memset` 128 B, `memmove` 264 B. A contract links
//! only the ones it calls.
//!
//! RECURSION GUARD: LLVM's loop-idiom recognition can lower copy/fill loops
//! back into `memcpy`/`memset` calls — inside these functions that would be
//! infinite recursion. The crate is `#![no_builtins]` (lib.rs) to forbid
//! that. CI asserts the override held per contract build (gnaw's
//! `elf::check_mem_symbols`: no WEAK mem symbol, no compiler_builtins mem
//! symbols); recursion itself was ruled out by disassembly when
//! this landed and no_builtins keeps it that way.

use core::mem::MaybeUninit;

// ---------------------------------------------------------------------------
// Pure copy/fill cores — target-independent so the overlap/seam logic can be
// unit-tested on the host (cf. dispatch.rs). `#[inline(always)]` so each rv32
// wrapper below compiles to exactly the same outlined body as writing the
// loop in the wrapper directly; the wrappers are the only exported symbols.
// ---------------------------------------------------------------------------

/// Bytes per iteration of the unrolled loops: eight words.
const BLOCK: usize = 32;

/// Copies the word at `off` from `s` to `d`, at any alignment.
#[inline(always)]
unsafe fn copy_word(
    d: *mut MaybeUninit<u8>,
    s: *const MaybeUninit<u8>,
    off: usize,
) {
    unsafe {
        // Copies must preserve uninitialized bytes, including struct padding.
        d.add(off).cast::<MaybeUninit<u32>>().write_unaligned(
            s.add(off).cast::<MaybeUninit<u32>>().read_unaligned(),
        );
    }
}

/// Strictly ascending copy: 32-byte blocks, then words, then bytes. The
/// ascending order is a load-bearing contract: `move_any` delegates
/// overlapping `dst < src` moves here, which is only correct because no
/// write overtakes a read. That holds inside a block too: each word is
/// loaded, then stored, lowest first. Any rewrite that reorders writes (a
/// tail copied early, a host-memcpy syscall with unspecified overlap
/// semantics) must first stop `move_any` from delegating overlaps here.
///
/// Each stage takes its bound from the pointer the stage before it left and
/// from `n`. Bounds derived from `dst` instead make LLVM carry a second
/// copy of the destination pointer through the loops, one more instruction
/// per iteration.
#[inline(always)]
unsafe fn copy_forward(
    dst: *mut MaybeUninit<u8>,
    src: *const MaybeUninit<u8>,
    n: usize,
) {
    unsafe {
        let mut d = dst;
        let mut s = src;
        let block_end = d.add(n & !(BLOCK - 1));
        while d < block_end {
            copy_word(d, s, 0);
            copy_word(d, s, 4);
            copy_word(d, s, 8);
            copy_word(d, s, 12);
            copy_word(d, s, 16);
            copy_word(d, s, 20);
            copy_word(d, s, 24);
            copy_word(d, s, 28);
            d = d.add(BLOCK);
            s = s.add(BLOCK);
        }
        let word_end = d.add(n & (BLOCK - 4));
        while d < word_end {
            copy_word(d, s, 0);
            d = d.add(4);
            s = s.add(4);
        }
        let end = d.add(n & 3);
        while d < end {
            *d = *s;
            d = d.add(1);
            s = s.add(1);
        }
    }
}

/// Strictly descending word-then-byte copy, for moves whose destination
/// overlaps the source from above. Not unrolled: no contract in this
/// repository reaches it, and the forward copy takes every other move.
/// Pointer-bump form (the pointers are the induction variables): the indexed
/// form costs a 6-insn word loop — LLVM recomputes dst+i and src+i every
/// iteration instead of decrementing the pointers.
#[inline(always)]
unsafe fn copy_backward(
    dst: *mut MaybeUninit<u8>,
    src: *const MaybeUninit<u8>,
    n: usize,
) {
    unsafe {
        let mut d = dst.add(n);
        let mut s = src.add(n);
        let word_stop = dst.add(n & 3);
        while d > word_stop {
            d = d.sub(4);
            s = s.sub(4);
            copy_word(d, s, 0);
        }
        while d > dst {
            d = d.sub(1);
            s = s.sub(1);
            *d = *s;
        }
    }
}

/// Overlap-safe copy: direction dispatch shared by `memmove` and the tests.
#[inline(always)]
unsafe fn move_any(
    dst: *mut MaybeUninit<u8>,
    src: *const MaybeUninit<u8>,
    n: usize,
) {
    unsafe {
        // The distance wraps when `dst` is below `src`, to a value no valid
        // length reaches. So this one comparison takes the forward copy for
        // every move it is safe for: a destination below the source
        // (ascending writes never overtake the reads), and a destination at
        // or past the source's end (disjoint).
        if (dst as usize).wrapping_sub(src as usize) >= n {
            copy_forward(dst, src, n);
        } else {
            // dst overlaps src from above: copy highest bytes first.
            copy_backward(dst, src, n);
        }
    }
}

/// Fill in 32-byte blocks, then words, then bytes; the fill byte is already
/// truncated by the caller. Bounds follow `copy_forward`.
#[inline(always)]
unsafe fn fill(dst: *mut u8, byte: u8, n: usize) {
    unsafe {
        let word = u32::from_ne_bytes([byte; 4]);
        let mut d = dst;
        let block_end = d.add(n & !(BLOCK - 1));
        while d < block_end {
            d.cast::<u32>().write_unaligned(word);
            d.add(4).cast::<u32>().write_unaligned(word);
            d.add(8).cast::<u32>().write_unaligned(word);
            d.add(12).cast::<u32>().write_unaligned(word);
            d.add(16).cast::<u32>().write_unaligned(word);
            d.add(20).cast::<u32>().write_unaligned(word);
            d.add(24).cast::<u32>().write_unaligned(word);
            d.add(28).cast::<u32>().write_unaligned(word);
            d = d.add(BLOCK);
        }
        let word_end = d.add(n & (BLOCK - 4));
        while d < word_end {
            d.cast::<u32>().write_unaligned(word);
            d = d.add(4);
        }
        let end = d.add(n & 3);
        while d < end {
            *d = byte;
            d = d.add(1);
        }
    }
}

// ---------------------------------------------------------------------------
// The exported strong symbols (rv32 contracts only).
// ---------------------------------------------------------------------------

// The signatures use `c_void` exactly as libc declares them: rustc (since
// nightly-2026-07) cross-checks definitions of the runtime mem symbols the
// standard library relies on and rejects mismatched pointer types.

/// Forward copy, eight words per iteration, any alignment.
/// C contract: overlapping ranges are the caller's UB; see `copy_forward` for
/// the internal ascending-order invariant `memmove` relies on.
#[cfg(target_arch = "riscv32")]
#[unsafe(no_mangle)]
unsafe extern "C" fn memcpy(
    dst: *mut core::ffi::c_void,
    src: *const core::ffi::c_void,
    n: usize,
) -> *mut core::ffi::c_void {
    unsafe { copy_forward(dst.cast(), src.cast(), n) };
    dst
}

/// Overlap-safe copy. Backward (descending, cannot clobber unread source
/// bytes) when `dst` overlaps `src` from above; forward otherwise.
#[cfg(target_arch = "riscv32")]
#[unsafe(no_mangle)]
unsafe extern "C" fn memmove(
    dst: *mut core::ffi::c_void,
    src: *const core::ffi::c_void,
    n: usize,
) -> *mut core::ffi::c_void {
    unsafe { move_any(dst.cast(), src.cast(), n) };
    dst
}

/// Fill, eight words per iteration, any alignment. C semantics: fill value
/// is `c as u8`.
#[cfg(target_arch = "riscv32")]
#[unsafe(no_mangle)]
unsafe extern "C" fn memset(
    dst: *mut core::ffi::c_void,
    c: i32,
    n: usize,
) -> *mut core::ffi::c_void {
    unsafe { fill(dst.cast::<u8>(), c as u8, n) };
    dst
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The longest length tested: two whole blocks, then every word count
    /// below a block, then three bytes.
    const MAX: usize = 3 * BLOCK - 1;

    /// Every test runs across all seam lengths 0..=MAX (covers: empty,
    /// byte-only, exact word, word+tail, exact block, block+words+tail,
    /// multi-block).
    const LENS: core::ops::RangeInclusive<usize> = 0..=MAX;

    /// Overlap distances below, at and above a word and a block.
    const DISTANCES: [usize; 7] = [1, 2, 3, 4, BLOCK - 1, BLOCK, BLOCK + 1];

    // Run under Miri: ordinary execution cannot detect uninitialized reads.
    #[test]
    fn copy_partially_initialized_disjoint() {
        for copy in [copy_forward, copy_backward] {
            for n in LENS {
                let mut src = [MaybeUninit::<u8>::uninit(); MAX];
                for i in (1..n).step_by(2) {
                    src[i].write(i as u8);
                }
                let mut dst = [MaybeUninit::new(0xCCu8); MAX + 2];
                unsafe {
                    copy(dst.as_mut_ptr().add(1), src.as_ptr(), n);
                    for i in (1..n).step_by(2) {
                        assert_eq!(dst[i + 1].assume_init(), i as u8);
                    }
                    assert_eq!(dst[0].assume_init(), 0xCC);
                    for byte in &dst[n + 1..] {
                        assert_eq!(byte.assume_init(), 0xCC);
                    }
                }
            }
        }
    }

    #[test]
    fn move_partially_initialized_overlaps() {
        let offsets = DISTANCES
            .iter()
            .flat_map(|&k| [(0, k), (k, 0)])
            .chain([(0, 0)]);
        for (src, dst) in offsets {
            for n in LENS {
                let mut buf = [MaybeUninit::<u8>::uninit(); MAX + BLOCK + 1];
                for i in (1..buf.len()).step_by(2) {
                    buf[i].write(i as u8);
                }
                let ptr = buf.as_mut_ptr();
                unsafe {
                    move_any(ptr.add(dst), ptr.add(src), n);
                    for i in 0..n {
                        if (src + i) % 2 == 1 {
                            assert_eq!(
                                buf[dst + i].assume_init(),
                                (src + i) as u8
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn copy_forward_disjoint() {
        for n in LENS {
            let src: Vec<u8> = (10..10 + MAX as u8).collect();
            let mut dst = vec![0u8; MAX];
            unsafe {
                copy_forward(dst.as_mut_ptr().cast(), src.as_ptr().cast(), n)
            };
            assert_eq!(&dst[..n], &src[..n], "n={n}");
            assert!(dst[n..].iter().all(|&b| b == 0), "n={n} wrote past end");
        }
    }

    #[test]
    fn copy_backward_disjoint() {
        for n in LENS {
            let src: Vec<u8> = (10..10 + MAX as u8).collect();
            let mut dst = vec![0u8; MAX];
            unsafe {
                copy_backward(dst.as_mut_ptr().cast(), src.as_ptr().cast(), n)
            };
            assert_eq!(&dst[..n], &src[..n], "n={n}");
            assert!(dst[n..].iter().all(|&b| b == 0), "n={n} wrote past end");
        }
    }

    #[test]
    fn move_any_forward_overlaps_all_distances() {
        // dst < src overlap: shift a window left by k bytes.
        for k in DISTANCES {
            for n in LENS {
                let before: Vec<u8> = (0..(n + k) as u8).collect();
                let mut buf = before.clone();
                let ptr = buf.as_mut_ptr().cast::<MaybeUninit<u8>>();
                unsafe { move_any(ptr, ptr.add(k), n) };
                assert_eq!(&buf[..n], &before[k..], "k={k} n={n}");
                // The move's last k source bytes lie past its destination.
                assert_eq!(&buf[n..], &before[n..], "k={k} n={n} tail");
            }
        }
    }

    #[test]
    fn move_any_backward_overlaps_all_distances() {
        // dst > src overlap: shift a window right by k bytes.
        for k in DISTANCES {
            for n in LENS {
                let before: Vec<u8> = (0..(n + k) as u8).collect();
                let mut buf = before.clone();
                let ptr = buf.as_mut_ptr().cast::<MaybeUninit<u8>>();
                unsafe { move_any(ptr.add(k), ptr, n) };
                assert_eq!(&buf[k..], &before[..n], "k={k} n={n}");
                // The move's first k source bytes lie below its destination.
                assert_eq!(
                    &buf[..k.min(n)],
                    &before[..k.min(n)],
                    "k={k} n={n}"
                );
            }
        }
    }

    #[test]
    fn move_any_disjoint_on_either_side() {
        // Source and destination touch but do not overlap, in both orders.
        for n in LENS {
            for (src, dst) in [(0, n), (n, 0)] {
                let before: Vec<u8> = (0..2 * n as u8).collect();
                let mut buf = before.clone();
                let ptr = buf.as_mut_ptr().cast::<MaybeUninit<u8>>();
                unsafe { move_any(ptr.add(dst), ptr.add(src), n) };
                assert_eq!(buf[dst..dst + n], before[src..src + n], "n={n}");
                assert_eq!(buf[src..src + n], before[src..src + n], "n={n}");
            }
        }
    }

    #[test]
    fn move_any_exact_overlap_is_noop() {
        for n in LENS {
            let mut buf: Vec<u8> = (0..MAX as u8).collect();
            let want = buf.clone();
            let ptr = buf.as_mut_ptr().cast::<MaybeUninit<u8>>();
            unsafe { move_any(ptr, ptr, n) };
            assert_eq!(buf, want, "n={n}");
        }
    }

    #[test]
    fn fill_all_seams_and_truncation() {
        for n in LENS {
            // Each alignment relative to a word.
            for at in 0..4 {
                let mut dst = vec![0u8; MAX + 4];
                unsafe { fill(dst.as_mut_ptr().add(at), 0xAB, n) };
                let (below, rest) = dst.split_at(at);
                let (filled, above) = rest.split_at(n);
                assert!(below.iter().all(|&b| b == 0), "n={n} at={at}");
                assert!(filled.iter().all(|&b| b == 0xAB), "n={n} at={at}");
                assert!(above.iter().all(|&b| b == 0), "n={n} at={at}");
            }
        }
        // C semantics: `c as u8` truncation happens in the memset wrapper —
        // mirror the documented contract here.
        let mut one = [0u8; 1];
        unsafe { fill(one.as_mut_ptr(), 0x1FFi32 as u8, 1) };
        assert_eq!(one[0], 0xFF);
    }
}
