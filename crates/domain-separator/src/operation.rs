//! Signed operations: the message an off-chain signature authorizes.
//!
//! A transaction signature covers a whole transaction and is checked by the
//! validator. An *operation* signature covers a structured application message
//! and is checked by a contract — a wallet validating a delegated action, a
//! token honouring a spend approval it never saw submitted. The signer is not
//! the submitter, so everything that bounds the signature's authority has to be
//! inside the signed bytes.
//!
//! Nothing here reaches the node. The contract verifies the signature with the
//! `VerifySignature` and `SHA256` syscalls it already has, then mints auth
//! tokens with `MintAuthToken`. `hogd` enforces the token tree it already
//! enforces and never parses an operation preimage.
//!
//! # What binds a signature
//!
//! [`SignedOp`]'s fields each close one replay path, and none are optional:
//!
//! | Field | Stops the signature being replayed |
//! |---|---|
//! | `network` | on another network carrying the same address |
//! | `verifying_contract` | against a different contract |
//! | `T::TYPE_DEF` | as a different kind of message |
//! | `nonce` / `valid_until` | again, or forever |
//!
//! The third has no counterpart in
//! [`transaction_signature_preimage`](crate::transaction_signature_preimage),
//! and it is the one a hand-rolled scheme forgets. CXS concatenates struct
//! fields with no framing — no type tag, no field count, no length prefix — so
//! two application structs with the same field types encode to identical bytes.
//! `Grant { amount: u64 }` and `Revoke { id: u64 }` are the same message. It
//! bites hardest with auth tokens, whose `Signed<T>` fields encode to *nothing*
//! at all: two authorizations differing only in which contract their children
//! are addressed to have byte-identical payloads.
//!
//! # Where the type binding comes from
//!
//! Not from an identifier this crate invents — from the type itself.
//! `#[contract_type]` already emits
//! [`ContractTypeSpec::TYPE_DEF`], a `const` carrying the type's name, its
//! field names and its field types, and that whole definition goes into the
//! preimage.
//!
//! This is EIP-712's `encodeType`, with its worst footgun removed: the
//! definition is *generated from the struct* rather than hand-written alongside
//! it, so it cannot drift. It is also structural rather than nominal — adding
//! or renaming a field of `T` changes the preimage and retires every old
//! signature, which a name or a hash of a name would not do.
//!
//! It also means there is no check to skip. A verifier reconstructs the
//! preimage from the `T` it expects; a message of some other shape simply
//! fails signature verification.
//!
//! ## Shallowness, and why it is fine here
//!
//! `TYPE_DEF` is shallow: a nested `#[contract_type]` appears as
//! `ContractTypeView::Named(NAME)`, not as an inlined definition. EIP-712 by
//! contrast appends referenced struct definitions transitively — because on
//! Ethereum a proxy can swap the code behind a stable `verifyingContract`, so
//! the same address really can reinterpret a type.
//!
//! Groundhog has no such premise. An address is derived from the contract hash
//! and there is no upgrade syscall, so one address means one code image and one
//! definition of every type it names, permanently. `verifying_contract` in the
//! preimage already binds a signature to that single interpretation. See
//! `tests::type_def_names_nested_types_rather_than_inlining_them`.
//!
//! The bound excludes bare tuples and primitives, which have a
//! `ContractTypeRefSpec` but no `ContractTypeSpec`. That is deliberate: an
//! anonymous shape has no identity to bind. Tuple structs are fine — the macro
//! handles unnamed fields — they just have to be named types.

use groundhog_contract_spec::{ContractTypeSpec, view::ContractTypeDefView};
use groundhog_xdr::types::{
    BlockHeight, ContractAddress, NetworkHash, SHA256Hash,
};
use xdrs::{WriteResult, WriteXDR, Writer, WriterUnchecked};

use crate::backend::Sha256Backend;
use crate::{DomainSeparator, SIGNED_OP};

/// The preimage layout, spelled exactly once.
///
/// `SignedOp`'s `WriteXDR` delegates here, so the bytes a signature covers and
/// the bytes a digest is taken over cannot drift apart.
#[derive(WriteXDR)]
struct OpPreimage<'a, T: WriteXDR + ?Sized> {
    network: &'a NetworkHash,
    verifying_contract: &'a ContractAddress,
    type_def: ContractTypeDefView<'a>,
    nonce: u64,
    valid_until: &'a BlockHeight,
    body: &'a T,
}

/// A structured message authorizing an on-chain action, signed off-chain.
///
/// `T` is the application's `#[contract_type]` payload. Its
/// [`ContractTypeSpec::TYPE_DEF`] goes into the preimage, which is what keeps a
/// signature over one message shape from being a signature over another — see
/// the module docs.
///
/// ```
/// # use groundhog_domain_separator::{SignedOp, Sha2};
/// # use groundhog_xdr::types::{BlockHeight, ContractAddress, NetworkHash, SHA256Hash};
/// # use groundhog_contract_spec::{ContractTypeSpec, view::*};
/// # use xdrs::WriteXDR;
/// # #[derive(WriteXDR)]
/// # struct Transfer { amount: u64 }
/// # impl ContractTypeSpec for Transfer {
/// #     const TYPE_DEF: ContractTypeDefView<'static> =
/// #         ContractTypeDefView::Struct(ContractStructDefView {
/// #             name: "Transfer",
/// #             fields: &[ContractFieldView { name: "amount", ty: ContractTypeView::U64 }],
/// #         });
/// # }
/// let op = SignedOp {
///     network: NetworkHash::new(SHA256Hash::new([0x5a; 32])),
///     verifying_contract: ContractAddress::from([0x03; 32]),
///     nonce: 1,
///     valid_until: BlockHeight::new(1_000),
///     body: &Transfer { amount: 42 },
/// };
/// let signature_covers = op.preimage();
///
/// // Any header change is a different message.
/// let replayed = SignedOp { nonce: 2, ..op };
/// assert_ne!(signature_covers, replayed.preimage());
/// ```
pub struct SignedOp<'a, T: WriteXDR + ContractTypeSpec + ?Sized> {
    /// The network this operation is valid on. Without it a signature is
    /// replayable on any chain that carries the same verifying contract —
    /// pubkey account addresses have no network in their derivation.
    pub network: NetworkHash,
    /// The contract entitled to act on this operation. Without it a signature
    /// is replayable against any other contract that accepts the same body.
    pub verifying_contract: ContractAddress,
    /// Per-signer counter the verifying contract consumes, so one signature
    /// authorizes one execution.
    pub nonce: u64,
    /// Last block height at which this operation may be executed. A signature
    /// that never expires is a signature the signer cannot retract.
    pub valid_until: BlockHeight,
    /// The application payload. Its type's definition is bound into the
    /// preimage alongside its value.
    pub body: &'a T,
}

impl<T: WriteXDR + ContractTypeSpec + ?Sized> SignedOp<'_, T> {
    fn repr(&self) -> OpPreimage<'_, T> {
        OpPreimage {
            network: &self.network,
            verifying_contract: &self.verifying_contract,
            type_def: T::TYPE_DEF,
            nonce: self.nonce,
            valid_until: &self.valid_until,
            body: self.body,
        }
    }

    /// The bytes a signature covers: `cxs(SIGNED_OP) || cxs(self)`.
    ///
    /// **This is what gets signed**, not [`hash`](Self::hash)'s output.
    /// Ed25519 hashes its own message, so pre-hashing would add an on-chain
    /// SHA-256 and a second convention for no cryptographic gain — the same
    /// call
    /// [`transaction_signature_preimage`](crate::transaction_signature_preimage)
    /// makes.
    #[cfg(feature = "alloc")]
    #[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
    pub fn preimage(&self) -> alloc::vec::Vec<u8> {
        self.preimage_in(&SIGNED_OP)
    }

    /// [`preimage`](Self::preimage) under an application's own separator.
    ///
    /// Declare one with [`domain_separator!`](crate::domain_separator) when you
    /// want a signing domain you can retire on your own schedule. `T::TYPE_DEF`
    /// already separates operations *within* a domain, so this is about
    /// lifecycle, not collision.
    #[cfg(feature = "alloc")]
    #[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
    pub fn preimage_in(
        &self,
        separator: &DomainSeparator,
    ) -> alloc::vec::Vec<u8> {
        separator.preimage(&self.repr())
    }

    /// `sha256` of [`preimage`](Self::preimage) — a stable *identifier* for
    /// this operation.
    ///
    /// Use it to key nonce records, log, or deduplicate. Do not sign it.
    #[cfg(feature = "alloc")]
    #[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
    pub fn hash<H: Sha256Backend>(&self) -> SHA256Hash {
        self.hash_in::<H>(&SIGNED_OP)
    }

    /// [`hash`](Self::hash) under an application's own separator.
    #[cfg(feature = "alloc")]
    #[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
    pub fn hash_in<H: Sha256Backend>(
        &self,
        separator: &DomainSeparator,
    ) -> SHA256Hash {
        separator.hash::<H, _>(&self.repr())
    }

    /// Allocation-free [`hash`](Self::hash), for contracts.
    ///
    /// # Safety
    ///
    /// Same contract as [`DomainSeparator::hash_into`]: `scratch` must hold
    /// `SIGNED_OP.as_bytes().len()` plus the worst-case CXS size of the
    /// preimage — [`SIGNED_OP_HEADER_MAX_LEN`] plus `T::TYPE_DEF`'s encoding
    /// plus the body's — and `body` must respect its declared XDR bounds.
    pub unsafe fn hash_into<H: Sha256Backend>(
        &self,
        scratch: &mut [core::mem::MaybeUninit<u8>],
    ) -> SHA256Hash {
        // SAFETY: forwarded caller contract.
        unsafe { SIGNED_OP.hash_into::<H, _>(&self.repr(), scratch) }
    }
}

/// Writes the preimage layout. Delegates to the one definition in `OpPreimage`.
impl<T: WriteXDR + ContractTypeSpec + ?Sized> WriteXDR for SignedOp<'_, T> {
    fn write_xdr<F, W: Writer<F>>(&self, w: &mut W) -> WriteResult<()> {
        self.repr().write_xdr(w)
    }

    unsafe fn write_xdr_unchecked<F, W: WriterUnchecked<F>>(&self, w: &mut W) {
        // SAFETY: forwarded caller contract.
        unsafe { self.repr().write_xdr_unchecked(w) }
    }
}

/// Bytes of fixed-width header before anything varies: two 32-byte hashes.
/// `type_def`, `nonce`, `valid_until` and the body all follow and are all
/// variable-length but self-delimiting.
pub const SIGNED_OP_FIXED_PREFIX_LEN: usize = 32 + 32;

/// Worst-case header length excluding `type_def` and the body:
/// [`SIGNED_OP_FIXED_PREFIX_LEN`] plus the widest CXS encoding of a `u64` nonce
/// and an `i64` block height (9 bytes each).
pub const SIGNED_OP_HEADER_MAX_LEN: usize = SIGNED_OP_FIXED_PREFIX_LEN + 9 + 9;

#[cfg(all(test, feature = "std", feature = "sha2"))]
mod tests {
    use super::*;
    use crate::backend::Sha2;
    use groundhog_contract_spec::view::{
        ContractFieldView, ContractStructDefView, ContractTypeView,
    };

    /// Stands in for a `#[contract_type]` payload; the macro emits exactly this
    /// pair of impls.
    #[derive(WriteXDR)]
    struct Grant {
        amount: u64,
    }

    impl ContractTypeSpec for Grant {
        const TYPE_DEF: ContractTypeDefView<'static> =
            ContractTypeDefView::Struct(ContractStructDefView {
                name: "Grant",
                fields: &[ContractFieldView {
                    name: "amount",
                    ty: ContractTypeView::U64,
                }],
            });
    }

    /// Byte-identical to `Grant` as a *value* — same field type, and CXS has no
    /// framing to tell them apart. Only `TYPE_DEF` separates them.
    #[derive(WriteXDR)]
    struct Revoke {
        id: u64,
    }

    impl ContractTypeSpec for Revoke {
        const TYPE_DEF: ContractTypeDefView<'static> =
            ContractTypeDefView::Struct(ContractStructDefView {
                name: "Revoke",
                fields: &[ContractFieldView {
                    name: "id",
                    ty: ContractTypeView::U64,
                }],
            });
    }

    /// `Grant` with a field added — the schema-evolution case a name-based or
    /// hash-of-name binding would silently keep accepting.
    #[derive(WriteXDR)]
    struct GrantV2 {
        amount: u64,
        recipient: u64,
    }

    impl ContractTypeSpec for GrantV2 {
        const TYPE_DEF: ContractTypeDefView<'static> =
            ContractTypeDefView::Struct(ContractStructDefView {
                name: "Grant",
                fields: &[
                    ContractFieldView {
                        name: "amount",
                        ty: ContractTypeView::U64,
                    },
                    ContractFieldView {
                        name: "recipient",
                        ty: ContractTypeView::U64,
                    },
                ],
            });
    }

    fn op<T: WriteXDR + ContractTypeSpec>(body: &T) -> SignedOp<'_, T> {
        SignedOp {
            network: NetworkHash::new(SHA256Hash::new([0x5a; 32])),
            verifying_contract: ContractAddress::from([0x03; 32]),
            nonce: 1,
            valid_until: BlockHeight::new(1_000),
            body,
        }
    }

    /// Pins the layout the TypeScript client has to reproduce byte for byte.
    #[test]
    fn layout_is_prefix_then_type_def_then_varints_then_body() {
        let body = Grant { amount: 42 };
        let encoded = xdrs::cxs::encode(&op(&body)).unwrap();

        assert_eq!(&encoded[0..32], &[0x5au8; 32], "network");
        assert_eq!(&encoded[32..64], &[0x03u8; 32], "verifying_contract");
        assert_eq!(SIGNED_OP_FIXED_PREFIX_LEN, 64);

        let mut tail = alloc::vec::Vec::new();
        tail.extend_from_slice(
            &xdrs::cxs::encode(&Grant::TYPE_DEF).unwrap(),
        );
        tail.extend_from_slice(&xdrs::cxs::encode(&1u64).unwrap());
        tail.extend_from_slice(
            &xdrs::cxs::encode(&BlockHeight::new(1_000)).unwrap(),
        );
        tail.extend_from_slice(&xdrs::cxs::encode(&body).unwrap());
        assert_eq!(&encoded[SIGNED_OP_FIXED_PREFIX_LEN..], tail.as_slice());
    }

    /// The whole point. Two payloads that encode to the same bytes must not
    /// produce the same message.
    #[test]
    fn type_def_separates_identically_encoded_bodies() {
        let grant = Grant { amount: 500 };
        let revoke = Revoke { id: 500 };
        assert_eq!(
            xdrs::cxs::encode(&grant).unwrap(),
            xdrs::cxs::encode(&revoke).unwrap(),
            "test is meaningless unless the values encode identically"
        );
        assert_ne!(op(&grant).preimage(), op(&revoke).preimage());
    }

    /// Adding a field retires old signatures automatically. A binding derived
    /// from the type *name* would not catch this — both are named "Grant".
    #[test]
    fn adding_a_field_changes_the_message() {
        assert_eq!(
            Grant::TYPE_DEF.symbol(),
            GrantV2::TYPE_DEF.symbol(),
            "both are named Grant; only the structure differs"
        );
        let v1 = Grant { amount: 7 };
        let v2 = GrantV2 {
            amount: 7,
            recipient: 0,
        };
        assert_ne!(op(&v1).preimage(), op(&v2).preimage());
    }

    /// `TYPE_DEF` is shallow: a nested `#[contract_type]` appears as
    /// `ContractTypeView::Named(NAME)`, not as an inlined definition —
    /// transitive definitions are resolved by spec-tool at link time, not in
    /// this const. This asserts that shape, because it surprises people.
    ///
    /// It has no security consequence *here*. For an old signature to be
    /// reinterpreted, one `verifying_contract` address would have to hold two
    /// different definitions of the nested type. It cannot: an address is
    /// derived from the contract hash (`get_deploy_address(nonce,
    /// contract_hash)`) and there is no upgrade syscall, so a given address has
    /// one code image, and one definition of each type, for its whole life.
    /// `verifying_contract` in the preimage already closes the door.
    ///
    /// EIP-712 needs its transitive `encodeType` precisely because Ethereum
    /// *does* let a proxy swap the code behind a stable `verifyingContract`.
    /// Groundhog's addressing removes the premise, not just the symptom.
    #[test]
    fn type_def_names_nested_types_rather_than_inlining_them() {
        #[derive(WriteXDR)]
        struct Inner {
            debit: u64,
        }
        impl ContractTypeSpec for Inner {
            const TYPE_DEF: ContractTypeDefView<'static> =
                ContractTypeDefView::Struct(ContractStructDefView {
                    name: "Inner",
                    fields: &[ContractFieldView {
                        name: "debit",
                        ty: ContractTypeView::U64,
                    }],
                });
        }

        #[derive(WriteXDR)]
        struct Outer {
            inner: Inner,
        }
        impl ContractTypeSpec for Outer {
            // What `#[contract_type]` emits for a nested contract-type field:
            // `<Inner as ContractTypeRefSpec>::INFO.ty`, which is a `Named` ref.
            const TYPE_DEF: ContractTypeDefView<'static> =
                ContractTypeDefView::Struct(ContractStructDefView {
                    name: "Outer",
                    fields: &[ContractFieldView {
                        name: "inner",
                        ty: ContractTypeView::Named("Inner"),
                    }],
                });
        }

        let ContractTypeDefView::Struct(outer) = Outer::TYPE_DEF else {
            panic!("Outer is a struct");
        };
        assert_eq!(
            outer.fields[0].ty,
            ContractTypeView::Named("Inner"),
            "nested types are referenced by name, not inlined"
        );
        // Inner's own field names are therefore absent from Outer's definition.
        assert!(
            !xdrs::cxs::encode(&Outer::TYPE_DEF)
                .unwrap()
                .windows(5)
                .any(|w| w == b"debit"),
        );
    }

    /// A tuple struct carrying `#[contract_type]` is fine — the macro handles
    /// `Fields::Unnamed`. What has no `ContractTypeSpec` impl, and so cannot be
    /// signed at all, is a *bare* tuple or primitive: an anonymous shape has no
    /// identity to bind, so the bound rejects it rather than silently signing
    /// something indistinguishable from any other pair of integers.
    #[test]
    fn tuple_structs_carry_a_type_def() {
        #[derive(WriteXDR)]
        struct Pair(u64, u64);
        impl ContractTypeSpec for Pair {
            const TYPE_DEF: ContractTypeDefView<'static> =
                ContractTypeDefView::Struct(ContractStructDefView {
                    name: "Pair",
                    fields: &[
                        ContractFieldView {
                            name: "0",
                            ty: ContractTypeView::U64,
                        },
                        ContractFieldView {
                            name: "1",
                            ty: ContractTypeView::U64,
                        },
                    ],
                });
        }

        let pair = Pair(1, 2);
        let grant = Grant { amount: 1 };
        // Distinct shapes stay distinct even though both start with a u64.
        assert_ne!(op(&pair).preimage(), op(&grant).preimage());
    }

    #[test]
    fn each_header_field_changes_the_message() {
        let body = Grant { amount: 42 };
        let base = op(&body).preimage();

        let variants = [
            (
                "network",
                SignedOp {
                    network: NetworkHash::new(SHA256Hash::new([0xa5; 32])),
                    ..op(&body)
                }
                .preimage(),
            ),
            (
                "verifying_contract",
                SignedOp {
                    verifying_contract: ContractAddress::from([0x04; 32]),
                    ..op(&body)
                }
                .preimage(),
            ),
            ("nonce", SignedOp { nonce: 2, ..op(&body) }.preimage()),
            (
                "valid_until",
                SignedOp {
                    valid_until: BlockHeight::new(1_001),
                    ..op(&body)
                }
                .preimage(),
            ),
        ];
        for (label, variant) in variants {
            assert_ne!(base, variant, "{label} did not change the message");
        }
    }

    #[test]
    fn preimage_is_separator_then_struct() {
        let body = Grant { amount: 42 };
        let o = op(&body);
        let preimage = o.preimage();
        let prefix = SIGNED_OP.as_bytes();
        assert_eq!(&preimage[..prefix.len()], prefix);
        assert_eq!(
            &preimage[prefix.len()..],
            xdrs::cxs::encode(&o).unwrap().as_slice()
        );
    }

    #[test]
    fn hash_is_sha256_of_the_preimage() {
        let body = Grant { amount: 42 };
        let o = op(&body);
        assert_eq!(
            o.hash::<Sha2>(),
            SHA256Hash::new(Sha2::sha256(&o.preimage()))
        );
    }

    #[test]
    fn application_separator_is_a_distinct_domain() {
        crate::domain_separator! {
            const MY_APP = "MyApp" version "1";
        }
        let body = Grant { amount: 42 };
        let o = op(&body);
        assert_ne!(o.preimage(), o.preimage_in(&MY_APP));
    }

    #[test]
    fn hash_into_matches_hash() {
        use core::mem::MaybeUninit;
        let body = Grant { amount: 42 };
        let o = op(&body);
        let mut scratch = [MaybeUninit::<u8>::uninit(); 512];
        assert_eq!(
            unsafe { o.hash_into::<Sha2>(&mut scratch) },
            o.hash::<Sha2>()
        );
    }
}
