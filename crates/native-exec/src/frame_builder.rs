use crate::frame_result::FrameResult;
use crate::mock_transaction_handler::{
    MockInvocationFrame, MockInvocationFrameConfig, frame_refusal_message,
};
use groundhog_xdr::syscall_error::SyscallErrorCode;
use std::panic::UnwindSafe;

/// What an `execute` closure tells the harness to do with its frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameAction {
    /// Leave the frame open for further `run_continuable` steps.
    Continue,
    /// Close the frame, producing its [`FrameResult`].
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

    /// Set the calldata the invoked frame observes (default: empty).
    pub fn with_calldata(self, calldata: xdr_common::BytesM) -> Self {
        Self {
            config: self.config.with_calldata(calldata),
        }
    }

    /// Set the frame's invoke flags, a bitset of
    /// [`InvokeFlag`](groundhog_xdr::invoke_flag::InvokeFlag) values
    /// (default: none).
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
    /// The return buffer is subject to the protocol's return-size cap
    /// (1024 bytes); a larger encoded value deterministically reverts the
    /// invocation, surfacing as an opaque frame failure. Keep test return
    /// values small — return a digest or summary instead of bulk data.
    ///
    /// If you need to open child frames from within the closure, use
    /// `run_continuable` instead, which provides a `FrameHandle` with
    /// `child_call()`.
    pub fn run<T: xdrs::WriteXDR>(
        self,
        f: impl FnOnce() -> T + UnwindSafe,
    ) -> FrameResult {
        self.try_run(f).unwrap_or_else(|refused| {
            panic!("{}", frame_refusal_message(refused))
        })
    }

    /// Execute the frame as [`run`](Self::run) does, but report a frame
    /// production refuses to admit rather than failing the test binary.
    ///
    /// `Err` carries the rule -- reentrance, the cross-invoke depth cap, or
    /// undefined invoke flags -- and means no frame opened and the closure
    /// never ran, which is exactly what a contract's refused `Invoke` gets.
    /// Use it to assert on a cap the chain sets; use `run` when the frame is
    /// expected to open.
    pub fn try_run<T: xdrs::WriteXDR>(
        self,
        f: impl FnOnce() -> T + UnwindSafe,
    ) -> Result<FrameResult, SyscallErrorCode> {
        let frame = self.config.try_build()?;
        frame.execute(|_| {
            let value = f();
            let encoded = xdrs::cxs::encode(&value)
                .expect("run(): failed to XDR-encode closure return value");
            if encoded.is_empty() {
                // Zero-byte encodings (e.g. `()`): an empty buffer is
                // already the valid encoding, so close normally with no
                // explicit return.
                FrameAction::Close
            } else {
                // Sets the return buffer and diverges; the frame closes
                // via the unwind path, equivalently to FrameAction::Close.
                groundhog_xdr::syscalls::raw_Return(&encoded, false, &[])
            }
        });
        Ok(frame.result())
    }

    /// Execute the frame with a closure that receives a FrameHandle
    /// and returns a FrameAction. If FrameAction::Continue, returns
    /// a ContinuableFrame for further chaining.
    ///
    /// To return a typed value from a continuable frame, end the closing
    /// step with an explicit `raw_Return` — it sets the buffer and
    /// diverges (closing the frame), and `!` coerces to `FrameAction`:
    ///
    /// ```rust,ignore
    /// let result = tx.call(&addr)
    ///     .run_continuable(|frame| {
    ///         // ... child calls, multi-step work ...
    ///         raw_Return(&xdrs::cxs::encode(&value).unwrap(), false, &[])
    ///     })
    ///     .finish();
    /// let v: T = result.assert_return_as();
    /// ```
    pub fn run_continuable(
        self,
        f: impl FnOnce(&mut FrameHandle) -> FrameAction + UnwindSafe,
    ) -> ContinuableFrame<'a> {
        self.try_run_continuable(f).unwrap_or_else(|refused| {
            panic!("{}", frame_refusal_message(refused))
        })
    }

    /// Open a continuable frame as
    /// [`run_continuable`](Self::run_continuable) does, reporting a refused
    /// frame rather than failing the test binary. See
    /// [`try_run`](Self::try_run).
    pub fn try_run_continuable(
        self,
        f: impl FnOnce(&mut FrameHandle) -> FrameAction + UnwindSafe,
    ) -> Result<ContinuableFrame<'a>, SyscallErrorCode> {
        let frame = self.config.try_build()?;
        frame.execute(|inner_frame| {
            let mut handle = FrameHandle { frame: inner_frame };
            f(&mut handle)
        });
        Ok(ContinuableFrame { frame })
    }
}

/// Handle to the current frame, available inside run_continuable closures.
/// Provides child_call() for opening nested frames.
pub struct FrameHandle<'a> {
    frame: &'a mut MockInvocationFrame,
}

impl<'a> FrameHandle<'a> {
    /// Open a child frame of the current frame, invoking `addr`. The
    /// address must differ from every frame already open on the stack (the
    /// reentrance rule); see [`MockInvocationFrame::open_child_frame`].
    pub fn child_call(
        &mut self,
        addr: &groundhog_xdr::types::ContractAddress,
    ) -> FrameBuilder<'_> {
        let config = self.frame.open_child_frame(*addr);
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
        let config = self.frame.open_child_frame(*addr);
        FrameBuilder::new(config)
    }

    /// Extract the frame result. Call this after the frame has been closed
    /// (i.e., after a run_continuable that returned FrameAction::Close).
    pub fn finish(&mut self) -> FrameResult {
        self.frame.result()
    }
}
