use groundhog_event::{EmittedEvent, Event, TypedEvent};
use groundhog_trace::InvocationFrameTrace;
use groundhog_xdr::event::TxEmittedEvent;
use groundhog_xdr::transaction::InvocationFrameOutput;
use groundhog_xdr::types::ContractAddress;

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
        }
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

    // -- Typed abort assertion --

    /// Assert the frame aborted and decode the abort buffer as a typed error.
    #[track_caller]
    pub fn assert_abort_with<E>(&self, check: impl FnOnce(&E))
    where
        E: xdrs::ReadXDROwned,
    {
        self.assert_abort();
        let buf = self.abort_buffer();
        let decoded: E = xdrs::cxs::decode(buf).unwrap_or_else(|e| {
            panic!("failed to decode abort buffer: {}", e)
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

    /// Assert the frame aborted and decode the abort buffer, returning
    /// the decoded error. The returning twin of `assert_abort_with`.
    #[track_caller]
    pub fn assert_abort_as<E>(&self) -> E
    where
        E: xdrs::ReadXDROwned,
    {
        self.assert_abort();
        xdrs::cxs::decode(self.abort_buffer())
            .unwrap_or_else(|e| panic!("failed to decode abort buffer: {}", e))
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
