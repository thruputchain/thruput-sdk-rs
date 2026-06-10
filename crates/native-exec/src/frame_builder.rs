use crate::frame_result::FrameResult;
use crate::mock_transaction_handler::{
    MockInvocationFrame, MockInvocationFrameConfig,
};
use std::panic::UnwindSafe;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameAction {
    Continue,
    Close,
}

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
    ///
    /// The closure's return value is XDR-encoded into the frame's return
    /// buffer (the same convention generated `#[contract]` entrypoints use),
    /// and can be recovered from the result via `assert_return_as`.
    /// Any value that encodes to zero bytes (e.g. `()`) skips the explicit
    /// return and leaves the buffer untouched: an empty buffer is already
    /// the valid encoding of such types, so decoding them still works.
    /// Separately, closures that call `raw_Return` themselves never reach
    /// this encode step at all, since `raw_Return` terminates the
    /// invocation immediately (it never returns).
    ///
    /// Mixing an explicit `raw_Return` with a non-unit closure return is
    /// unsupported; the explicit buffer wins and the closure's value is
    /// never encoded.
    ///
    /// If you need to open child frames from within the closure, use
    /// `run_continuable` instead, which provides a `FrameHandle` with
    /// `child_call()`.
    pub fn run<T: xdrs::WriteXDR>(
        self,
        f: impl FnOnce() -> T + UnwindSafe,
    ) -> FrameResult {
        let frame = self.config.build();
        frame.execute(|_| {
            let value = f();
            let encoded = xdrs::cxs::encode(&value)
                .expect("run(): failed to XDR-encode closure return value");
            if !encoded.is_empty() {
                groundhog_xdr::syscalls::raw_Return(&encoded, false, &[]);
            }
            FrameAction::Close
        });
        frame.result()
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
            f(&mut handle)
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
            f(&mut handle)
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
        self.frame.result()
    }
}
