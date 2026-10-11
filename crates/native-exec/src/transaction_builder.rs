use crate::frame_result::FrameResult;
use crate::mock_transaction_handler::{
    MockTransactionConfig, MockTransactionHandler,
};
use crate::test_environment::TestEnvironmentHandle;
use crate::transact_result::TransactResult;
use thruput_xdr::auth_claim::AuthClaim;

/// Builder for configuring a mocked transaction before executing it.
///
/// Obtain one from [`TestEnvironment::transaction`](crate::test_environment::TestEnvironment::transaction),
/// chain `with_*` methods to configure it, then finish with
/// [`transact`](Self::transact), [`transact_manual`](Self::transact_manual),
/// or [`build`](Self::build):
///
/// ```ignore
/// env.transaction()
///     .with_auth_claim(my_claim)
///     .transact(|tx| tx.call(&addr).run(|| { /* ... */ }));
/// ```
///
/// `env.transact(f)` and `env.transact_manual(f)` are shorthand for
/// `env.transaction().transact(f)` / `.transact_manual(f)` with the default
/// configuration.
#[must_use = "a TransactionBuilder does nothing until `transact`, `transact_manual`, or `build` is called"]
pub struct TransactionBuilder {
    env: TestEnvironmentHandle,
    config: MockTransactionConfig,
}

impl TransactionBuilder {
    /// Start a builder with the default [`MockTransactionConfig`].
    pub fn new(env: TestEnvironmentHandle) -> Self {
        Self {
            env,
            config: MockTransactionConfig::default(),
        }
    }

    /// Attach `auth_claim` to the mocked transaction instead of the default
    /// source-address claim, controlling the auth-claim the contract under
    /// test observes (e.g. via `get_auth_claim`).
    pub fn with_auth_claim(mut self, auth_claim: AuthClaim) -> Self {
        self.config.auth_claim = auth_claim;
        self
    }

    /// Create the configured [`MockTransactionHandler`] for manual
    /// orchestration. Prefer [`transact`](Self::transact) or
    /// [`transact_manual`](Self::transact_manual) unless you need the handler
    /// directly.
    pub fn build(self) -> MockTransactionHandler {
        MockTransactionHandler::new_with_config(self.env, self.config)
    }

    /// Execute a transaction with auto-commit. See
    /// [`TestEnvironment::transact`](crate::test_environment::TestEnvironment::transact).
    #[must_use]
    pub fn transact(
        self,
        f: impl FnOnce(&mut MockTransactionHandler) -> FrameResult,
    ) -> TransactResult {
        let mut tx = self.build();
        let frame_result = f(&mut tx);
        let exec_result = tx.try_commit_transaction().exec_result();
        TransactResult::new(frame_result, exec_result)
    }

    /// Execute a transaction with manual commit/abort control. See
    /// [`TestEnvironment::transact_manual`](crate::test_environment::TestEnvironment::transact_manual).
    ///
    /// The closure must call `tx.try_commit_transaction()` or
    /// `tx.abort_transaction()` before returning; forgetting panics here
    /// (a dropped handler would abort silently).
    pub fn transact_manual<R>(
        self,
        f: impl FnOnce(&mut MockTransactionHandler) -> R,
    ) -> R {
        let mut tx = self.build();
        let out = f(&mut tx);
        assert!(
            !tx.is_running(),
            "transact_manual closure returned without finishing the \
             transaction: call tx.try_commit_transaction() or \
             tx.abort_transaction() before returning"
        );
        out
    }
}
