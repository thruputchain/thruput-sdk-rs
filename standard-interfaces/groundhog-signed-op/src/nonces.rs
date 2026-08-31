//! Replay protection for signed operations.
//!
//! # This is a stopgap with a known shape
//!
//! The filter here is a sliding window: constant storage per signer, garbage
//! collected by sliding forward. It is deliberately the *simplest* thing that
//! accepts operations out of order, because it is meant to be replaced by
//! `hogd`'s garbage-collecting replay cache once that is exposed to contracts.
//!
//! So the public surface is written to survive that swap. Nothing here exposes
//! a word index, a bitmap, or any other artifact of the representation — a
//! replay cache has none of those. What callers see is behavioural:
//! "would this nonce be accepted", "give me one that would be", "record this
//! one". Those three questions have the same answers under either
//! implementation.
//!
//! [`SignedOpError::NonceTooOld`] is the one exception: it exists only because
//! a window forgets, and should disappear when the replay cache lands. Treat
//! it as the marker for functionality that is knowingly incomplete.
//!
//! # What the window does and does not fix
//!
//! It fixes storage growth. A per-operation record grows without bound and
//! nothing here could collect it; a window is 24 bytes per signer forever.
//!
//! # Channels: concurrency without unbounded state
//!
//! A window is read-modify-written, so two operations spending from the same
//! window in one block conflict and only one commits — the constraint
//! `Vector`'s length field documents. There is no mergeable primitive that
//! would avoid it: the storage layer offers commutative *addition*
//! ([`NNInt`](sdk::storage::NNInt)) but no commutative bit-set.
//!
//! So a signer picks a **channel**, and replay state is per
//! `(signer, channel)`. Operations in different channels are consumed
//! independently and commit in the same block; operations sharing a channel
//! serialise. That is the concurrency knob, and it is the signer's to turn.
//!
//! Channel numbers are the signer's to choose, and nothing here polices them.
//! What matters is that state grows only when a signer *decides* to use another
//! channel, and then stops: a signer who works in channels 0..4 has four
//! windows, permanently, however many operations it signs. Contrast the shape
//! this replaced, where a bitmap was minted for every 64 nonces — growth that
//! nobody chose and nothing could collect. A signer that invents a thousand
//! channels gets a thousand windows and has only itself to answer to.
//!
//! `channel` is part of the *signed* preimage. It has to be: if a submitter
//! could choose it, one signature would replay once per channel.

use groundhog_contract_spec::ContractTypeSpec;
use groundhog_xdr::types::NetworkHash;
use sdk::exn::NoneIfMissing;
use sdk::storage::{Map, Value};
use sdk::types::{Ed25519PublicKey, Ed25519Signature};
use sdk::xdr::{ReadXDR, WriteXDR};

use crate::{Operation, SignedOpError, verify_signed_op};

/// How far below the highest-accepted nonce the window remembers. A nonce
/// older than this is refused even if never spent.
pub const WINDOW: u64 = 128;

/// Per-`(signer, channel)` replay state: the highest nonce accepted, and a
/// bitmap of the
/// [`WINDOW`] nonces at and below it. Bit `i` is nonce `high - i`, so sliding
/// forward is a left shift and everything shifted out stays permanently
/// unusable.
#[derive(ReadXDR, WriteXDR)]
#[xdrs(crate = "sdk::__xdrs")]
struct ReplayWindow {
    high: u64,
    seen: u128,
}

impl ReplayWindow {
    /// The window a signer's first operation creates.
    fn opened_at(nonce: u64) -> Self {
        Self {
            high: nonce,
            seen: 1,
        }
    }

    /// Whether `nonce` would be accepted, and why not if it would not.
    fn admits(&self, nonce: u64) -> Result<(), SignedOpError> {
        if nonce > self.high {
            return Ok(());
        }
        let age = self.high - nonce;
        if age >= WINDOW {
            return Err(SignedOpError::NonceTooOld);
        }
        if self.seen & (1u128 << age) != 0 {
            return Err(SignedOpError::NonceAlreadySpent);
        }
        Ok(())
    }

    /// Refuse every nonce at or below `through`, sliding forward if needed.
    fn cancel_through(&mut self, through: u64) {
        if through >= self.high {
            self.high = through;
            self.seen = u128::MAX;
        } else {
            let age = self.high - through;
            if age < WINDOW {
                self.seen |= u128::MAX << age;
            }
        }
    }

    /// Record `nonce`, sliding the window forward if it is above `high`.
    /// Caller must have checked [`admits`](Self::admits) first.
    fn record(&mut self, nonce: u64) {
        if nonce > self.high {
            let shift = nonce - self.high;
            self.seen = if shift >= WINDOW {
                0
            } else {
                self.seen << shift
            };
            self.seen |= 1;
            self.high = nonce;
        } else {
            self.seen |= 1u128 << (self.high - nonce);
        }
    }
}

/// Replay protection and network binding for a signed-operation verifier.
///
/// Embed this in a verifying contract's storage and drive it with
/// [`verify_and_spend`](Self::verify_and_spend). The verify and the spend are
/// one call so that "checked the signature but never consumed the nonce" — a
/// silently replayable operation — is not expressible.
///
/// Nonces are opaque `u64`s chosen by the signer, not a counter it must keep in
/// step with. They may be spent out of order, which is the point: the signer
/// does not control submission and may have several operations outstanding.
/// Ask [`next_nonce`](Self::next_nonce) for one that will be accepted.
///
/// Keyed by the signing **key**, not an address: an operation is authorised by
/// whoever holds the key, and address-from-pubkey derivation needs an account
/// nonce this layer does not carry.
#[sdk::storage]
pub struct SignedOpNonces {
    /// The network these signatures are valid on. Stored because a contract
    /// cannot read its own network hash — see the crate docs.
    network: Value<NetworkHash>,
    /// One window per `(signer, channel)` a signer has actually used.
    windows: Map<(Ed25519PublicKey, u32), Value<ReplayWindow>>,
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

    fn window(
        &self,
        signer: &Ed25519PublicKey,
        channel: u32,
    ) -> Option<ReplayWindow> {
        self.windows
            .at(&(*signer, channel))
            .try_get()
            .none_if_missing()
    }

    /// Whether `nonce` would be accepted for `signer` right now.
    ///
    /// Behavioural, not structural: it asks the filter a question rather than
    /// exposing how the filter stores its answer, so it means the same thing
    /// under a replay cache.
    pub fn is_usable(
        &self,
        signer: &Ed25519PublicKey,
        channel: u32,
        nonce: u64,
    ) -> bool {
        self.window(signer, channel)
            .is_none_or(|window| window.admits(nonce).is_ok())
    }

    /// A nonce that would be accepted for `signer` right now.
    ///
    /// Clients call this to pick one. Two callers racing get the same answer
    /// and one will lose; a signer preparing several operations at once should
    /// take this value and increment locally, since anything above the current
    /// high water mark is equally acceptable.
    pub fn next_nonce(&self, signer: &Ed25519PublicKey, channel: u32) -> u64 {
        self.window(signer, channel)
            .map_or(0, |window| window.high + 1)
    }

    /// Refuse every nonce at or below `through` for `signer`, revoking any
    /// outstanding operations in that range.
    ///
    /// The only way to retract a signature before its `valid_until`. Expressible
    /// against a replay cache too, which is why it is phrased as a range rather
    /// than as clearing a bitmap word.
    pub fn cancel_through(
        &mut self,
        signer: &Ed25519PublicKey,
        channel: u32,
        through: u64,
    ) {
        let mut window = self
            .window(signer, channel)
            .unwrap_or(ReplayWindow::opened_at(through));
        window.cancel_through(through);
        self.windows.at_mut(&(*signer, channel)).set(&window);
    }

    /// Verify an operation and consume its nonce, returning the decoded
    /// payload.
    ///
    /// Throws [`SignedOpError`] on any failed check, in which case nothing is
    /// written. `T` is the payload type this contract expects; its `TYPE_DEF`
    /// is part of the signed bytes, so naming the wrong one fails verification
    /// rather than reinterpreting the message.
    pub fn verify_and_spend<T>(
        &mut self,
        signer: &Ed25519PublicKey,
        signature: &Ed25519Signature,
        op: &Operation,
    ) -> T
    where
        T: WriteXDR + for<'r> ReadXDR<'r> + ContractTypeSpec,
    {
        let mut window = self
            .window(signer, op.channel)
            .unwrap_or(ReplayWindow::opened_at(op.nonce));
        if let Err(e) = window.admits(op.nonce) {
            sdk::throw(e);
        }

        let body = self.verify_only::<T>(signature, op, signer);

        // Only after every check: a throw above leaves the nonce unspent.
        window.record(op.nonce);
        self.windows.at_mut(&(*signer, op.channel)).set(&window);
        body
    }

    /// [`verify_and_spend`](Self::verify_and_spend) without writing — answers
    /// "would this be accepted?" from a read-only entrypoint.
    pub fn would_accept<T>(
        &self,
        signer: &Ed25519PublicKey,
        signature: &Ed25519Signature,
        op: &Operation,
    ) -> T
    where
        T: WriteXDR + for<'r> ReadXDR<'r> + ContractTypeSpec,
    {
        if let Some(window) = self.window(signer, op.channel) {
            if let Err(e) = window.admits(op.nonce) {
                sdk::throw(e);
            }
        }
        self.verify_only::<T>(signature, op, signer)
    }

    fn verify_only<T>(
        &self,
        signature: &Ed25519Signature,
        op: &Operation,
        signer: &Ed25519PublicKey,
    ) -> T
    where
        T: WriteXDR + for<'r> ReadXDR<'r> + ContractTypeSpec,
    {
        verify_signed_op::<T>(
            &self.network.get(),
            &sdk::get_current_address(),
            op,
            signer,
            signature,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opened(high: u64) -> ReplayWindow {
        ReplayWindow::opened_at(high)
    }

    #[test]
    fn a_fresh_slot_is_spent_then_refused() {
        let mut w = opened(10);
        assert_eq!(w.admits(10), Err(SignedOpError::NonceAlreadySpent));
        assert!(w.admits(11).is_ok());
        w.record(11);
        assert_eq!(w.admits(11), Err(SignedOpError::NonceAlreadySpent));
    }

    /// The whole reason for a window rather than a counter.
    #[test]
    fn nonces_may_be_spent_out_of_order() {
        let mut w = opened(100);
        w.record(105);
        assert!(w.admits(102).is_ok());
        w.record(102);
        assert_eq!(w.admits(102), Err(SignedOpError::NonceAlreadySpent));
        assert_eq!(w.admits(105), Err(SignedOpError::NonceAlreadySpent));
    }

    /// Garbage collection: sliding forward forgets, and what is forgotten is
    /// refused rather than silently reusable.
    #[test]
    fn sliding_forward_refuses_what_it_forgot() {
        let mut w = opened(0);
        w.record(1_000);
        assert_eq!(w.admits(1), Err(SignedOpError::NonceTooOld));
        assert_eq!(
            w.admits(1_000 - WINDOW),
            Err(SignedOpError::NonceTooOld)
        );
        assert!(w.admits(1_000 - WINDOW + 1).is_ok());
    }

    /// A jump wider than the window must clear it, not shift bits into
    /// positions that would mark unrelated nonces as spent.
    #[test]
    fn a_jump_past_the_window_clears_it() {
        let mut w = opened(5);
        w.record(5 + WINDOW * 3);
        assert_eq!(w.seen, 1, "only the new high is marked");
    }

    #[test]
    fn cancel_through_revokes_the_range_and_nothing_above() {
        let mut w = opened(10);
        w.record(20);
        assert!(w.admits(15).is_ok(), "unspent before cancelling");

        w.cancel_through(20);

        assert_eq!(w.admits(20), Err(SignedOpError::NonceAlreadySpent));
        assert_eq!(w.admits(15), Err(SignedOpError::NonceAlreadySpent));
        assert!(w.admits(21).is_ok(), "above the cancelled range");
    }

    /// Cancelling below the high water mark must not disturb what is above it.
    #[test]
    fn cancel_through_below_high_spares_later_nonces() {
        let mut w = opened(10);
        w.record(30);
        w.cancel_through(20);
        assert_eq!(w.admits(20), Err(SignedOpError::NonceAlreadySpent));
        assert_eq!(w.admits(12), Err(SignedOpError::NonceAlreadySpent));
        assert!(w.admits(25).is_ok(), "between the cancel point and high");
        assert!(w.admits(31).is_ok());
    }
}
