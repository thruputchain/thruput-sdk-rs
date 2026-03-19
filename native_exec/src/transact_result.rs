use crate::frame_result::FrameResult;
use groundhog_xdr::event::{TxEmittedEvent, TxEventLog};
use groundhog_xdr::exec_result::ExecResult;

/// Result of a full transaction: root frame result + commit outcome + events.
pub struct TransactResult {
    frame: FrameResult,
    exec_result: ExecResult,
    event_log: Option<TxEventLog>,
}

impl TransactResult {
    pub fn new(
        frame: FrameResult,
        exec_result: ExecResult,
        event_log: Option<TxEventLog>,
    ) -> Self {
        Self {
            frame,
            exec_result,
            event_log,
        }
    }

    /// Asserts the root frame returned successfully AND the transaction committed OK.
    pub fn assert_success(&self) {
        self.frame.assert_success();
        assert_eq!(
            self.exec_result,
            ExecResult::ExecResultOK,
            "expected successful commit, got {:?}",
            self.exec_result
        );
    }

    pub fn assert_abort(&self) {
        self.frame.assert_abort();
    }

    pub fn assert_fault(&self) {
        self.frame.assert_fault();
    }

    pub fn assert_abort_with<E>(&self, check: impl FnOnce(&E))
    where
        E: xdrs::ReadXDROwned,
    {
        self.frame.assert_abort_with(check);
    }

    /// Asserts the root frame succeeded but the DB commit step failed.
    pub fn assert_commit_failed(&self) {
        self.frame.assert_success();
        assert_ne!(
            self.exec_result,
            ExecResult::ExecResultOK,
            "expected commit failure, but commit succeeded"
        );
    }

    pub fn exec_result(&self) -> &ExecResult {
        &self.exec_result
    }

    pub fn frame(&self) -> &FrameResult {
        &self.frame
    }

    // -- Events --

    pub fn events(&self) -> &[TxEmittedEvent] {
        match &self.event_log {
            Some(log) => &log.events,
            None => &[],
        }
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
        E: xdrs::ReadXDROwned,
    {
        let events = self.events();
        let mut decoded_count = 0usize;
        for event in events {
            let topics_len = 32 * event.num_topics as usize;
            if topics_len > event.serialized_event.len() {
                continue;
            }
            let payload = &event.serialized_event[topics_len..];
            if let Ok(decoded) = xdrs::txs::decode::<E>(payload) {
                decoded_count += 1;
                if predicate(&decoded) {
                    return;
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
