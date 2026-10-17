use crate::invoke_error::PanicRecord;
use crate::rv32_stack_usage::Rv32StackUsage;
use thruput_event::{EmittedEvent, Event, TypedEvent};
use thruput_exn::{Exn, RawExceptionRef};
use thruput_trace::InvocationFrameTrace;
use thruput_xdr::event::TxEmittedEvent;
use thruput_xdr::transaction::InvocationFrameOutput;
use thruput_xdr::types::ContractAddress;

/// One-line trace context for assertion messages: the syscalls the frame
/// made, ending with the one closest to the failure.
fn describe_trace(trace: Option<&InvocationFrameTrace>) -> String {
    let Some(trace) = trace else {
        return String::new();
    };
    let count = trace.system_calls.len();
    let last = trace.system_calls.iter().last().map(|syscall| {
        let mut dbg = format!("{syscall:?}");
        const MAX: usize = 160;
        if dbg.len() > MAX {
            let mut end = MAX;
            while !dbg.is_char_boundary(end) {
                end -= 1;
            }
            dbg.truncate(end);
            dbg.push('…');
        }
        dbg
    });
    match last {
        None => format!("\n  trace: {count} syscalls"),
        Some(last) => {
            format!("\n  trace: {count} syscalls, last: {last}")
        }
    }
}

/// Result of executing a single invocation frame.
/// Wraps InvocationFrameOutput + optional trace + per-frame events.
#[derive(Debug, Clone)]
pub struct FrameResult {
    output: InvocationFrameOutput,
    trace: Option<InvocationFrameTrace>,
    events: Vec<TxEmittedEvent>,
    fault_panic: Option<PanicRecord>,
    rv32_stack_usage: Vec<Rv32StackUsage>,
}

impl FrameResult {
    /// Wrap a frame's output, optional trace, and emitted events.
    pub fn new(
        output: InvocationFrameOutput,
        trace: Option<InvocationFrameTrace>,
        events: Vec<TxEmittedEvent>,
    ) -> Self {
        Self {
            output,
            trace,
            events,
            fault_panic: None,
            rv32_stack_usage: Vec::new(),
        }
    }

    /// Attach the rv32 stack records read as this frame closed.
    ///
    /// Crate-internal, like `with_fault_panic`: only the frame executor
    /// knows which of the transaction's records are this frame's.
    pub(crate) fn with_rv32_stack_usage(
        mut self,
        records: Vec<Rv32StackUsage>,
    ) -> Self {
        self.rv32_stack_usage = records;
        self
    }

    /// The stack record of the most recent rv32 call made under this frame:
    /// the last of [`rv32_stack_usage_records`](Self::rv32_stack_usage_records).
    /// For a mock frame whose body calls an rv32 contract through its
    /// client, that is the last such call.
    ///
    /// `None` if no rv32 invocation ran under this frame, or if the test
    /// FFI library was built without the diagnostic (the prebuilt one has
    /// it). See [`Rv32StackUsage`] for the fields.
    pub fn rv32_stack_usage(&self) -> Option<Rv32StackUsage> {
        self.rv32_stack_usage.last().copied()
    }

    /// The stack records of the rv32 invocations that ran under this frame,
    /// in the order they finished: a callee's before its caller's.
    ///
    /// "Under this frame" is what its body ran while the frame was open:
    /// the rv32 contracts it called directly, and those called by a mock
    /// contract it invoked. It is never a call that finished before this
    /// frame opened, whichever frame of the transaction made it, and never
    /// a call made under a child frame opened with `child_call`: those are
    /// the child's own result's. Pick a call by its `contract_address` when
    /// the body made several.
    pub fn rv32_stack_usage_records(&self) -> &[Rv32StackUsage] {
        &self.rv32_stack_usage
    }

    /// Attach the panic that faulted this frame.
    ///
    /// Crate-internal: the frame executor is the only thing that may set
    /// this, which is what makes a record here always *this* frame's body.
    pub(crate) fn with_fault_panic(
        mut self,
        panic: Option<PanicRecord>,
    ) -> Self {
        self.fault_panic = panic;
        self
    }

    /// The panic that faulted this frame's own body.
    ///
    /// `None` whenever there is nothing to report rather than nothing to
    /// find, and the two are worth telling apart:
    ///
    /// - the frame did not fault, or faulted with neither a string payload
    ///   nor a hook record;
    /// - the frame faulted because a `raw_Invoke`ed callee did. That callee
    ///   ran in a frame the runtime owns, caught its own panic in
    ///   `invoke_wrapper`, and reported an error code — nothing reaches this
    ///   side. It is still printed to stderr by the panic hook.
    ///
    /// So a record here is always *this* frame's body panicking, never a
    /// nested frame's.
    pub fn fault_panic(&self) -> Option<&PanicRecord> {
        self.fault_panic.as_ref()
    }

    /// Just the message from [`fault_panic`](Self::fault_panic), for callers
    /// that do not want the location.
    pub fn fault_message(&self) -> Option<&str> {
        self.fault_panic.as_ref()?.message.as_deref()
    }

    // -- Status checks (using pattern matching for safety) --

    /// True if the frame returned successfully.
    pub fn is_success(&self) -> bool {
        matches!(&self.output, InvocationFrameOutput::Return { .. })
    }

    /// True if the frame aborted (threw a contract exception).
    pub fn is_abort(&self) -> bool {
        matches!(&self.output, InvocationFrameOutput::Abort { .. })
    }

    /// True if the frame faulted (panicked or trapped, with no abort data).
    pub fn is_fault(&self) -> bool {
        matches!(&self.output, InvocationFrameOutput::Fault)
    }

    // -- Assertions --

    /// Assert the frame returned successfully. Frame-only check; use
    /// [`TransactResult::assert_success`] when the commit outcome matters.
    ///
    /// [`TransactResult::assert_success`]: crate::transact_result::TransactResult::assert_success
    #[track_caller]
    pub fn assert_success(&self) {
        assert!(
            self.is_success(),
            "expected success, got {:?}{}",
            self.output,
            describe_trace(self.trace())
        );
    }

    /// Assert the frame aborted (threw a contract exception). Frame-only
    /// check.
    #[track_caller]
    pub fn assert_abort(&self) {
        assert!(
            self.is_abort(),
            "expected abort, got {:?}{}",
            self.output,
            describe_trace(self.trace())
        );
    }

    /// Assert the frame faulted (panicked or trapped). Frame-only check.
    #[track_caller]
    pub fn assert_fault(&self) {
        assert!(
            self.is_fault(),
            "expected fault, got {:?}{}",
            self.output,
            describe_trace(self.trace())
        );
    }

    // -- Buffer accessors --

    /// Returns the return buffer. Panics if the frame did not succeed.
    pub fn return_buffer(&self) -> &[u8] {
        match &self.output {
            InvocationFrameOutput::Return { return_buffer } => {
                return_buffer.as_slice()
            }
            other => {
                panic!(
                    "return_buffer() called on non-success frame: {other:?}"
                )
            }
        }
    }

    /// Returns the abort buffer. Panics if the frame did not abort.
    pub fn abort_buffer(&self) -> &[u8] {
        match &self.output {
            InvocationFrameOutput::Abort { abort_buffer } => {
                abort_buffer.as_slice()
            }
            other => {
                panic!("abort_buffer() called on non-abort frame: {other:?}")
            }
        }
    }

    // -- Typed return assertion --

    /// Assert the frame succeeded and decode the return buffer as a typed value.
    #[track_caller]
    pub fn assert_return_with<R>(&self, check: impl FnOnce(&R))
    where
        R: xdrs::ReadXDROwned,
    {
        self.assert_success();
        let buf = self.return_buffer();
        let decoded: R = xdrs::cxs::decode(buf).unwrap_or_else(|e| {
            panic!("failed to decode return buffer: {}", e)
        });
        check(&decoded);
    }

    /// Assert the frame succeeded and decode the return buffer, returning
    /// the decoded value. The returning twin of `assert_return_with`.
    #[track_caller]
    pub fn assert_return_as<R>(&self) -> R
    where
        R: xdrs::ReadXDROwned,
    {
        self.assert_success();
        xdrs::cxs::decode(self.return_buffer()).unwrap_or_else(|e| {
            panic!("failed to decode return buffer: {}", e)
        })
    }

    // -- Typed abort assertion --

    /// Assert the frame aborted with a thrown `E` and return it. Panics
    /// unless the frame aborted with a `[CXS(E::SELECTOR)][payload]` buffer
    /// whose payload decodes as `E`.
    #[track_caller]
    pub fn assert_abort_as<E>(&self) -> E
    where
        E: Exn + xdrs::ReadXDROwned,
    {
        self.assert_abort();
        let buf = self.abort_buffer();
        let exn = RawExceptionRef::parse(buf).unwrap_or_else(|e| {
            panic!("abort buffer is not a contract exception ({e}): {buf:?}")
        });
        let name = core::any::type_name::<E>();
        match exn.try_decode::<E>() {
            Some(Ok(e)) => e,
            Some(Err(e)) => {
                panic!("failed to decode abort payload as {name}: {e}")
            }
            None => panic!(
                "expected exception {name} (selector {:#x}), got {exn:?}",
                E::SELECTOR
            ),
        }
    }

    // -- Raw access --

    /// The frame's raw [`InvocationFrameOutput`].
    pub fn output(&self) -> &InvocationFrameOutput {
        &self.output
    }

    /// The frame's execution trace, when tracing was enabled.
    pub fn trace(&self) -> Option<&InvocationFrameTrace> {
        self.trace.as_ref()
    }

    // -- Per-frame events --

    /// The events this frame emitted (empty if the frame reverted).
    pub fn events(&self) -> &[TxEmittedEvent] {
        &self.events
    }

    /// Assert exactly `expected` events were emitted by this frame.
    #[track_caller]
    pub fn assert_event_count(&self, expected: usize) {
        let actual = self.events.len();
        assert_eq!(
            actual, expected,
            "expected {} events, got {}",
            expected, actual
        );
    }

    /// Assert this frame emitted no events.
    #[track_caller]
    pub fn assert_no_events(&self) {
        assert!(
            self.events.is_empty(),
            "expected no events, got {}",
            self.events.len()
        );
    }

    /// All raw events emitted at the given address.
    pub fn events_from(
        &self,
        address: &ContractAddress,
    ) -> Vec<&TxEmittedEvent> {
        self.events
            .iter()
            .filter(|e| &e.contract_address == address)
            .collect()
    }

    /// Collect events from `address` that decode as `E` via
    /// [`TypedEvent::try_new`]: well-formed, topic 0 matching `E`'s declared
    /// discriminant (when [`Event::signature`] returns one), topic count
    /// matching [`Event::num_topics`], and payload decoding as `E::Payload`.
    /// Foreign and malformed events are skipped, never panicked on.
    pub fn filter_events<E>(
        &self,
        address: &ContractAddress,
    ) -> Vec<TypedEvent<'_, E>>
    where
        E: Event,
    {
        self.events
            .iter()
            .filter(|e| &e.contract_address == address)
            .filter_map(|e| TypedEvent::try_new(e))
            .collect()
    }

    /// Inspect a raw emitted event by index.
    #[track_caller]
    pub fn assert_raw_event(
        &self,
        index: usize,
        check: impl FnOnce(&EmittedEvent<'_>),
    ) {
        let events = self.events();
        assert!(
            index < events.len(),
            "event index {} out of range (have {} events)",
            index,
            events.len()
        );
        let emitted = EmittedEvent::from(&events[index]);
        check(&emitted);
    }

    /// Decode an event as a typed event and pass it to the check closure.
    /// Provides access to both topics and the decoded payload.
    #[track_caller]
    pub fn assert_event<E>(
        &self,
        index: usize,
        check: impl FnOnce(&TypedEvent<'_, E>),
    ) where
        E: Event,
    {
        let events = self.events();
        assert!(
            index < events.len(),
            "event index {} out of range (have {} events)",
            index,
            events.len()
        );
        let typed = TypedEvent::<E>::new(&events[index]);
        check(&typed);
    }

    /// Assert that exactly one event from `address` decodes as `E` (per
    /// [`TypedEvent::try_new`] — see [`filter_events`](Self::filter_events)
    /// for the matching rules) and satisfies `predicate`. Panics if zero or
    /// more than one event matches.
    #[track_caller]
    pub fn assert_emitted<E>(
        &self,
        address: &ContractAddress,
        predicate: impl Fn(&TypedEvent<'_, E>) -> bool,
    ) where
        E: Event,
    {
        let match_count = self
            .events
            .iter()
            .filter(|e| &e.contract_address == address)
            .filter_map(|e| TypedEvent::try_new(e))
            .filter(|typed| predicate(typed))
            .count();
        assert!(
            match_count == 1,
            "expected exactly 1 matching event, got {}",
            match_count
        );
    }
}
