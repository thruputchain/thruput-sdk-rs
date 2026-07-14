//! Lean `memcpy`/`memmove`/`memset` for the hogd rv32 VM.
//!
//! compiler_builtins' generic versions are written for hardware where
//! unaligned word access traps: memcpy alone is ~500 B, ~340 B of which is a
//! shift-and-splice path for misaligned sources (plus a 16 B stack frame).
//! The hogd VM permits unaligned scalar loads/stores (the contract rustflags
//! set `-C target-feature=+unaligned-scalar-mem`), so word loops here use
//! `read_unaligned`/`write_unaligned` and need no alignment handling at all:
//! the hot loop is 5 instructions per 4 bytes at ANY alignment.
//! (Without `+unaligned-scalar-mem` — e.g. the publish-workflow rlib builds
//! that use prebuilt core and no contract rustflags — each unaligned word
//! access byte-expands: still correct, just not the fast path. The flag is
//! what makes these bodies a win; it is part of the canonical contract
//! rustflags.)
//!
//! These are strong definitions overriding compiler_builtins' weak symbols
//! at link time. Gas = 1 per executed instruction on this VM, so the loops
//! are written for minimal instruction count, not host-CPU tricks.
//!
//! RECURSION GUARD: LLVM's loop-idiom recognition can lower copy/fill loops
//! back into `memcpy`/`memset` calls — inside these functions that would be
//! infinite recursion. The crate is `#![no_builtins]` (lib.rs) to forbid
//! that. CI asserts the override held per contract build (dev.sh
//! check_rv32_mem_symbols: strong memcpy present, compiler_builtins mem
//! symbols absent); recursion itself was ruled out by disassembly when
//! this landed and no_builtins keeps it that way.

// ---------------------------------------------------------------------------
// Pure copy/fill cores — target-independent so the overlap/seam logic can be
// unit-tested on the host (cf. dispatch.rs). `#[inline(always)]` so each rv32
// wrapper below compiles to exactly the same outlined body as writing the
// loop in the wrapper directly; the wrappers are the only exported symbols.
// ---------------------------------------------------------------------------

/// Strictly ascending word-then-byte copy. The ascending order is a load-
/// bearing contract: `move_any` delegates overlapping `dst <= src` copies
/// here, which is only correct because writes never overtake reads. Any
/// rewrite that reorders writes (unrolling with reordering, a host-memcpy
/// syscall with unspecified overlap semantics) must first stop `move_any`
/// from delegating overlaps here.
#[inline(always)]
unsafe fn copy_forward(dst: *mut u8, src: *const u8, n: usize) {
    unsafe {
        let mut d = dst;
        let mut s = src;
        let word_end = dst.add(n & !3);
        while d < word_end {
            d.cast::<u32>()
                .write_unaligned(s.cast::<u32>().read_unaligned());
            d = d.add(4);
            s = s.add(4);
        }
        let end = dst.add(n);
        while d < end {
            *d = *s;
            d = d.add(1);
            s = s.add(1);
        }
    }
}

/// Strictly descending word-then-byte copy, for `dst > src` overlaps.
/// Pointer-bump form (the pointers are the induction variables): the indexed
/// form costs a 6-insn word loop — LLVM recomputes dst+i and src+i every
/// iteration instead of decrementing the pointers.
#[inline(always)]
unsafe fn copy_backward(dst: *mut u8, src: *const u8, n: usize) {
    unsafe {
        let mut d = dst.add(n);
        let mut s = src.add(n);
        let word_stop = dst.add(n & 3);
        while d > word_stop {
            d = d.sub(4);
            s = s.sub(4);
            d.cast::<u32>()
                .write_unaligned(s.cast::<u32>().read_unaligned());
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
unsafe fn move_any(dst: *mut u8, src: *const u8, n: usize) {
    unsafe {
        if (dst as usize) <= (src as usize) {
            // Covers dst == src and every forward-safe overlap (ascending
            // writes never overtake the reads when the source is ahead).
            copy_forward(dst, src, n);
        } else {
            // dst overlaps src's tail: copy highest bytes first.
            copy_backward(dst, src, n);
        }
    }
}

/// Word-granular fill; the fill byte is already truncated by the caller.
#[inline(always)]
unsafe fn fill(dst: *mut u8, byte: u8, n: usize) {
    unsafe {
        let word = u32::from_ne_bytes([byte; 4]);
        let mut d = dst;
        let word_end = dst.add(n & !3);
        while d < word_end {
            d.cast::<u32>().write_unaligned(word);
            d = d.add(4);
        }
        let end = dst.add(n);
        while d < end {
            *d = byte;
            d = d.add(1);
        }
    }
}

// ---------------------------------------------------------------------------
// The exported strong symbols (rv32 contracts only).
// ---------------------------------------------------------------------------

/// Forward copy, word-granular, any alignment. Hot loop: lw/sw/addi/addi/bltu.
/// C contract: overlapping ranges are the caller's UB; see `copy_forward` for
/// the internal ascending-order invariant `memmove` relies on.
#[cfg(target_arch = "riscv32")]
#[unsafe(no_mangle)]
unsafe extern "C" fn memcpy(
    dst: *mut u8,
    src: *const u8,
    n: usize,
) -> *mut u8 {
    unsafe { copy_forward(dst, src, n) };
    dst
}

/// Overlap-safe copy. Forward when `dst` is at or below `src`; backward
/// (descending, cannot clobber unread source bytes) otherwise.
#[cfg(target_arch = "riscv32")]
#[unsafe(no_mangle)]
unsafe extern "C" fn memmove(
    dst: *mut u8,
    src: *const u8,
    n: usize,
) -> *mut u8 {
    unsafe { move_any(dst, src, n) };
    dst
}

/// Word-granular fill, any alignment. C semantics: fill value is `c as u8`.
#[cfg(target_arch = "riscv32")]
#[unsafe(no_mangle)]
unsafe extern "C" fn memset(dst: *mut u8, c: i32, n: usize) -> *mut u8 {
    unsafe { fill(dst, c as u8, n) };
    dst
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every test runs across all seam lengths 0..=9 (covers: empty,
    /// byte-only, exact word, word+tail, multi-word).
    const LENS: core::ops::RangeInclusive<usize> = 0..=9;

    #[test]
    fn copy_forward_disjoint() {
        for n in LENS {
            let src: Vec<u8> = (10..19).collect();
            let mut dst = vec![0u8; 9];
            unsafe { copy_forward(dst.as_mut_ptr(), src.as_ptr(), n) };
            assert_eq!(&dst[..n], &src[..n], "n={n}");
            assert!(dst[n..].iter().all(|&b| b == 0), "n={n} wrote past end");
        }
    }

    #[test]
    fn copy_backward_disjoint() {
        for n in LENS {
            let src: Vec<u8> = (10..19).collect();
            let mut dst = vec![0u8; 9];
            unsafe { copy_backward(dst.as_mut_ptr(), src.as_ptr(), n) };
            assert_eq!(&dst[..n], &src[..n], "n={n}");
            assert!(dst[n..].iter().all(|&b| b == 0), "n={n} wrote past end");
        }
    }

    #[test]
    fn move_any_forward_overlaps_all_distances() {
        // dst < src overlap: shift a window left by k=1..=4 bytes.
        for k in 1..=4usize {
            for n in LENS {
                let mut buf: Vec<u8> = (0..(n + k) as u8).collect();
                let want: Vec<u8> = buf[k..k + n].to_vec();
                let ptr = buf.as_mut_ptr();
                unsafe { move_any(ptr, ptr.add(k), n) };
                assert_eq!(&buf[..n], &want[..], "k={k} n={n}");
            }
        }
    }

    #[test]
    fn move_any_backward_overlaps_all_distances() {
        // dst > src overlap: shift a window right by k=1..=4 bytes.
        for k in 1..=4usize {
            for n in LENS {
                let mut buf: Vec<u8> = (0..(n + k) as u8).collect();
                let want: Vec<u8> = buf[..n].to_vec();
                let ptr = buf.as_mut_ptr();
                unsafe { move_any(ptr.add(k), ptr, n) };
                assert_eq!(&buf[k..k + n], &want[..], "k={k} n={n}");
            }
        }
    }

    #[test]
    fn move_any_exact_overlap_is_noop() {
        for n in LENS {
            let mut buf: Vec<u8> = (0..9).collect();
            let want = buf.clone();
            let ptr = buf.as_mut_ptr();
            unsafe { move_any(ptr, ptr, n) };
            assert_eq!(buf, want, "n={n}");
        }
    }

    #[test]
    fn fill_all_seams_and_truncation() {
        for n in LENS {
            let mut dst = vec![0u8; 9];
            unsafe { fill(dst.as_mut_ptr(), 0xAB, n) };
            assert!(dst[..n].iter().all(|&b| b == 0xAB), "n={n}");
            assert!(dst[n..].iter().all(|&b| b == 0), "n={n} wrote past end");
        }
        // C semantics: `c as u8` truncation happens in the memset wrapper —
        // mirror the documented contract here.
        let mut one = [0u8; 1];
        unsafe { fill(one.as_mut_ptr(), 0x1FFi32 as u8, 1) };
        assert_eq!(one[0], 0xFF);
    }
}
