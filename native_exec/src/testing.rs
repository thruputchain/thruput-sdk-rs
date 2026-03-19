use crate::frame_result::FrameResult;
use crate::test_environment::TestEnvironmentHandle;
use crate::transact_result::TransactResult;
use crate::transaction::Transaction;

/// Extension trait adding ergonomic transaction methods to TestEnvironmentHandle.
pub trait TestEnvExt {
    /// Execute a transaction with auto-commit.
    ///
    /// Creates a MockTransactionHandler, gives the closure a Transaction handle,
    /// and always calls try_commit_transaction when the closure returns
    /// (hogd handles abort internally when the root frame reverted).
    fn transact(
        &self,
        f: impl FnOnce(&mut Transaction) -> FrameResult,
    ) -> TransactResult;

    /// Execute a transaction with manual commit/abort control.
    ///
    /// The closure receives a Transaction and is responsible for
    /// calling tx.commit() or tx.abort().
    #[allow(dead_code)]
    fn transact_manual(&self, f: impl FnOnce(&mut Transaction));
}

impl TestEnvExt for TestEnvironmentHandle {
    fn transact(
        &self,
        f: impl FnOnce(&mut Transaction) -> FrameResult,
    ) -> TransactResult {
        let mut tx = Transaction::new(self.clone());

        let frame_result = f(&mut tx);

        let exec_result = tx.commit();
        let event_log = tx.get_event_logs();

        TransactResult::new(frame_result, exec_result, event_log)
    }

    fn transact_manual(&self, f: impl FnOnce(&mut Transaction)) {
        let mut tx = Transaction::new(self.clone());
        f(&mut tx);
        // Caller is responsible for commit/abort.
        // If neither was called, Transaction's Drop (via MockTransactionHandler::Drop)
        // will handle cleanup.
    }
}
