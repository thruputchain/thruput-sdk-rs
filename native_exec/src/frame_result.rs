use groundhog_trace::InvocationFrameTrace;
use groundhog_xdr::transaction::InvocationFrameOutput;

/// Result of executing a single invocation frame.
/// Wraps InvocationFrameOutput + optional trace.
pub struct FrameResult {
    output: InvocationFrameOutput,
    trace: Option<InvocationFrameTrace>,
}

impl FrameResult {
    pub fn new(
        output: InvocationFrameOutput,
        trace: Option<InvocationFrameTrace>,
    ) -> Self {
        Self { output, trace }
    }

    // -- Status checks (using pattern matching for safety) --

    pub fn is_success(&self) -> bool {
        matches!(
            &self.output,
            InvocationFrameOutput::kInvocationFrameStatusCodeReturn { .. }
        )
    }

    pub fn is_abort(&self) -> bool {
        matches!(
            &self.output,
            InvocationFrameOutput::kInvocationFrameStatusCodeAbort { .. }
        )
    }

    pub fn is_fault(&self) -> bool {
        matches!(
            &self.output,
            InvocationFrameOutput::kInvocationFrameStatusCodeFault
        )
    }

    // -- Assertions --

    pub fn assert_success(&self) {
        assert!(
            self.is_success(),
            "expected success, got status code {}",
            self.output.status_code()
        );
    }

    pub fn assert_abort(&self) {
        assert!(
            self.is_abort(),
            "expected abort, got status code {}",
            self.output.status_code()
        );
    }

    pub fn assert_fault(&self) {
        assert!(
            self.is_fault(),
            "expected fault, got status code {}",
            self.output.status_code()
        );
    }

    // -- Buffer accessors --

    /// Returns the return buffer. Panics if the frame did not succeed.
    pub fn return_buffer(&self) -> &[u8] {
        match &self.output {
            InvocationFrameOutput::kInvocationFrameStatusCodeReturn {
                return_buffer,
            } => return_buffer.as_slice(),
            _ => panic!(
                "return_buffer() called on non-success frame (status {})",
                self.output.status_code()
            ),
        }
    }

    /// Returns the abort buffer. Panics if the frame did not abort.
    pub fn abort_buffer(&self) -> &[u8] {
        match &self.output {
            InvocationFrameOutput::kInvocationFrameStatusCodeAbort {
                abort_buffer,
            } => abort_buffer.as_slice(),
            _ => panic!(
                "abort_buffer() called on non-abort frame (status {})",
                self.output.status_code()
            ),
        }
    }

    // -- Typed abort assertion --

    /// Assert the frame aborted and decode the abort buffer as a typed error.
    pub fn assert_abort_with<E>(&self, check: impl FnOnce(&E))
    where
        E: xdrs::ReadXDROwned,
    {
        self.assert_abort();
        let buf = self.abort_buffer();
        let decoded: E = xdrs::txs::decode(buf).unwrap_or_else(|e| {
            panic!("failed to decode abort buffer: {}", e)
        });
        check(&decoded);
    }

    // -- Raw access --

    pub fn output(&self) -> &InvocationFrameOutput {
        &self.output
    }

    pub fn trace(&self) -> Option<&InvocationFrameTrace> {
        self.trace.as_ref()
    }

    // -- Per-frame events (TODO) --

    /// Per-frame event access not yet supported.
    /// Requires adding events to InvocationFrameTrace in hogd.
    /// Use TransactResult::events() for transaction-level events.
    pub fn events(&self) -> &[groundhog_xdr::event::TxEmittedEvent] {
        &[]
    }
}
