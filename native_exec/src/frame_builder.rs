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

    #[allow(dead_code)]
    pub fn with_calldata(self, calldata: xdr_common::BytesM) -> Self {
        Self {
            config: self.config.with_calldata(calldata),
        }
    }

    #[allow(dead_code)]
    pub fn with_flags(self, flags: i32) -> Self {
        Self {
            config: self.config.with_invoke_flags(flags),
        }
    }

    /// Execute the frame with the given closure, then close it.
    /// This is the common single-shot path.
    pub fn run(self, f: impl FnOnce() + UnwindSafe) -> FrameResult {
        let frame = self.config.build();
        frame.execute(|_| {
            f();
            false // always close
        });
        let (output, trace) = frame.result();
        FrameResult::new(output, trace)
    }

    /// Execute the frame with a closure that receives a ContinuableFrame
    /// and returns a FrameAction. Advanced -- for frame interleaving.
    #[allow(dead_code)]
    pub fn run_continuable(
        self,
        _f: impl FnOnce(&mut ContinuableFrame) -> FrameAction + UnwindSafe,
    ) -> ContinuableFrame<'a> {
        todo!("run_continuable: deferred -- requires careful lifetime management around execute()'s closure model")
    }
}

/// Wraps a MockInvocationFrame that is still open.
/// Supports chaining additional run_continuable() calls and child_call().
#[allow(dead_code)]
pub struct ContinuableFrame<'a> {
    frame: &'a mut MockInvocationFrame,
}

#[allow(dead_code)]
impl<'a> ContinuableFrame<'a> {
    pub fn child_call(
        &mut self,
        addr: &groundhog_xdr::types::ContractAddress,
    ) -> FrameBuilder<'_> {
        let config = self.frame.open_child_frame().at_contract_address(*addr);
        FrameBuilder::new(config)
    }

    pub fn run_continuable(
        self,
        _f: impl FnOnce(&mut ContinuableFrame) -> FrameAction + UnwindSafe,
    ) -> ContinuableFrame<'a> {
        todo!("continuable chaining: deferred")
    }

    pub fn finish(self) -> FrameResult {
        let (output, trace) = self.frame.result();
        FrameResult::new(output, trace)
    }
}
