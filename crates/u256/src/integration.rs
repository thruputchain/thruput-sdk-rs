//! XDR serialization and contract-spec integration.
//!
//! # Why these are hand-written
//!
//! `crates/macros` hard-codes `::sdk::` paths in its expansions and offers no
//! crate-path override, so a crate that `sdk` depends on cannot use
//! `#[sdk::contract_type]` -- that would be a dependency cycle. The traits
//! themselves live in `groundhog-contract-spec` and `xdrs`, both below `sdk`,
//! so implementing them here is orphan-safe and needs no macro change.
//!
//! `CollectAuthHandles`, `ReceiveAuthHandles` and `MintableType` are owned by
//! `sdk` and so are implemented there, for this type. Together the two halves
//! give `U256` the same capabilities a `#[contract_type]` struct would have.
//!
//! # Wire format
//!
//! Eight `u32` limbs, **most significant first**, matching the `(high, low)`
//! convention xdrs already uses for `u128` (see `xdrs::int128`). Since XDR
//! integers are themselves big-endian, the encoded form is the value as 32
//! big-endian bytes -- which is also Ethereum's word order, and what a
//! non-Rust SDK would guess.
//!
//! Note that this is the reverse of the in-memory limb order, which is
//! least-significant-first.

use crate::{LIMBS, U256};
use groundhog_contract_spec as spec;
use xdrs::{
    ReadResult, ReadXDR, Reader, WriteResult, WriteXDR, Writer,
    WriterUnchecked,
};

// ---------------------------------------------------------------------------
// XDR
// ---------------------------------------------------------------------------

impl<'r> ReadXDR<'r> for U256 {
    #[inline]
    fn read_xdr<R: Reader<'r>>(r: &mut R) -> ReadResult<Self> {
        let mut limbs = [0u32; LIMBS];
        // Most significant first.
        let mut i = LIMBS;
        while i > 0 {
            i -= 1;
            limbs[i] = <u32 as ReadXDR>::read_xdr(r)?;
        }
        Ok(Self(limbs))
    }
}

impl WriteXDR for U256 {
    #[inline]
    fn write_xdr<F, W: Writer<F>>(&self, w: &mut W) -> WriteResult<()> {
        let mut i = LIMBS;
        while i > 0 {
            i -= 1;
            <u32 as WriteXDR>::write_xdr(&self.0[i], w)?;
        }
        Ok(())
    }

    #[inline]
    unsafe fn write_xdr_unchecked<F, W: WriterUnchecked<F>>(&self, w: &mut W) {
        let mut i = LIMBS;
        while i > 0 {
            i -= 1;
            // SAFETY: the caller guarantees space for all eight limbs, and a
            // `u32` has no XDR bounds to violate.
            unsafe {
                <u32 as WriteXDR>::write_xdr_unchecked(&self.0[i], w);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Contract spec
// ---------------------------------------------------------------------------

impl spec::ContractTypeName for U256 {
    const NAME: &'static str = "U256";
}

impl spec::ContractTypeSpec for U256 {
    const TYPE_DEF: spec::ContractTypeDefView<'static> =
        spec::ContractTypeDefView::Struct(spec::ContractStructDefView {
            name: <Self as spec::ContractTypeName>::NAME,
            fields: &[spec::ContractFieldView {
                name: "limbs",
                ty: spec::ContractTypeView::Array(
                    spec::ContractArrayTypeView {
                        element: &spec::ContractTypeView::U32,
                        len: LIMBS as u32,
                    },
                ),
            }],
        });
}

/// Linker anchor. `U256` has no named field types, so there is nothing
/// transitive to retain and this is the identity contribution -- but the
/// symbol has to exist and sit in the anchor section, because that is how
/// `ContractTypeRefSpec` wires typedef retention.
#[doc(hidden)]
#[inline(never)]
#[cfg_attr(
    target_arch = "riscv32",
    unsafe(link_section = "groundhogspecanchortextv0")
)]
pub fn __groundhog_spec_anchor_fn_u256() -> usize {
    0usize
}

impl spec::ContractTypeRefSpec for U256 {
    const INFO: spec::ContractTypeInfo<'static> = spec::ContractTypeInfo {
        ty: spec::ContractTypeView::Named(
            <Self as spec::ContractTypeName>::NAME,
        ),
    };

    const __GROUNDHOG_SPEC_ANCHOR_ROOT: fn() -> usize =
        __groundhog_spec_anchor_fn_u256;

    fn __groundhog_spec_anchor() -> usize {
        __groundhog_spec_anchor_fn_u256()
    }
}

// ---------------------------------------------------------------------------
// Raw spec fragment
//
// Mirrors what `#[derive(ContractTypeSpec)]` emits: the typedef, encoded once
// at compile time into a link section that the CLI extracts. Keep the section
// name in sync with `groundhog_spec_tool::RAW_SECTION_NAME`.
// ---------------------------------------------------------------------------

#[doc(hidden)]
pub const __GROUNDHOG_SPEC_FRAGMENT_U256: spec::SpecFragmentView<'static> =
    spec::SpecFragmentView::TypeDef(
        <U256 as spec::ContractTypeSpec>::TYPE_DEF,
    );

#[doc(hidden)]
const __GROUNDHOG_SPEC_RAW_LEN_U256: usize =
    __GROUNDHOG_SPEC_FRAGMENT_U256.const_txs_size();

// RV32 only: `#[used]` keeps the section live through ELF linking for CLI
// extraction. On wasm the custom section is retained without it, and `#[used]`
// there would force the fragment bytes into linear memory (tripping the
// spec-tool leak guard).
#[cfg(any(target_family = "wasm", target_arch = "riscv32"))]
#[doc(hidden)]
#[cfg_attr(
    any(target_family = "wasm", target_arch = "riscv32"),
    unsafe(link_section = "groundhogspecrawv1")
)]
#[cfg_attr(target_arch = "riscv32", used)]
static __GROUNDHOG_SPEC_RAW_TYPEDEF_FRAGMENT_U256: [u8;
    __GROUNDHOG_SPEC_RAW_LEN_U256] = {
    let mut w = xdrs::txs::ConstWriter::<__GROUNDHOG_SPEC_RAW_LEN_U256>::new();
    __GROUNDHOG_SPEC_FRAGMENT_U256.const_write_txs(&mut w);
    w.into_bytes()
};
