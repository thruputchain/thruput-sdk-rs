use crate::frame_action::FrameAction;
use crate::frame_result::FrameResult;
use crate::mock_transaction_handler::{
    MockInvocationFrame, MockInvocationFrameConfig,
};
use std::panic::UnwindSafe;

/// Builder for configuring and executing a single invocation frame.
pub struct FrameBuilder<'a> {
    config: MockInvocationFrameConfig<'a>,
}

impl<'a> FrameBuilder<'a> {
    pub(crate) fn new(config: MockInvocationFrameConfig<'a>) -> Self {
        Self { config }
    }

    pub fn with_calldata(self, calldata: xdr_common::BytesM) -> Self {
        Self {
            config: self.config.with_calldata(calldata),
        }
    }

    pub fn with_flags(self, flags: i32) -> Self {
        Self {
            config: self.config.with_invoke_flags(flags),
        }
    }

    /// Execute the frame with the given closure, then close it.
    /// This is the common single-shot path for leaf-only execution
    /// (no child frames needed). If you need to open child frames
    /// from within the closure, use `run_continuable` instead,
    /// which provides a `FrameHandle` with `child_call()`.
    pub fn run(self, f: impl FnOnce() + UnwindSafe) -> FrameResult {
        let frame = self.config.build();
        frame.execute(|_| {
            f();
            false // always close
        });
        let (output, trace) = frame.result();
        FrameResult::new(output, trace)
    }

    /// Execute the frame with a closure that receives a FrameHandle
    /// and returns a FrameAction. If FrameAction::Continue, returns
    /// a ContinuableFrame for further chaining.
    pub fn run_continuable(
        self,
        f: impl FnOnce(&mut FrameHandle) -> FrameAction + UnwindSafe,
    ) -> ContinuableFrame<'a> {
        let frame = self.config.build();
        frame.execute(|inner_frame| {
            let mut handle = FrameHandle { frame: inner_frame };
            let action = f(&mut handle);
            action.as_bool()
        });
        ContinuableFrame { frame }
    }
}

/// Handle to the current frame, available inside run_continuable closures.
/// Provides child_call() for opening nested frames.
pub struct FrameHandle<'a> {
    frame: &'a mut MockInvocationFrame,
}

impl<'a> FrameHandle<'a> {
    pub fn child_call(
        &mut self,
        addr: &groundhog_xdr::types::ContractAddress,
    ) -> FrameBuilder<'_> {
        let config = self.frame.open_child_frame().at_contract_address(*addr);
        FrameBuilder::new(config)
    }
}

/// A frame that is still open after run_continuable returned FrameAction::Continue.
/// Supports chaining additional run_continuable() calls or finishing.
pub struct ContinuableFrame<'a> {
    frame: &'a mut Box<MockInvocationFrame>,
}

impl<'a> ContinuableFrame<'a> {
    /// Execute another step on this frame.
    pub fn run_continuable(
        &mut self,
        f: impl FnOnce(&mut FrameHandle) -> FrameAction + UnwindSafe,
    ) -> &mut Self {
        self.frame.execute(|inner_frame| {
            let mut handle = FrameHandle { frame: inner_frame };
            let action = f(&mut handle);
            action.as_bool()
        });
        self
    }

    /// Open a child frame from within this continuable frame.
    pub fn child_call(
        &mut self,
        addr: &groundhog_xdr::types::ContractAddress,
    ) -> FrameBuilder<'_> {
        let config = self.frame.open_child_frame().at_contract_address(*addr);
        FrameBuilder::new(config)
    }

    /// Extract the frame result. Call this after the frame has been closed
    /// (i.e., after a run_continuable that returned FrameAction::Close).
    pub fn finish(&mut self) -> FrameResult {
        let (output, trace) = self.frame.result();
        FrameResult::new(output, trace)
    }
}
