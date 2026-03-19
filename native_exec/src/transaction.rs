use crate::frame_builder::FrameBuilder;
use crate::mock_transaction_handler::MockTransactionHandler;
use crate::test_environment::TestEnvironmentHandle;
use groundhog_xdr::exec_result::ExecResult;
use groundhog_xdr::types::ContractAddress;

/// Handle to a mock transaction in progress.
pub struct Transaction {
    handler: MockTransactionHandler,
}

impl Transaction {
    pub fn new(env: TestEnvironmentHandle) -> Self {
        Self {
            handler: MockTransactionHandler::new(env),
        }
    }

    /// Begin a root frame call to the given contract address.
    pub fn call(&mut self, addr: &ContractAddress) -> FrameBuilder<'_> {
        let config = self.handler.open_root_frame().at_contract_address(*addr);
        FrameBuilder::new(config)
    }

    /// Commit the transaction. Returns the ExecResult.
    pub fn commit(&mut self) -> ExecResult {
        self.handler.notify_root_frame_closed();
        self.handler.try_commit_transaction()
    }

    /// Abort the transaction.
    pub fn abort(&mut self) {
        self.handler.notify_root_frame_closed();
        self.handler.abort_transaction();
    }

    /// Get event logs after commit/abort.
    pub(crate) fn get_event_logs(
        &self,
    ) -> Option<groundhog_xdr::event::TxEventLog> {
        self.handler.get_event_logs()
    }
}
