//! The rv32 guest stack-usage diagnostic the test FFI records per invocation.

use core::ffi::c_void;
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

/// The record of the most recent rv32 invocation that finished on
/// `handler`, or `None` if none has (or the library was built without the
/// diagnostic).
pub(crate) fn last_rv32_stack_usage(
    handler: *mut c_void,
) -> Option<Rv32StackUsage> {
    // `records` frees itself on drop (see thruput_sys::FFIDataBuffer).
    let res = unsafe { thruput_sys::get_rv32_stack_usage(handler) };
    let err: FFIErrorCode = res.ffi_error_code.into();
    if err != FFIErrorCode::None {
        panic!("failed to read rv32 stack usage: {err}");
    }
    res.records
        .as_slice()
        .and_then(|bytes| decode_records(bytes).pop())
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
}
