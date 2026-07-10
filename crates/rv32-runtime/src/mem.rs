//! Lean `memcpy`/`memmove`/`memset` for the hogd rv32 VM.
//!
//! compiler_builtins' generic versions are written for hardware where
//! unaligned word access traps: memcpy alone is ~500 B, ~340 B of which is a
//! shift-and-splice path for misaligned sources (plus a 16 B stack frame).
//! The hogd VM permits unaligned scalar loads/stores (the contract rustflags
//! set `-C target-feature=+unaligned-scalar-mem`), so word loops here use
//! `read_unaligned`/`write_unaligned` and need no alignment handling at all:
//! the hot loop is 5 instructions per 4 bytes at ANY alignment.
//!
//! These are strong definitions overriding compiler_builtins' weak symbols
//! at link time. Gas = 1 per executed instruction on this VM, so the loops
//! are written for minimal instruction count, not host-CPU tricks.
//!
//! RECURSION GUARD: LLVM's loop-idiom recognition can lower copy/fill loops
//! back into `memcpy`/`memset` calls — inside these functions that would be
//! infinite recursion. The crate is `#![no_builtins]` (lib.rs) to forbid
//! that; the contract-CI disassembly check asserts it stayed true.

/// Forward copy, word-granular, any alignment. Hot loop: lw/sw/addi/addi/bltu.
#[unsafe(no_mangle)]
unsafe extern "C" fn memcpy(
    dst: *mut u8,
    src: *const u8,
    n: usize,
) -> *mut u8 {
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
    dst
}

/// Overlap-safe copy. Forward when `dst` is at or below `src` (ascending
/// writes never overtake the reads when the source is ahead). Backward word
/// loop otherwise: with `dst` above `src`, descending writes cannot clobber
/// source bytes before they are read.
#[unsafe(no_mangle)]
unsafe extern "C" fn memmove(
    dst: *mut u8,
    src: *const u8,
    n: usize,
) -> *mut u8 {
    unsafe {
        if (dst as usize) <= (src as usize) {
            // Covers dst == src and every forward-safe overlap.
            memcpy(dst, src, n);
        } else {
            // dst overlaps src's tail: copy highest bytes first.
            let mut i = n;
            while i >= 4 {
                i -= 4;
                dst.add(i).cast::<u32>().write_unaligned(
                    src.add(i).cast::<u32>().read_unaligned(),
                );
            }
            while i > 0 {
                i -= 1;
                *dst.add(i) = *src.add(i);
            }
        }
    }
    dst
}

/// Word-granular fill, any alignment. C semantics: fill value is `c as u8`.
#[unsafe(no_mangle)]
unsafe extern "C" fn memset(dst: *mut u8, c: i32, n: usize) -> *mut u8 {
    unsafe {
        let word = u32::from_ne_bytes([c as u8; 4]);
        let mut d = dst;
        let word_end = dst.add(n & !3);
        while d < word_end {
            d.cast::<u32>().write_unaligned(word);
            d = d.add(4);
        }
        let end = dst.add(n);
        while d < end {
            *d = c as u8;
            d = d.add(1);
        }
    }
    dst
}
