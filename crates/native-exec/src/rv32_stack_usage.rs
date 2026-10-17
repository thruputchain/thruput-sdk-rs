//! The rv32 guest stack-usage diagnostic the test FFI records per invocation.

use core::ffi::c_void;
use core::ops::Range;
use thruput_sys::FFIErrorCode;
use thruput_xdr::types::ContractAddress;

/// The value [`Rv32StackUsage::sp_initial`] and [`Rv32StackUsage::sp_min`]
/// hold when the guest never wrote `sp`.
pub const SP_UNWRITTEN: u32 = u32::MAX;

/// What an rv32 guest did with its stack pointer over one invocation.
///
/// hogd records one of these per rv32 invocation, when the invocation ends,
/// in Debug builds with `HOGD_RV32_STACK_USAGE` on -- which the prebuilt
/// test_ffi library this crate links is. `sp_initial` is the first value the
/// guest stored to `sp` (x2): the stack top its `_start` established.
/// `sp_min` is the lowest, the deepest point the stack reached, so
/// [`bytes_used`](Self::bytes_used) is their difference. A fused
/// `lui sp; addi sp, sp, imm` prologue stores only the constant it
/// materializes, so `sp_initial` is the top the program established, not the
/// `lui` intermediate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rv32StackUsage {
    /// The invoked contract.
    pub contract_address: ContractAddress,
    /// The invoked method id.
    pub method_id: u64,
    /// First value the guest wrote to `sp`, or [`SP_UNWRITTEN`].
    pub sp_initial: u32,
    /// Lowest value the guest wrote to `sp`, or [`SP_UNWRITTEN`].
    pub sp_min: u32,
}

impl Rv32StackUsage {
    /// True if the guest wrote `sp` at all.
    pub fn wrote_sp(&self) -> bool {
        self.sp_initial != SP_UNWRITTEN
    }

    /// Bytes of stack the invocation used: `sp_initial - sp_min`. `None` if
    /// the guest never wrote `sp`.
    pub fn bytes_used(&self) -> Option<u32> {
        self.wrote_sp()
            .then(|| self.sp_initial.saturating_sub(self.sp_min))
    }

    fn from_ffi(record: &thruput_sys::FFIRv32StackUsage) -> Self {
        Self {
            contract_address: record.contract_address.into(),
            method_id: record.method_id,
            sp_initial: record.sp_initial,
            sp_min: record.sp_min,
        }
    }
}

/// Decode the packed `FFIRv32StackUsage` array `get_rv32_stack_usage`
/// returns, in the order the invocations finished.
fn decode_records(bytes: &[u8]) -> Vec<Rv32StackUsage> {
    let size = size_of::<thruput_sys::FFIRv32StackUsage>();
    assert!(
        bytes.len().is_multiple_of(size),
        "rv32 stack usage buffer of {} bytes is not a whole number of \
         {size}-byte records",
        bytes.len()
    );
    bytes
        .chunks_exact(size)
        .map(|chunk| {
            // The buffer is a C array of the struct, so every chunk is one
            // complete record; read_unaligned makes no alignment assumption
            // about the allocation.
            let record: thruput_sys::FFIRv32StackUsage = unsafe {
                core::ptr::read_unaligned(
                    chunk.as_ptr() as *const thruput_sys::FFIRv32StackUsage
                )
            };
            Rv32StackUsage::from_ffi(&record)
        })
        .collect()
}

/// Every record on `handler`, in the order the invocations finished: a
/// callee's precedes its caller's. Empty if no rv32 invocation has finished
/// on the handler, or if the library was built without the diagnostic.
///
/// The handler appends to this list for the life of its transaction and
/// never removes from it, so the length a frame saw when it opened is where
/// the records of the calls made under it begin; see [`frame_records`].
pub(crate) fn records(handler: *mut c_void) -> Vec<Rv32StackUsage> {
    // `records` frees itself on drop (see thruput_sys::FFIDataBuffer).
    let res = unsafe { thruput_sys::get_rv32_stack_usage(handler) };
    let err: FFIErrorCode = res.ffi_error_code.into();
    if err != FFIErrorCode::None {
        panic!("failed to read rv32 stack usage: {err}");
    }
    res.records.as_slice().map_or_else(Vec::new, decode_records)
}

/// The records a frame owns, out of the handler's `records`: those at the
/// indices in `window` (appended while the frame was open) less those in any
/// of `child_windows` (appended while one of its child frames was open; each
/// lies inside `window`). Order is kept, so the last is the frame's most
/// recent call.
pub(crate) fn frame_records(
    records: &[Rv32StackUsage],
    window: Range<usize>,
    child_windows: &[Range<usize>],
) -> Vec<Rv32StackUsage> {
    let start = window.start;
    records
        .get(window)
        .unwrap_or(&[])
        .iter()
        .enumerate()
        .filter(|(offset, _)| {
            let index = start + offset;
            !child_windows.iter().any(|child| child.contains(&index))
        })
        .map(|(_, record)| *record)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ffi_record(
        address_byte: u8,
        method_id: u64,
        sp_initial: u32,
        sp_min: u32,
    ) -> thruput_sys::FFIRv32StackUsage {
        thruput_sys::FFIRv32StackUsage {
            contract_address: [address_byte; 32],
            method_id,
            sp_initial,
            sp_min,
        }
    }

    fn as_bytes(records: &[thruput_sys::FFIRv32StackUsage]) -> &[u8] {
        unsafe {
            core::slice::from_raw_parts(
                records.as_ptr() as *const u8,
                size_of_val(records),
            )
        }
    }

    #[test]
    fn decodes_records_in_order_and_computes_usage() {
        let records = [
            ffi_record(0xA1, 7, 0x10000, 0x10000 - 96),
            ffi_record(0xB2, 9, SP_UNWRITTEN, SP_UNWRITTEN),
        ];
        let decoded = decode_records(as_bytes(&records));
        assert_eq!(decoded.len(), 2);

        let first = decoded[0];
        assert_eq!(first.contract_address, [0xA1u8; 32].into());
        assert_eq!(first.method_id, 7);
        assert_eq!(first.sp_initial, 0x10000);
        assert_eq!(first.sp_min, 0x10000 - 96);
        assert!(first.wrote_sp());
        assert_eq!(first.bytes_used(), Some(96));

        let second = decoded[1];
        assert_eq!(second.method_id, 9);
        assert!(!second.wrote_sp());
        assert_eq!(second.bytes_used(), None);
    }

    #[test]
    fn empty_buffer_decodes_to_nothing() {
        assert!(decode_records(&[]).is_empty());
    }

    #[test]
    #[should_panic(expected = "not a whole number")]
    fn partial_record_is_rejected() {
        let records = [ffi_record(0, 0, 0, 0)];
        let bytes = as_bytes(&records);
        let _ = decode_records(&bytes[..bytes.len() - 1]);
    }

    /// A transaction's records numbered by finish order, so a frame's
    /// selection reads as a list of indices.
    fn numbered(count: u64) -> Vec<Rv32StackUsage> {
        (0..count)
            .map(|n| Rv32StackUsage {
                contract_address: [n as u8; 32].into(),
                method_id: n,
                sp_initial: 0x1000,
                sp_min: 0x0f00,
            })
            .collect()
    }

    fn method_ids(records: &[Rv32StackUsage]) -> Vec<u64> {
        records.iter().map(|record| record.method_id).collect()
    }

    #[test]
    fn frame_records_are_the_window_less_the_children() {
        // 0 and 1 finished before the frame opened; 2 is its own; 3 and 4
        // ran under its first child; 5 is its own; 6 ran under its second
        // child; 7 finished after it closed.
        let records = numbered(8);
        let own = frame_records(&records, 2..7, &[3..5, 6..7]);
        assert_eq!(method_ids(&own), [2, 5]);
        assert_eq!(own.last().map(|record| record.method_id), Some(5));
    }

    #[test]
    fn frame_records_without_children_is_the_whole_window() {
        let records = numbered(4);
        assert_eq!(method_ids(&frame_records(&records, 1..3, &[])), [1, 2]);
    }

    #[test]
    fn frame_that_ran_no_rv32_code_has_no_records() {
        let records = numbered(3);
        // Opened after every record was appended.
        assert!(frame_records(&records, 3..3, &[]).is_empty());
        // Every record in its window was its child's.
        assert!(frame_records(&records, 1..3, &[1..3]).is_empty());
        // Nothing ran at all.
        assert!(frame_records(&[], 0..0, &[]).is_empty());
    }
}
