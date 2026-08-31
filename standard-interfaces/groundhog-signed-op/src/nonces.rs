//! Unordered nonce slots, and the verify-and-spend that consumes them.

use groundhog_contract_spec::ContractTypeSpec;
use groundhog_xdr::types::NetworkHash;
use sdk::exn::NoneIfMissing;
use sdk::storage::{Map, Value};
use sdk::types::{Ed25519PublicKey, Ed25519Signature};
use sdk::xdr::{ReadXDR, WriteXDR};

use crate::{Operation, SignedOpError, verify_signed_op};

/// Nonce slots per stored word. A signer's nonce `n` lives at bit `n % 64` of
/// word `n / 64`.
pub const SLOTS_PER_WORD: u64 = 64;

/// Storage for signed-operation replay protection, and the network hash the
/// signatures are bound to.
///
/// Embed this in a verifying contract's storage and drive it with
/// [`verify_and_spend`](Self::verify_and_spend); it exists so that "verified
/// the signature but forgot to consume the nonce" is not expressible.
///
/// # Why slots rather than a counter
///
/// A sequential counter forces submission *order*: sign operations 5 and 6, and
/// if 6 lands first it fails and stays failed. That is the wrong default for
/// the case this whole mechanism exists to serve — the signer does not control
/// submission, and may have several operations outstanding at once.
///
/// So a nonce here is a **slot index**, not a counter. Slot `n` occupies bit
/// `n % 64` of word `n / 64`; the signer picks any unused slot, and slots may
/// be spent in any order. Reading one word
/// ([`word`](Self::word)) tells a client the state of 64 slots at once, and
/// [`cancel_word`](Self::cancel_word) revokes up to 64 outstanding operations
/// in a single write.
///
/// Slots are keyed by the signing **key**, not by an address: an operation is
/// authorized by whoever holds the key, and address-from-pubkey derivation
/// needs an account nonce this layer does not carry.
#[sdk::storage]
pub struct SignedOpNonces {
    /// The network these signatures are valid on. Stored because a contract
    /// cannot read its own network hash — see the crate docs.
    network: Value<NetworkHash>,
    /// `(signer, word) -> 64-bit slot bitmap`. An absent entry is all-unused.
    words: Map<(Ed25519PublicKey, u64), Value<u64>>,
}

impl SignedOpNonces {
    /// Record the network these signatures are bound to. Call once at
    /// deployment; a wrong or missing value silently removes a replay bound.
    pub fn initialize(&mut self, network: &NetworkHash) {
        self.network.set(network);
    }

    /// The configured network hash.
    pub fn network(&self) -> NetworkHash {
        self.network.get()
    }

    /// The 64-slot bitmap for `signer`'s word `word`; a set bit is a spent
    /// slot. Clients read this to choose a free slot.
    pub fn word(&self, signer: &Ed25519PublicKey, word: u64) -> u64 {
        self.words
            .at(&(*signer, word))
            .try_get()
            .none_if_missing()
            .unwrap_or(0)
    }

    /// Whether `nonce` has already been spent by `signer`.
    pub fn is_spent(&self, signer: &Ed25519PublicKey, nonce: u64) -> bool {
        let (word, bit) = slot(nonce);
        self.word(signer, word) & bit != 0
    }

    /// Burn every unspent slot in `word`, revoking up to 64 outstanding
    /// operations at once. Cancelling is what a signer reaches for when a
    /// signature has escaped; there is no other way to retract one before
    /// `valid_until`.
    pub fn cancel_word(&mut self, signer: &Ed25519PublicKey, word: u64) {
        self.words.at_mut(&(*signer, word)).set(&u64::MAX);
    }

    /// Verify an operation and consume its slot, returning the decoded payload.
    ///
    /// The two halves are one call on purpose: a verify that does not spend
    /// leaves the signature replayable forever, and that is not a mistake a
    /// caller should be able to make. Throws [`SignedOpError`] on any failed
    /// check, in which case nothing is written.
    ///
    /// `T` is the payload type this contract expects; its `TYPE_DEF` is part of
    /// the signed bytes, so naming the wrong one fails verification rather than
    /// reinterpreting the message.
    pub fn verify_and_spend<T>(
        &mut self,
        signer: &Ed25519PublicKey,
        signature: &Ed25519Signature,
        op: &Operation,
    ) -> T
    where
        T: WriteXDR + for<'r> ReadXDR<'r> + ContractTypeSpec,
    {
        let (word, bit) = slot(op.nonce);
        let mut entry = self.words.at_mut(&(*signer, word));
        let bitmap = entry.try_get().none_if_missing().unwrap_or(0);
        if bitmap & bit != 0 {
            sdk::throw(SignedOpError::NonceAlreadySpent);
        }

        let body = verify_signed_op::<T>(
            &self.network.get(),
            &sdk::get_current_address(),
            op,
            signer,
            signature,
        );

        // Only after every check: a throw above leaves the slot unspent.
        entry.set(&(bitmap | bit));
        body
    }

    /// [`verify_and_spend`](Self::verify_and_spend) without writing — answers
    /// "would this be accepted?" from a read-only entrypoint. Returns the
    /// decoded payload; still throws on a failed check.
    pub fn would_accept<T>(
        &self,
        signer: &Ed25519PublicKey,
        signature: &Ed25519Signature,
        op: &Operation,
    ) -> T
    where
        T: WriteXDR + for<'r> ReadXDR<'r> + ContractTypeSpec,
    {
        if self.is_spent(signer, op.nonce) {
            sdk::throw(SignedOpError::NonceAlreadySpent);
        }
        verify_signed_op::<T>(
            &self.network.get(),
            &sdk::get_current_address(),
            op,
            signer,
            signature,
        )
    }
}

/// Split a nonce into its word index and its single-bit mask.
fn slot(nonce: u64) -> (u64, u64) {
    (nonce / SLOTS_PER_WORD, 1u64 << (nonce % SLOTS_PER_WORD))
}
