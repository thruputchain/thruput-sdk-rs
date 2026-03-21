use groundhog_event::Event;
use groundhog_trace::InvocationFrameTrace;
use groundhog_xdr::event::TxEmittedEvent;
use groundhog_xdr::transaction::InvocationFrameOutput;

/// Check whether a decoded event's topic 0 matches the raw emitted event's
/// topic 0. Returns true if the topics are consistent:
/// - Both have 0 topics -> match
/// - Decoded declares topics and topic 0 matches raw -> match
/// - Mismatch in topic count direction or topic 0 value -> no match
fn topic0_matches<E: Event>(decoded: &E, raw: &TxEmittedEvent) -> bool {
    if decoded.num_topics() > 0 {
        if raw.num_topics == 0 {
            return false;
        }
        if let Some(expected) = decoded.topic(0) {
            let expected_bytes: [u8; 32] = expected.into();
            if raw.serialized_event.len() < 32 {
                return false;
            }
            if &raw.serialized_event[..32] != &expected_bytes {
                return false;
            }
        }
        // If num_topics > 0 but topic(0) returns None, this is a
        // degenerate Event impl. We allow it through — the type declared
        // it has topics but didn't provide one.
        true
    } else {
        // Event type has no topics — only match raw events with no topics
        raw.num_topics == 0
    }
}

/// Result of executing a single invocation frame.
/// Wraps InvocationFrameOutput + optional trace + per-frame events.
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

    pub fn assert_event_count(&self, expected: usize) {
        let actual = self.events.len();
        assert_eq!(
            actual, expected,
            "expected {} events, got {}",
            expected, actual
        );
    }

    pub fn assert_no_events(&self) {
        assert!(
            self.events.is_empty(),
            "expected no events, got {}",
            self.events.len()
        );
    }

    /// Collect all events whose payload decodes as `E` and whose topic 0
    /// matches the `Event` trait's declared topic 0. Events that don't
    /// match are silently skipped.
    pub fn filter_events<E>(&self) -> Vec<E>
    where
        E: xdrs::ReadXDROwned + groundhog_event::Event,
    {
        self.events
            .iter()
            .filter_map(|event| {
                let topics_len = 32 * event.num_topics as usize;
                if topics_len > event.serialized_event.len() {
                    return None;
                }
                let payload = &event.serialized_event[topics_len..];
                let decoded = xdrs::txs::decode::<E>(payload).ok()?;
                if !topic0_matches(&decoded, event) {
                    return None;
                }
                Some(decoded)
            })
            .collect()
    }

    pub fn assert_event(
        &self,
        index: usize,
        check: impl FnOnce(&TxEmittedEvent),
    ) {
        let events = self.events();
        assert!(
            index < events.len(),
            "event index {} out of range (have {} events)",
            index,
            events.len()
        );
        check(&events[index]);
    }

    pub fn assert_event_data<E>(&self, index: usize, check: impl FnOnce(&E))
    where
        E: xdrs::ReadXDROwned,
    {
        let events = self.events();
        assert!(
            index < events.len(),
            "event index {} out of range (have {} events)",
            index,
            events.len()
        );
        let event = &events[index];
        let topics_len = 32 * event.num_topics as usize;
        assert!(
            topics_len <= event.serialized_event.len(),
            "event {} topics_len {} exceeds serialized_event length {}",
            index,
            topics_len,
            event.serialized_event.len()
        );
        let payload = &event.serialized_event[topics_len..];
        let decoded: E = xdrs::txs::decode(payload)
            .unwrap_or_else(|e| panic!("failed to decode event data: {}", e));
        check(&decoded);
    }

    pub fn assert_emitted<E>(&self, predicate: impl Fn(&E) -> bool)
    where
        E: xdrs::ReadXDROwned + groundhog_event::Event,
    {
        let events = self.events();
        let mut decoded_count = 0usize;
        for event in events {
            let topics_len = 32 * event.num_topics as usize;
            assert!(
                topics_len <= event.serialized_event.len(),
                "assert_emitted: event has topics_len {} but serialized_event is only {} bytes",
                topics_len,
                event.serialized_event.len()
            );
            let payload = &event.serialized_event[topics_len..];
            if let Ok(decoded) = xdrs::txs::decode::<E>(payload) {
                if topic0_matches(&decoded, event) {
                    decoded_count += 1;
                    if predicate(&decoded) {
                        return;
                    }
                }
            }
        }
        if decoded_count == 0 {
            panic!(
                "no event matched the predicate: 0 of {} events could be decoded as the expected type",
                events.len()
            );
        }
        panic!(
            "no event matched the predicate (decoded {}/{} events, none passed predicate)",
            decoded_count,
            events.len()
        );
    }
}
