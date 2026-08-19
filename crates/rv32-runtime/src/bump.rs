//! Pure bump-allocation arithmetic for the guest global allocator.
//!
//! Target-independent so the alignment / overflow / exhaustion logic can be
//! unit-tested on the host (same pattern as dispatch.rs and mem.rs). The
//! allocator module is the only caller: it supplies the cursor and the heap
//! ceiling, and turns the returned address back into a pointer.
//!
//! Self-contained integer math with no dependencies, which is what keeps this
//! runtime's dependency graph empty.

use core::alloc::Layout;

/// Place `layout` at or after `cursor`, keeping the whole allocation below
/// `ceiling`.
///
/// Returns `(start, new_cursor)` as absolute addresses in the guest's flat
/// address space, or `None` if the allocation does not fit — which the caller
/// reports as allocation failure (a null return), never as a wrapped or
/// out-of-heap pointer.
///
/// # Domain
///
/// Requires `cursor <= ceiling`, with `ceiling` at most the guest's memory
/// size. The allocator upholds this by construction: `cursor` starts at
/// `__heap_base` and thereafter only takes values this function already
/// checked against `ceiling`.
///
/// Given that, neither addition below can overflow on a 32-bit target —
/// `cursor` is at most 64 Ki, `Layout` caps alignment at 2^29, and `size`
/// rounded up to `align` fits in `isize::MAX`. So they are left unchecked:
/// a `checked_add` pair costs code size and an extra gas-metered branch per
/// allocation to handle a case that cannot arise. The `debug_assert` pins
/// the invariant for host tests and Miri, and compiles out in release.
#[inline(always)]
pub(crate) fn bump(
    cursor: usize,
    ceiling: usize,
    layout: Layout,
) -> Option<(usize, usize)> {
    debug_assert!(cursor <= ceiling, "cursor escaped the heap");
    // `Layout` guarantees a nonzero power-of-two alignment, so `align - 1`
    // cannot underflow and is a valid mask.
    let mask = layout.align() - 1;
    let start = (cursor + mask) & !mask;
    let new_cursor = start + layout.size();
    if new_cursor > ceiling {
        return None;
    }
    Some((start, new_cursor))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout(size: usize, align: usize) -> Layout {
        Layout::from_size_align(size, align).unwrap()
    }

    #[test]
    fn aligns_up_then_advances() {
        // cursor 1, align 8 -> start 8, cursor 8 + 4 = 12.
        assert_eq!(bump(1, 1024, layout(4, 8)), Some((8, 12)));
        // already aligned: no movement.
        assert_eq!(bump(16, 1024, layout(4, 8)), Some((16, 20)));
        // align 1 never moves the start.
        assert_eq!(bump(7, 1024, layout(3, 1)), Some((7, 10)));
    }

    #[test]
    fn exact_fit_to_the_ceiling_is_allowed() {
        assert_eq!(bump(60, 64, layout(4, 1)), Some((60, 64)));
    }

    #[test]
    fn one_byte_past_the_ceiling_is_refused() {
        assert_eq!(bump(60, 64, layout(5, 1)), None);
    }

    #[test]
    fn alignment_padding_counts_against_the_ceiling() {
        // Start rounds to 64, which is already the ceiling, so even a
        // 1-byte allocation must fail rather than run past it.
        assert_eq!(bump(60, 64, layout(1, 64)), None);
    }

    #[test]
    fn absurdly_large_requests_are_refused_not_wrapped() {
        // Inside the documented domain, a request far larger than the heap
        // must be refused outright. If the size addition ever wrapped it
        // would produce a small `new_cursor` that slips past the ceiling
        // check and yields a pointer outside the heap, so pin the behaviour
        // even though the invariants make a wrap unreachable.
        const CEILING: usize = 64 * 1024;
        assert_eq!(bump(0, CEILING, layout(isize::MAX as usize, 1)), None);
        assert_eq!(
            bump(CEILING - 1, CEILING, layout(isize::MAX as usize, 1)),
            None
        );
        // Alignment far larger than the heap, likewise.
        assert_eq!(bump(1, CEILING, layout(1, 1 << 20)), None);
    }

    /// Drive `bump` over a real allocation and use every pointer it yields.
    ///
    /// This is the case Miri checks: the arithmetic is the production
    /// function, and pointers formed from its results are written through and
    /// read back, so overlapping or out-of-heap results would surface as an
    /// aliasing or out-of-bounds error rather than a silent pass.
    ///
    /// What it does not cover: on device the base address comes from the
    /// `__heap_base` linker symbol, which Miri cannot model. Here the base is
    /// a real Rust allocation, so this validates the arithmetic and the usage
    /// pattern, not the extern-static derivation.
    #[test]
    fn allocations_are_aligned_disjoint_and_writable() {
        const N: usize = 512;
        let mut heap = vec![0u8; N];
        let base = heap.as_mut_ptr();
        let base_addr = base as usize;
        let ceiling = base_addr + N;

        let mut cursor = base_addr;
        let mut handed_out: Vec<(usize, usize)> = Vec::new();

        let requests = [(1usize, 1usize), (3, 4), (8, 8), (17, 2), (32, 16)];
        for (i, &(size, align)) in requests.iter().enumerate() {
            let (start, next) =
                bump(cursor, ceiling, layout(size, align)).unwrap();
            cursor = next;

            assert_eq!(start % align, 0, "alloc {i} misaligned");
            assert!(start >= base_addr, "alloc {i} below the heap");
            assert!(next <= ceiling, "alloc {i} past the ceiling");

            // Derive the pointer from `base` rather than from an integer, so
            // the write carries the heap allocation's provenance.
            let p = unsafe { base.add(start - base_addr) };
            unsafe { core::ptr::write_bytes(p, 0xA0 | i as u8, size) };
            handed_out.push((start, size));
        }

        for (i, &(s1, n1)) in handed_out.iter().enumerate() {
            for &(s2, n2) in &handed_out[i + 1..] {
                assert!(
                    s1 + n1 <= s2 || s2 + n2 <= s1,
                    "alloc {i} overlaps a later allocation"
                );
            }
        }

        // Each allocation still holds its own tag: proof the ranges really
        // were disjoint, not merely asserted to be.
        for (i, &(start, size)) in handed_out.iter().enumerate() {
            let p = unsafe { base.add(start - base_addr) };
            let seen = unsafe { core::slice::from_raw_parts(p, size) };
            assert!(
                seen.iter().all(|&b| b == (0xA0 | i as u8)),
                "alloc {i} was clobbered"
            );
        }
    }

    #[test]
    fn exhaustion_refuses_instead_of_leaving_the_heap() {
        const N: usize = 64;
        let mut heap = vec![0u8; N];
        let base_addr = heap.as_mut_ptr() as usize;
        let ceiling = base_addr + N;

        let mut cursor = base_addr;
        let mut served = 0usize;
        // Far more requests than the heap can satisfy.
        for _ in 0..N * 2 {
            match bump(cursor, ceiling, layout(8, 8)) {
                Some((start, next)) => {
                    assert!(next <= ceiling, "served past the ceiling");
                    assert!(start >= base_addr);
                    cursor = next;
                    served += 1;
                }
                None => break,
            }
        }
        // Capacity is measured from the first 8-aligned address at or after
        // the base, not from the base itself: nothing guarantees a `Vec`'s
        // buffer is 8-aligned, and any padding the first allocation needs
        // comes out of the heap. (On device `__heap_base` is `ALIGN(8)`, but
        // this test should not depend on that.)
        let first = (base_addr + 7) & !7;
        assert_eq!(
            served,
            (ceiling - first) / 8,
            "should serve exactly the heap's capacity"
        );
        // Refusal does not move the cursor, so the same request keeps being
        // refused. Checked with the same shape: a smaller or less-aligned
        // request may still fit in the tail below the ceiling, which is
        // correct behaviour rather than exhaustion.
        assert_eq!(bump(cursor, ceiling, layout(8, 8)), None);
        assert_eq!(bump(cursor, ceiling, layout(8, 8)), None);
    }
}
