//! The interface table: which interfaces this contract implements, collected
//! by the linker from its `#[contract]` impl blocks.
//!
//! Every `#[contract] impl Trait for T` (and every bare `#[contract] impl T`)
//! expands to one 8-byte [`InterfaceId`] record via
//! [`__interface_record!`](crate::__interface_record). [`implements!`](crate::implements)
//! answers whether a given id is among them. This is what `groundhog-erc165-if`
//! builds `supports_interface` on: the claim is read off the impls that
//! actually linked, so there is no list to maintain and nothing to get wrong.
//!
//! # Layout and cost
//!
//! On the guest the records use the entrypoint table's shape: `#[used]`
//! statics in the `groundhog_rv32_interfaces` input section, which the
//! runtime linker script `KEEP`s and brackets as its own output section.
//! Whether that section reaches the deployed image is the lowerer's call,
//! made the way it already drops the raw spec section: the reader below
//! touches a one-word anchor in `groundhog_rv32_interfaces_anchor`, an
//! ordinary GC-able section that survives the link only if the table reader
//! behind [`implements!`](crate::implements) was instantiated. The lowerer copies the table into VM memory only when
//! the anchor section is present. A contract with no ERC-165 impl therefore
//! ships an image byte-identical to one built without this module; one with
//! an impl pays four bytes of anchor plus 8 per impl block, plus the scan.
//!
//! # What counts as "this contract"
//!
//! The unit is the contract image, not the storage struct. A crate with
//! `impl X for A` and `impl Y for B` links both into one image whose
//! dispatch table answers to both interfaces at one address, so its table
//! holds both ids and `supports_interface` says yes to both — which is the
//! truth ERC-165 asks about. On the guest the table is simply the whole
//! image. On the host, where one test process holds many contracts, the
//! records go through a `linkme` slice keyed by the package that expanded
//! them, name and version both, so [`implements!`](crate::implements) from
//! inside a contract crate sees every impl block of that crate — `A`'s and
//! `B`'s alike, the same as the guest — while another contract crate's
//! records, or another version of this one (a migration test may link two),
//! stay apart. That is the scope the entrypoint guards and the mock
//! registry already use for "one contract".

use crate::InterfaceId;

/// One host-side record: the package (name and version) whose `#[contract]`
/// impl block expanded it, and the interface that block implements.
#[cfg(not(target_arch = "riscv32"))]
#[doc(hidden)]
pub struct HostRecord {
    pub package: &'static str,
    pub version: &'static str,
    pub id: InterfaceId,
}

#[cfg(not(target_arch = "riscv32"))]
#[doc(hidden)]
#[crate::__linkme::distributed_slice]
#[linkme(crate = crate::__linkme)]
pub static HOST_RECORDS: [HostRecord];

/// True if the contract identified by `package`/`version` carries a
/// `#[contract]` impl of the interface identified by `interface_id`.
///
/// [`implements!`](crate::implements) supplies the caller crate's own
/// package name and version, which is the form contracts use. On the guest
/// there is one contract per image, so the scope is not consulted and the
/// table is scanned whole; on the host the records are filtered to that
/// package. Neither touches storage.
#[cfg(target_arch = "riscv32")]
pub fn implements_in(
    _package: &str,
    _version: &str,
    interface_id: InterfaceId,
) -> bool {
    rv32::contains(interface_id)
}

/// True if the contract identified by `package`/`version` carries a
/// `#[contract]` impl of the interface identified by `interface_id`.
///
/// [`implements!`](crate::implements) supplies the caller crate's own
/// package name and version, which is the form contracts use. On the guest
/// there is one contract per image, so the scope is not consulted and the
/// table is scanned whole; on the host the records are filtered to that
/// package. Neither touches storage.
#[cfg(not(target_arch = "riscv32"))]
pub fn implements_in(
    package: &str,
    version: &str,
    interface_id: InterfaceId,
) -> bool {
    HOST_RECORDS.iter().any(|r| {
        r.id == interface_id && r.package == package && r.version == version
    })
}

/// True if this contract carries a `#[contract]` impl of the interface
/// identified by the given [`InterfaceId`].
///
/// Expands to [`interface_table::implements_in`](crate::interface_table::implements_in)
/// scoped to the invoking crate's package name and version, so it must be
/// invoked from the contract crate itself — which is where a
/// `supports_interface` body lives:
///
/// ```ignore
/// fn supports_interface(id: InterfaceId) -> bool {
///     sdk::implements!(id)
/// }
/// ```
#[macro_export]
macro_rules! implements {
    ($id:expr) => {
        $crate::interface_table::implements_in(
            env!("CARGO_PKG_NAME"),
            env!("CARGO_PKG_VERSION"),
            $id,
        )
    };
}

#[cfg(target_arch = "riscv32")]
mod rv32 {
    // Bracket symbols for the `groundhog_rv32_interfaces` output section,
    // defined by link/groundhog-rv32.ld in the rv32-runtime crate. Declared
    // as records so `&raw const` yields correctly-typed pointers; only their
    // addresses are read.
    unsafe extern "C" {
        static __groundhog_rv32_interfaces_start: super::InterfaceId;
        static __groundhog_rv32_interfaces_end: super::InterfaceId;
    }

    // The lowerer's signal that the table has a reader. Deliberately not
    // `#[used]`: the linker keeps this static only because `contains` reads
    // it, so its section exists in the ELF exactly when `implements` was
    // instantiated, and the lowerer keys the table's presence in the image
    // off that. Section name shared with the linker script and the
    // groundhog-rpc lowerer (`INTERFACE_TABLE_ANCHOR_SECTION`).
    #[unsafe(link_section = "groundhog_rv32_interfaces_anchor")]
    static ANCHOR: u32 = 1;

    /// Scan the table for `wanted`. Out of line and cold so LLVM keeps it
    /// a plain eight-byte-stride loop: inlined at -O3 it unrolls the scan
    /// eightfold, ~250 bytes of code for a table of a handful of records.
    #[cold]
    #[inline(never)]
    pub(super) fn contains(wanted: super::InterfaceId) -> bool {
        // SAFETY: the linker script ALIGN(8)s the section start and brackets
        // exactly the KEEP'd `groundhog_rv32_interfaces` input section, whose
        // sole writer is `__interface_record!` (each entry a `static [u8;
        // 8]`, so the section is a well-formed array of stride 8). An empty
        // table gives start == end and no reads. The bracket symbols are
        // distinct declarations, so they are compared as addresses rather
        // than through `offset_from`, which needs shared provenance.
        unsafe {
            core::ptr::read_volatile(&raw const ANCHOR);
            let mut p = &raw const __groundhog_rv32_interfaces_start;
            let end = &raw const __groundhog_rv32_interfaces_end;
            while (p as usize) < (end as usize) {
                if core::ptr::read_volatile(p) == wanted {
                    return true;
                }
                p = p.add(1);
            }
            false
        }
    }
}

/// Register one interface-table record for the invoking crate. Emitted by
/// `#[contract]` for every impl block; not for hand-written use.
#[doc(hidden)]
#[macro_export]
macro_rules! __interface_record {
    ($id:expr) => {
        const _: () = {
            #[cfg(target_arch = "riscv32")]
            #[used]
            #[unsafe(link_section = "groundhog_rv32_interfaces")]
            static __GH_INTERFACE_RECORD: $crate::InterfaceId = $id;

            #[cfg(not(target_arch = "riscv32"))]
            #[$crate::__linkme::distributed_slice(
                $crate::interface_table::HOST_RECORDS
            )]
            #[linkme(crate = $crate::__linkme)]
            static __GH_INTERFACE_RECORD: $crate::interface_table::HostRecord =
                $crate::interface_table::HostRecord {
                    package: env!("CARGO_PKG_NAME"),
                    version: env!("CARGO_PKG_VERSION"),
                    id: $id,
                };
        };
    };
}
