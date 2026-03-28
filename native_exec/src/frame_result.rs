use groundhog_event::{EmittedEvent, Event, TypedEvent};
use groundhog_trace::InvocationFrameTrace;
use groundhog_xdr::event::TxEmittedEvent;
use groundhog_xdr::transaction::InvocationFrameOutput;
use groundhog_xdr::types::ContractAddress;

/// Check whether a decoded event's topic 0 matches the raw emitted event's
/// topic 0. Returns true if the topics are consistent:
/// - Both have 0 topics -> match
/// - Decoded declares topics and topic 0 matches raw -> match
/// - Mismatch in topic count or topic 0 value -> no match
fn topic0_matches<E: Event>(decoded: &E, raw: &TxEmittedEvent) -> bool {
    if decoded.num_topics() != raw.num_topics {
        return false;
    }
    if decoded.num_topics() > 0 {
        if let Some(expected) = decoded.topic(0) {
            let expected_bytes: [u8; 32] = expected.into();
            if raw.serialized_event.len() < 32 {
                return false;
            }
            if raw.serialized_event[..32] != expected_bytes {
                return false;
            }
            return true;
        }
        panic!(
            "invalid event implementation: claimed to have topics but did not provide topic 0"
        );
    } else {
        // Event type has no topics — only match raw events with no topics
        assert!(raw.num_topics == 0);
        true
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

    pub fn is_success(&self) -> bool {
        matches!(&self.output, InvocationFrameOutput::Return { .. })
    }

    pub fn is_abort(&self) -> bool {
        matches!(&self.output, InvocationFrameOutput::Abort { .. })
    }

    pub fn is_fault(&self) -> bool {
        matches!(&self.output, InvocationFrameOutput::Fault)
    }

    // -- Assertions --

    #[track_caller]
    pub fn assert_success(&self) {
        assert!(
            self.is_success(),
            "expected success, got status code {}",
            self.output.status_code()
        );
    }

    #[track_caller]
    pub fn assert_abort(&self) {
        assert!(
            self.is_abort(),
            "expected abort, got status code {}",
            self.output.status_code()
        );
    }

    #[track_caller]
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
            InvocationFrameOutput::Return { return_buffer } => {
                return_buffer.as_slice()
            }
            _ => panic!(
                "return_buffer() called on non-success frame (status {})",
                self.output.status_code()
            ),
        }
    }

    /// Returns the abort buffer. Panics if the frame did not abort.
    pub fn abort_buffer(&self) -> &[u8] {
        match &self.output {
            InvocationFrameOutput::Abort { abort_buffer } => {
                abort_buffer.as_slice()
            }
            _ => panic!(
                "abort_buffer() called on non-abort frame (status {})",
                self.output.status_code()
            ),
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
        let decoded: R = xdrs::txs::decode(buf).unwrap_or_else(|e| {
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

    // -- Per-frame events --

    pub fn events(&self) -> &[TxEmittedEvent] {
        &self.events
    }

    #[track_caller]
    pub fn assert_event_count(&self, expected: usize) {
        let actual = self.events.len();
        assert_eq!(
            actual, expected,
            "expected {} events, got {}",
            expected, actual
        );
    }

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

    /// Collect events from `address` whose payload decodes as `E` and whose
    /// topic 0 matches the `Event` trait's declared topic 0.
    pub fn filter_events<E>(&self, address: &ContractAddress) -> Vec<E>
    where
        E: xdrs::ReadXDROwned + groundhog_event::Event,
    {
        self.events
            .iter()
            .filter(|e| &e.contract_address == address)
            .filter_map(|event| {
                let topics_len = 32 * event.num_topics as usize;
                if topics_len > event.serialized_event.len() {
                    return None;
                }
                let payload = &event.serialized_event[topics_len..];
                let decoded = xdrs::cxs::decode::<E>(payload).ok()?;
                if !topic0_matches(&decoded, event) {
                    return None;
                }
                Some(decoded)
            })
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

    /// Assert that exactly one event from `address` matches `predicate`
    /// when decoded as `E` (with topic 0 check). Panics if zero or more
    /// than one event matches.
    #[track_caller]
    pub fn assert_emitted<E>(
        &self,
        address: &ContractAddress,
        predicate: impl Fn(&E) -> bool,
    ) where
        E: xdrs::ReadXDROwned + groundhog_event::Event,
    {
        let mut match_count = 0usize;
        for event in &self.events {
            if &event.contract_address != address {
                continue;
            }
            let topics_len = 32 * event.num_topics as usize;
            assert!(
                topics_len <= event.serialized_event.len(),
                "assert_emitted: event has topics_len {} but serialized_event is only {} bytes",
                topics_len,
                event.serialized_event.len()
            );
            let payload = &event.serialized_event[topics_len..];
            if let Ok(decoded) = xdrs::cxs::decode::<E>(payload)
                && topic0_matches(&decoded, event)
                && predicate(&decoded)
            {
                match_count += 1;
            }
        }
        assert!(
            match_count == 1,
            "expected exactly 1 matching event, got {}",
            match_count
        );
    }
}
