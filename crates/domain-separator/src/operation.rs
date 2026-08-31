//! Signed operations: the message an off-chain signature authorizes.
//!
//! A transaction signature covers a whole transaction and is checked by the
//! validator. An *operation* signature covers a structured application message
//! and is checked by a contract — a wallet validating a delegated action, a
//! token honouring a spend approval it never saw submitted. The signer is not
//! the submitter, so everything that bounds the signature's authority has to be
//! inside the signed bytes.
//!
//! [`SignedOp`] is that envelope. Its four header fields each close one replay
//! path, and none of them are optional:
//!
//! | Field | Stops the signature being replayed |
//! |---|---|
//! | `network` | on another network carrying the same address |
//! | `verifying_contract` | against a different contract |
//! | `type_id` | as a different kind of message |
//! | `nonce` / `valid_until` | again, or forever |
//!
//! `type_id` is the one with no counterpart in
//! [`transaction_signature_preimage`](crate::transaction_signature_preimage),
//! and it is the field a hand-rolled scheme forgets. CXS concatenates struct
//! fields with no framing — no type tag, no field count, no length prefix — so
//! two application structs with the same field types encode to identical bytes.
//! `Grant { amount: u64 }` and `Revoke { id: u64 }` are the same message. It
//! bites hardest with auth tokens, whose `Signed<T>` fields encode to *nothing*
//! at all: two authorizations differing only in which contract their children
//! are addressed to have byte-identical payloads. Binding the type into the
//! preimage is what keeps a signature over one from being a signature over the
//! other.

use groundhog_xdr::types::{
    BlockHeight, ContractAddress, NetworkHash, SHA256Hash,
};
use xdrs::{ReadXDR, WriteXDR};

use crate::backend::Sha256Backend;
use crate::{DomainSeparator, SIGNED_OP};

/// The 8-byte discriminator for a kind of signed operation.
///
/// Derived exactly like a contract entrypoint ID —
/// `sha256("Interface::MajorVersion::name")[0..8]` — by
/// `groundhog_contract_util::operation_type_id` on the host. Naming an
/// operation after the method it authorizes therefore gives it that method's
/// ID, which is a convenience rather than a requirement: the two are never
/// compared, and live in different positions (one routes a call, one sits
/// inside a hash preimage).
///
/// Contracts should hold this as a `const` literal rather than deriving it at
/// runtime, the way generated entrypoint IDs do — a SHA-256 per verification is
/// pure gas for a value fixed at compile time.
///
/// ```
/// use groundhog_domain_separator::OperationTypeId;
///
/// const TRANSFER: OperationTypeId = OperationTypeId::from_u64(0x1122334455667788);
/// assert_eq!(TRANSFER.as_bytes(), &[0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88]);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[derive(WriteXDR, ReadXDR)]
pub struct OperationTypeId {
    inner: [u8; 8],
}

impl OperationTypeId {
    /// Wraps the 8 derived bytes.
    pub const fn new(inner: [u8; 8]) -> Self {
        Self { inner }
    }

    /// Wraps a big-endian literal, so a derived ID reads in source the same way
    /// it prints in hex.
    pub const fn from_u64(value: u64) -> Self {
        Self {
            inner: value.to_be_bytes(),
        }
    }

    /// The 8 derived bytes.
    pub const fn as_bytes(&self) -> &[u8; 8] {
        &self.inner
    }

    /// The 8 derived bytes as a big-endian integer — the inverse of
    /// [`from_u64`](Self::from_u64).
    pub const fn to_u64(self) -> u64 {
        u64::from_be_bytes(self.inner)
    }
}

impl From<[u8; 8]> for OperationTypeId {
    fn from(inner: [u8; 8]) -> Self {
        Self::new(inner)
    }
}

/// The message a signed operation's signature covers:
///
/// ```text
/// sha256(
///     cxs(DomainSeparator("SignedOp::1"))
///  || network || verifying_contract || type_id || nonce || valid_until
///  || cxs(body)
/// )
/// ```
///
/// The first three fields are fixed-width — 32, 32 and 8 bytes, so
/// [`SIGNED_OP_FIXED_PREFIX_LEN`] of header before anything varies — and the
/// two integers after them are CXS varints, which are self-delimiting. The
/// body's start is therefore unambiguous without CXS framing it, the same
/// reasoning that lets
/// [`transaction_signature_preimage`](crate::transaction_signature_preimage)
/// concatenate a network hash and a transaction.
///
/// Varints are canonical here rather than merely deterministic, because *this
/// preimage is only ever constructed, never parsed*. Signer and verifier both
/// build it from typed fields and re-encode; no attacker-supplied byte string
/// is ever accepted as a preimage. A verifier that instead took preimage bytes
/// over the wire would have to reject non-canonical varints itself.
///
/// `body` is whatever the application signs, and is the only part it chooses.
/// It is CXS-encoded bytes, not a typed value, and it is deliberately not
/// generic: `&[u8]` encodes as a length-prefixed var-opaque, while a typed `T`
/// would inline its fields, so the same logical body would hash two different
/// ways depending on which side held it as bytes. A signer with a struct
/// encodes it first (`xdrs::cxs::encode(&my_struct)`) and a verifier — which
/// receives it as bytes over the wire anyway — passes them straight through.
/// The length prefix also makes the body's extent explicit rather than merely
/// implied by the end of the preimage.
///
/// Reach for [`hash`](Self::hash) to sign or verify:
///
/// ```
/// # use groundhog_domain_separator::{OperationTypeId, SignedOp, Sha2};
/// # use groundhog_xdr::types::{BlockHeight, ContractAddress, NetworkHash, SHA256Hash};
/// const TRANSFER: OperationTypeId = OperationTypeId::from_u64(0x1122334455667788);
///
/// // The application encodes its own payload first.
/// let body = xdrs::cxs::encode(&(42u64, 7u64)).unwrap();
///
/// let op = SignedOp {
///     network: NetworkHash::new(SHA256Hash::new([0x5a; 32])),
///     verifying_contract: ContractAddress::from([0x03; 32]),
///     type_id: TRANSFER,
///     nonce: 1,
///     valid_until: BlockHeight::new(1_000),
///     body: &body,
/// };
/// let digest = op.hash::<Sha2>();
///
/// // Any header change is a different message.
/// let replayed = SignedOp { nonce: 2, ..op.clone() };
/// assert_ne!(digest, replayed.hash::<Sha2>());
/// ```
#[derive(Clone, Debug, WriteXDR)]
pub struct SignedOp<'a> {
    /// The network this operation is valid on. Without it a signature is
    /// replayable on any chain that carries the same verifying contract.
    pub network: NetworkHash,
    /// The contract entitled to act on this operation. Without it a signature
    /// is replayable against any other contract that accepts the same body.
    pub verifying_contract: ContractAddress,
    /// Which kind of operation this is. Without it a signature over one message
    /// shape is a signature over every message that encodes the same way.
    pub type_id: OperationTypeId,
    /// Per-signer counter the verifying contract consumes, so one signature
    /// authorizes one execution.
    pub nonce: u64,
    /// Last block height at which this operation may be executed. A signature
    /// that never expires is a signature the signer cannot retract.
    pub valid_until: BlockHeight,
    /// The CXS-encoded application payload, written last as a length-prefixed
    /// var-opaque.
    pub body: &'a [u8],
}

impl SignedOp<'_> {
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
    #[cfg(feature = "alloc")]
    #[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
    pub fn preimage_in(
        &self,
        separator: &DomainSeparator,
    ) -> alloc::vec::Vec<u8> {
        separator.preimage(self)
    }

    /// `sha256(cxs(SIGNED_OP) || cxs(self))` — a stable *identifier* for this
    /// operation.
    ///
    /// Use it to key nonce records, log, or deduplicate. Do not sign it:
    /// [`preimage`](Self::preimage) is the message. It is also what the
    /// cross-SDK vectors pin, since a digest compares more cheaply than a
    /// variable-length preimage.
    #[cfg(feature = "alloc")]
    #[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
    pub fn hash<H: Sha256Backend>(&self) -> SHA256Hash {
        self.hash_in::<H>(&SIGNED_OP)
    }

    /// [`hash`](Self::hash) under an application's own separator instead of the
    /// shared [`SIGNED_OP`].
    ///
    /// Declare one with
    /// [`domain_separator!`](crate::domain_separator) when you want a signing
    /// domain you can retire on your own schedule — bumping your separator's
    /// version invalidates every signature under it, which the shared separator
    /// cannot do for one application. `type_id` already separates operations
    /// *within* a domain, so this is about lifecycle, not collision.
    #[cfg(feature = "alloc")]
    #[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
    pub fn hash_in<H: Sha256Backend>(
        &self,
        separator: &DomainSeparator,
    ) -> SHA256Hash {
        separator.hash::<H, _>(self)
    }

    /// Allocation-free [`hash`](Self::hash), for contracts.
    ///
    /// # Safety
    ///
    /// Same contract as
    /// [`DomainSeparator::hash_into`]: `scratch` must hold
    /// `SIGNED_OP.as_bytes().len()` plus the worst-case CXS size of `self`
    /// ([`SIGNED_OP_HEADER_MAX_LEN`] plus the body's), and `body` must respect
    /// its declared XDR bounds. Size it from `xdrs::cxs::ConstMaxSize`, not by
    /// inspection.
    pub unsafe fn hash_into<H: Sha256Backend>(
        &self,
        scratch: &mut [core::mem::MaybeUninit<u8>],
    ) -> SHA256Hash {
        // SAFETY: forwarded caller contract.
        unsafe { SIGNED_OP.hash_into::<H, _>(self, scratch) }
    }
}

/// Bytes of fixed-width header before the first varint: two 32-byte hashes and
/// the 8-byte type ID.
pub const SIGNED_OP_FIXED_PREFIX_LEN: usize = 32 + 32 + 8;

/// Worst-case encoded header length: [`SIGNED_OP_FIXED_PREFIX_LEN`] plus the
/// widest CXS encoding of a `u64` nonce and an `i64` block height (9 bytes
/// each). Size a [`SignedOp::hash_into`] scratch buffer from this plus the
/// body's `xdrs::cxs::ConstMaxSize::MAX_SIZE`, plus the separator prefix.
pub const SIGNED_OP_HEADER_MAX_LEN: usize = SIGNED_OP_FIXED_PREFIX_LEN + 9 + 9;

#[cfg(all(test, feature = "std", feature = "sha2"))]
mod tests {
    use super::*;
    use crate::backend::Sha2;

    const TYPE_A: OperationTypeId = OperationTypeId::from_u64(0xaaaa_aaaa_aaaa_aaaa);
    const TYPE_B: OperationTypeId = OperationTypeId::from_u64(0xbbbb_bbbb_bbbb_bbbb);

    /// A CXS-encoded application payload, as a signer would produce it.
    fn body_bytes(value: u64) -> alloc::vec::Vec<u8> {
        xdrs::cxs::encode(&value).unwrap()
    }

    fn op(body: &[u8]) -> SignedOp<'_> {
        SignedOp {
            network: NetworkHash::new(SHA256Hash::new([0x5a; 32])),
            verifying_contract: ContractAddress::from([0x03; 32]),
            type_id: TYPE_A,
            nonce: 1,
            valid_until: BlockHeight::new(1_000),
            body,
        }
    }

    /// Pins the layout the C++ validator and the TypeScript client have to
    /// reproduce byte for byte.
    #[test]
    fn layout_is_fixed_prefix_then_varints_then_body() {
        let body = body_bytes(42);
        let encoded = xdrs::cxs::encode(&op(&body)).unwrap();

        assert_eq!(&encoded[0..32], &[0x5au8; 32], "network");
        assert_eq!(&encoded[32..64], &[0x03u8; 32], "verifying_contract");
        assert_eq!(&encoded[64..72], TYPE_A.as_bytes(), "type_id");
        assert_eq!(SIGNED_OP_FIXED_PREFIX_LEN, 72);

        // Everything after the fixed prefix is CXS-varint: nonce, then expiry,
        // then the body, each encoded exactly as it would be standalone.
        let mut tail = alloc::vec::Vec::new();
        tail.extend_from_slice(&xdrs::cxs::encode(&1u64).unwrap());
        tail.extend_from_slice(
            &xdrs::cxs::encode(&BlockHeight::new(1_000)).unwrap(),
        );
        tail.extend_from_slice(&xdrs::cxs::encode(&body.as_slice()).unwrap());
        assert_eq!(&encoded[SIGNED_OP_FIXED_PREFIX_LEN..], tail.as_slice());
    }

    /// The worst-case constant has to actually bound the widest header, or a
    /// `hash_into` scratch buffer sized from it is a buffer overrun.
    #[test]
    fn header_max_len_bounds_the_widest_header() {
        let widest = SignedOp {
            network: NetworkHash::new(SHA256Hash::new([0xff; 32])),
            verifying_contract: ContractAddress::from([0xff; 32]),
            type_id: TYPE_A,
            nonce: u64::MAX,
            valid_until: BlockHeight::new(i64::MAX),
            body: &[],
        };
        let encoded = xdrs::cxs::encode(&widest).unwrap();
        // The empty body still costs its own length byte.
        assert!(
            encoded.len() <= SIGNED_OP_HEADER_MAX_LEN + 1,
            "header encoded to {} bytes, bound is {}",
            encoded.len(),
            SIGNED_OP_HEADER_MAX_LEN,
        );
    }

    #[test]
    fn hash_is_separator_prefixed() {
        let body = body_bytes(42);
        let o = op(&body);
        assert_eq!(o.hash::<Sha2>(), SIGNED_OP.hash::<Sha2, _>(&o));
    }

    /// Every header field has to be load-bearing, or it is not closing the
    /// replay path it claims to.
    #[test]
    fn each_header_field_changes_the_digest() {
        let body = body_bytes(42);
        let base = op(&body).hash::<Sha2>();

        let other_network = SignedOp {
            network: NetworkHash::new(SHA256Hash::new([0xa5; 32])),
            ..op(&body)
        };
        let other_contract = SignedOp {
            verifying_contract: ContractAddress::from([0x04; 32]),
            ..op(&body)
        };
        let other_type = SignedOp {
            type_id: TYPE_B,
            ..op(&body)
        };
        let other_nonce = SignedOp {
            nonce: 2,
            ..op(&body)
        };
        let other_expiry = SignedOp {
            valid_until: BlockHeight::new(1_001),
            ..op(&body)
        };

        for (label, variant) in [
            ("network", other_network.hash::<Sha2>()),
            ("verifying_contract", other_contract.hash::<Sha2>()),
            ("type_id", other_type.hash::<Sha2>()),
            ("nonce", other_nonce.hash::<Sha2>()),
            ("valid_until", other_expiry.hash::<Sha2>()),
        ] {
            assert_ne!(base, variant, "{label} did not change the digest");
        }
    }

    /// The reason `type_id` exists. These two bodies are byte-identical under
    /// CXS — only the type ID tells the signatures apart.
    #[test]
    fn type_id_separates_identically_encoded_bodies() {
        let grant = body_bytes(500);
        let revoke = body_bytes(500);
        assert_eq!(grant, revoke);

        let as_grant = SignedOp {
            type_id: TYPE_A,
            ..op(&grant)
        };
        let as_revoke = SignedOp {
            type_id: TYPE_B,
            ..op(&revoke)
        };
        assert_ne!(as_grant.hash::<Sha2>(), as_revoke.hash::<Sha2>());
    }

    /// An operation signed under the shared separator must not verify under an
    /// application's own, and vice versa.
    #[test]
    fn application_separator_is_a_distinct_domain() {
        crate::domain_separator! {
            const MY_APP = "MyApp" version "1";
        }
        let body = body_bytes(42);
        let o = op(&body);
        assert_ne!(o.hash::<Sha2>(), o.hash_in::<Sha2>(&MY_APP));
    }

    /// `hash` is an identifier *derived from* the signed bytes, so the two
    /// must not be able to disagree about what the message is.
    #[test]
    fn hash_is_sha256_of_the_preimage() {
        let body = body_bytes(42);
        let o = op(&body);
        assert_eq!(
            o.hash::<Sha2>(),
            SHA256Hash::new(Sha2::sha256(&o.preimage()))
        );
    }

    /// The signed bytes are the separator followed by the struct encoding, and
    /// nothing else.
    #[test]
    fn preimage_is_separator_then_struct() {
        let body = body_bytes(42);
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
    fn hash_into_matches_hash() {
        use core::mem::MaybeUninit;
        let body = body_bytes(42);
        let o = op(&body);
        let mut scratch = [MaybeUninit::<u8>::uninit(); 256];
        assert_eq!(unsafe { o.hash_into::<Sha2>(&mut scratch) }, o.hash::<Sha2>());
    }

    #[test]
    fn type_id_round_trips_through_u64() {
        assert_eq!(TYPE_A.to_u64(), 0xaaaa_aaaa_aaaa_aaaa);
        assert_eq!(OperationTypeId::from_u64(TYPE_A.to_u64()), TYPE_A);
    }
}
