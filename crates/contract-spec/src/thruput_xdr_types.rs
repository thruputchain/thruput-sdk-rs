use thruput_xdr::{
    SyscallNo,
    contract::ContractEntrypointID,
    syserr::SysErr,
    types::{
        BlockHeight, ContractAddress, CryptographicScheme, Ed25519PublicKey,
        Ed25519Signature, MlDsa65PublicKey, MlDsa65Signature, PublicKey,
        SHA256Hash, Signature,
    },
};

use crate::{
    ContractTypeInfo, ContractTypeName, ContractTypeRefSpec, ContractTypeSpec,
    view,
};

impl ContractTypeRefSpec for ContractAddress {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::Address,
    };
}

impl ContractTypeRefSpec for BlockHeight {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::I64,
    };
}

impl ContractTypeRefSpec for SHA256Hash {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::Array(view::ContractArrayTypeView {
            element: &view::ContractTypeView::U8,
            len: 32,
        }),
    };
}

impl ContractTypeRefSpec for Ed25519PublicKey {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::Array(view::ContractArrayTypeView {
            element: &view::ContractTypeView::U8,
            len: 32,
        }),
    };
}

impl ContractTypeRefSpec for Ed25519Signature {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::Array(view::ContractArrayTypeView {
            element: &view::ContractTypeView::U8,
            len: 64,
        }),
    };
}

impl ContractTypeRefSpec for MlDsa65PublicKey {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::Array(view::ContractArrayTypeView {
            element: &view::ContractTypeView::U8,
            len: 1952,
        }),
    };
}

impl ContractTypeRefSpec for MlDsa65Signature {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::Array(view::ContractArrayTypeView {
            element: &view::ContractTypeView::U8,
            len: 3309,
        }),
    };
}

impl ContractTypeRefSpec for ContractEntrypointID {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::Array(view::ContractArrayTypeView {
            element: &view::ContractTypeView::U8,
            len: 8,
        }),
    };
}

impl ContractTypeRefSpec for SyscallNo {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::U64,
    };
}

impl ContractTypeRefSpec for SysErr {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::I32,
    };
}

/// Major version of the protocol unions this crate declares to the spec on
/// `thruput_xdr`'s behalf. Their wire layout is the protocol's, so the id
/// changes only if that layout does, not with this crate's releases.
const XDR_UNION_SPEC_MAJOR: &str = "1";

/// Declares a scheme-tagged protocol union (one case per
/// [`CryptographicScheme`], each a single byte-array field) to the contract
/// spec: the same `ContractTypeName` / `ContractTypeSpec` /
/// `ContractTypeRefSpec` impls and raw typedef fragment that
/// `#[derive(ContractTypeSpec)]` emits for an enum, which cannot be derived
/// here because the type is `thruput_xdr`'s.
macro_rules! scheme_union_spec {
    (
        $ty:ident,
        $fragment:ident,
        $raw_len:ident,
        $raw_fragment:ident,
        $anchor:ident,
        $(($case:literal, $scheme:expr, $field:literal, $field_ty:ty)),+ $(,)?
    ) => {
        impl ContractTypeName for $ty {
            const NAME: view::ContractSpecIdView<'static> =
                view::ContractSpecIdView {
                    crate_name: ::core::env!("CARGO_PKG_NAME"),
                    major: XDR_UNION_SPEC_MAJOR,
                    module_path: ::core::module_path!(),
                    declared: ::core::stringify!($ty),
                };
        }

        impl ContractTypeSpec for $ty {
            const TYPE_DEF: view::ContractTypeDefView<'static> =
                view::ContractTypeDefView::Union(view::ContractUnionDefView {
                    name: <$ty as ContractTypeName>::NAME,
                    cases: &[$(view::ContractUnionCaseView {
                        name: $case,
                        discriminant: $scheme.0,
                        case_type: view::ContractUnionCaseTypeView::Struct(&[
                            view::ContractFieldView {
                                name: $field,
                                ty: <$field_ty as ContractTypeRefSpec>::INFO.ty,
                            },
                        ]),
                    }),+],
                });
        }

        // The cases are byte arrays, which define no types of their own, so
        // there is nothing to retain beyond this union's own fragment.
        #[doc(hidden)]
        #[inline(never)]
        #[cfg_attr(
            target_arch = "riscv32",
            unsafe(link_section = "thruputspecanchortextv0")
        )]
        pub fn $anchor() -> usize {
            0
        }

        impl ContractTypeRefSpec for $ty {
            const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
                ty: view::ContractTypeView::Named(
                    <$ty as ContractTypeName>::NAME,
                ),
            };

            const __THRUPUT_SPEC_ANCHOR_ROOT: fn() -> usize = $anchor;

            fn __thruput_spec_anchor() -> usize {
                $anchor()
            }
        }

        // Read only by the RV32 static below and this crate's tests.
        #[doc(hidden)]
        #[cfg_attr(not(target_arch = "riscv32"), allow(dead_code))]
        pub const $fragment: crate::SpecFragmentView<'static> =
            crate::SpecFragmentView::TypeDef(
                <$ty as ContractTypeSpec>::TYPE_DEF,
            );

        #[cfg_attr(not(target_arch = "riscv32"), allow(dead_code))]
        const $raw_len: usize = $fragment.const_txs_size();

        // Same section and encoding as `#[derive(ContractTypeSpec)]`; keep
        // in sync with `thruput_spec_tool::RAW_SECTION_NAME`.
        #[cfg(target_arch = "riscv32")]
        #[doc(hidden)]
        #[unsafe(link_section = "groundhogspecrawv4")]
        #[used]
        static $raw_fragment: [u8; $raw_len] = {
            let mut w = ::xdrs::txs::ConstWriter::<$raw_len>::new();
            $fragment.const_write_txs(&mut w);
            w.into_bytes()
        };
    };
}

scheme_union_spec!(
    PublicKey,
    PUBLIC_KEY_SPEC_FRAGMENT,
    PUBLIC_KEY_SPEC_RAW_LEN,
    PUBLIC_KEY_SPEC_RAW_FRAGMENT,
    public_key_spec_anchor,
    (
        "Ed25519",
        CryptographicScheme::Ed25519,
        "ed25519",
        Ed25519PublicKey
    ),
    (
        "MlDsa65",
        CryptographicScheme::MlDsa65,
        "mldsa65",
        MlDsa65PublicKey
    ),
);

scheme_union_spec!(
    Signature,
    SIGNATURE_SPEC_FRAGMENT,
    SIGNATURE_SPEC_RAW_LEN,
    SIGNATURE_SPEC_RAW_FRAGMENT,
    signature_spec_anchor,
    (
        "Ed25519",
        CryptographicScheme::Ed25519,
        "ed25519",
        Ed25519Signature
    ),
    (
        "MlDsa65",
        CryptographicScheme::MlDsa65,
        "mldsa65",
        MlDsa65Signature
    ),
);

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;
    use view::{
        ContractArrayTypeView, ContractTypeDefView, ContractTypeView,
        ContractUnionCaseTypeView,
    };

    /// `(case name, discriminant, field name, array length)` for each case.
    fn cases(
        def: ContractTypeDefView<'static>,
    ) -> Vec<(&'static str, i32, &'static str, u32)> {
        let ContractTypeDefView::Union(union) = def else {
            panic!("expected a union, got {def:?}");
        };
        union
            .cases
            .iter()
            .map(|case| {
                let ContractUnionCaseTypeView::Struct([field]) =
                    case.case_type
                else {
                    panic!("expected one field, got {:?}", case.case_type);
                };
                let ContractTypeView::Array(ContractArrayTypeView {
                    element: ContractTypeView::U8,
                    len,
                }) = field.ty
                else {
                    panic!("expected a byte array, got {:?}", field.ty);
                };
                (case.name, case.discriminant, field.name, len)
            })
            .collect()
    }

    #[test]
    fn public_key_is_a_union_over_schemes() {
        assert_eq!(
            cases(<PublicKey as ContractTypeSpec>::TYPE_DEF),
            [
                ("Ed25519", 0, "ed25519", 32),
                ("MlDsa65", 1, "mldsa65", 1952)
            ]
        );
        let ContractTypeView::Named(id) = PublicKey::INFO.ty else {
            panic!("PublicKey must be referenced by name");
        };
        assert_eq!(id.declared, "PublicKey");
        assert!(PUBLIC_KEY_SPEC_RAW_LEN > 0);
        assert!(matches!(
            PUBLIC_KEY_SPEC_FRAGMENT,
            crate::SpecFragmentView::TypeDef(_)
        ));
    }

    #[test]
    fn signature_is_a_union_over_schemes() {
        assert_eq!(
            cases(<Signature as ContractTypeSpec>::TYPE_DEF),
            [
                ("Ed25519", 0, "ed25519", 64),
                ("MlDsa65", 1, "mldsa65", 3309)
            ]
        );
        let ContractTypeView::Named(id) = Signature::INFO.ty else {
            panic!("Signature must be referenced by name");
        };
        assert_eq!(id.declared, "Signature");
        assert!(SIGNATURE_SPEC_RAW_LEN > 0);
        assert!(matches!(
            SIGNATURE_SPEC_FRAGMENT,
            crate::SpecFragmentView::TypeDef(_)
        ));
    }
}
