use crate::frame_result::FrameResult;
use thruput_event::{EmittedEvent, Event, TypedEvent};
use thruput_exn::Exn;
use thruput_xdr::event::TxEmittedEvent;
use thruput_xdr::exec_result::ExecResult;
use thruput_xdr::types::ContractAddress;

/// Result of a full transaction: root frame result + commit outcome.
/// Events are accessed via the root frame's per-frame events.
#[derive(Debug)]
pub struct TransactResult {
    frame: FrameResult,
    exec_result: ExecResult,
}

impl TransactResult {
    /// Combine a root-frame result with the transaction's commit outcome.
    pub fn new(frame: FrameResult, exec_result: ExecResult) -> Self {
        Self { frame, exec_result }
    }

    /// The panic that faulted the root frame's body, if it faulted with one.
    /// See [`FrameResult::fault_panic`] for what `None` covers — notably, a
    /// fault caused by a called contract rather than by this frame's own body.
    pub fn fault_panic(&self) -> Option<&crate::invoke_error::PanicRecord> {
        self.frame.fault_panic()
    }

    /// Just the message from [`fault_panic`](Self::fault_panic).
    pub fn fault_message(&self) -> Option<&str> {
        self.frame.fault_message()
    }

    /// True if the root frame returned successfully AND the commit was OK.
    pub fn is_success(&self) -> bool {
        self.frame.is_success() && self.exec_result == ExecResult::ExecResultOK
    }

    /// Asserts the root frame returned successfully AND the transaction committed OK.
    #[track_caller]
    pub fn assert_success(&self) {
        self.frame.assert_success();
        assert_eq!(
            self.exec_result,
            ExecResult::ExecResultOK,
            "expected successful commit, got {:?}",
            self.exec_result
        );
    }

    /// Assert the root frame aborted. Frame-only check; the commit outcome
    /// is not consulted.
    #[track_caller]
    pub fn assert_abort(&self) {
        self.frame.assert_abort();
    }

    /// Assert the root frame faulted. Frame-only check.
    #[track_caller]
    pub fn assert_fault(&self) {
        self.frame.assert_fault();
    }

    /// Assert the root frame returned successfully and decode its return buffer.
    ///
    /// This checks only the frame outcome (contract execution), not the
    /// database commit. Use `assert_success()` when you need to verify both
    /// frame return *and* commit. This separation lets you inspect the return
    /// value even when the commit is expected to fail or hasn't happened yet.
    #[track_caller]
    pub fn assert_return_with<R>(&self, check: impl FnOnce(&R))
    where
        R: xdrs::ReadXDROwned,
    {
        self.frame.assert_return_with(check);
    }

    /// Assert the root frame returned successfully and decode its return
    /// buffer, returning the decoded value. Frame-only check, like
    /// `assert_return_with`.
    #[track_caller]
    pub fn assert_return_as<R>(&self) -> R
    where
        R: xdrs::ReadXDROwned,
    {
        self.frame.assert_return_as()
    }

    /// Assert the root frame aborted with a thrown `E` and return it; see
    /// [`FrameResult::assert_abort_as`]. Frame-only check (commit status not
    /// checked).
    #[track_caller]
    pub fn assert_abort_as<E>(&self) -> E
    where
        E: Exn + xdrs::ReadXDROwned,
    {
        self.frame.assert_abort_as()
    }

    /// Asserts the root frame succeeded but the DB commit step failed.
    #[track_caller]
    pub fn assert_commit_failed(&self) {
        self.frame.assert_success();
        assert_ne!(
            self.exec_result,
            ExecResult::ExecResultOK,
            "expected commit failure, but commit succeeded"
        );
    }

    /// Assert the root frame succeeded but the commit failed with
    /// `ExecResultTxNonDeterministicError` (a storage conflict).
    #[track_caller]
    pub fn assert_commit_failed_nondeterministically(&self) {
        self.frame.assert_success();
        assert_eq!(
            self.exec_result,
            ExecResult::ExecResultTxNonDeterministicError,
            "expected non-deterministic commit failure"
        );
    }

    /// The transaction's commit outcome.
    pub fn exec_result(&self) -> &ExecResult {
        &self.exec_result
    }

    /// The root frame's result.
    pub fn frame(&self) -> &FrameResult {
        &self.frame
    }

    // -- Events (delegated to root frame) --

    /// The events the root frame emitted; see [`FrameResult::events`].
    pub fn events(&self) -> &[TxEmittedEvent] {
        self.frame.events()
    }

    /// Assert exactly `expected` events were emitted by the root frame.
    #[track_caller]
    pub fn assert_event_count(&self, expected: usize) {
        self.frame.assert_event_count(expected);
    }

    /// Assert the root frame emitted no events.
    #[track_caller]
    pub fn assert_no_events(&self) {
        self.frame.assert_no_events();
    }

    /// All raw events emitted at the given address by the root frame.
    pub fn events_from(
        &self,
        address: &ContractAddress,
    ) -> Vec<&TxEmittedEvent> {
        self.frame.events_from(address)
    }

    /// See [`FrameResult::filter_events`] for the matching rules.
    pub fn filter_events<E>(
        &self,
        address: &ContractAddress,
    ) -> Vec<TypedEvent<'_, E>>
    where
        E: Event,
    {
        self.frame.filter_events(address)
    }

    /// Inspect a raw emitted event by index; see
    /// [`FrameResult::assert_raw_event`].
    #[track_caller]
    pub fn assert_raw_event(
        &self,
        index: usize,
        check: impl FnOnce(&EmittedEvent<'_>),
    ) {
        self.frame.assert_raw_event(index, check);
    }

    /// Decode an event as a typed event and pass it to `check`; see
    /// [`FrameResult::assert_event`].
    #[track_caller]
    pub fn assert_event<E>(
        &self,
        index: usize,
        check: impl FnOnce(&TypedEvent<'_, E>),
    ) where
        E: Event,
    {
        self.frame.assert_event::<E>(index, check);
    }

    /// See [`FrameResult::assert_emitted`] for the matching rules.
    #[track_caller]
    pub fn assert_emitted<E>(
        &self,
        address: &ContractAddress,
        predicate: impl Fn(&TypedEvent<'_, E>) -> bool,
    ) where
        E: Event,
    {
        self.frame.assert_emitted(address, predicate);
    }
}
