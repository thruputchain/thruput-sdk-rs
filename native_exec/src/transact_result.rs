use crate::frame_result::FrameResult;
use groundhog_xdr::event::TxEmittedEvent;
use groundhog_xdr::exec_result::ExecResult;

/// Result of a full transaction: root frame result + commit outcome.
/// Events are accessed via the root frame's per-frame events.
pub struct TransactResult {
    frame: FrameResult,
    exec_result: ExecResult,
}

impl TransactResult {
    pub fn new(frame: FrameResult, exec_result: ExecResult) -> Self {
        Self { frame, exec_result }
    }

    /// Asserts the root frame returned successfully AND the transaction committed OK.
    pub fn assert_success(&self) {
        self.frame.assert_success();
        assert_eq!(
            self.exec_result,
            ExecResult::ExecResultOK,
            "expected successful commit, got {:?}",
            self.exec_result
        );
    }

    pub fn assert_abort(&self) {
        self.frame.assert_abort();
    }

    pub fn assert_fault(&self) {
        self.frame.assert_fault();
    }

    /// Assert the root frame returned successfully and decode its return buffer.
    ///
    /// This checks only the frame outcome (contract execution), not the
    /// database commit. Use `assert_success()` when you need to verify both
    /// frame return *and* commit. This separation lets you inspect the return
    /// value even when the commit is expected to fail or hasn't happened yet.
    pub fn assert_return_with<R>(&self, check: impl FnOnce(&R))
    where
        R: xdrs::ReadXDROwned,
    {
        self.frame.assert_return_with(check);
    }

    pub fn assert_abort_with<E>(&self, check: impl FnOnce(&E))
    where
        E: xdrs::ReadXDROwned,
    {
        self.frame.assert_abort_with(check);
    }

    /// Asserts the root frame succeeded but the DB commit step failed.
    pub fn assert_commit_failed(&self) {
        self.frame.assert_success();
        assert_ne!(
            self.exec_result,
            ExecResult::ExecResultOK,
            "expected commit failure, but commit succeeded"
        );
    }

    pub fn exec_result(&self) -> &ExecResult {
        &self.exec_result
    }

    pub fn frame(&self) -> &FrameResult {
        &self.frame
    }

    // -- Events (delegated to root frame) --

    pub fn events(&self) -> &[TxEmittedEvent] {
        self.frame.events()
    }

    pub fn assert_event_count(&self, expected: usize) {
        self.frame.assert_event_count(expected);
    }

    pub fn assert_no_events(&self) {
        self.frame.assert_no_events();
    }

    pub fn filter_events<E>(&self) -> Vec<E>
    where
        E: xdrs::ReadXDROwned + groundhog_event::Event,
    {
        self.frame.filter_events()
    }

    pub fn assert_event(
        &self,
        index: usize,
        check: impl FnOnce(&TxEmittedEvent),
    ) {
        self.frame.assert_event(index, check);
    }

    pub fn assert_event_data<E>(&self, index: usize, check: impl FnOnce(&E))
    where
        E: xdrs::ReadXDROwned,
    {
        self.frame.assert_event_data(index, check);
    }

    pub fn assert_emitted<E>(&self, predicate: impl Fn(&E) -> bool)
    where
        E: xdrs::ReadXDROwned + groundhog_event::Event,
    {
        self.frame.assert_emitted(predicate);
    }
}
