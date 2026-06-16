use crate::{Event, Topic};
use groundhog_xdr::event::TxEmittedEvent;

/// Size of each topic in serialized event data
pub const TOPIC_SIZE: usize = 32;

/// A view over a raw `TxEmittedEvent` providing typed access to
/// topics and the XDR-encoded payload.
pub struct EmittedEvent<'a>(&'a TxEmittedEvent);

impl<'a> EmittedEvent<'a> {
    /// Byte length of the topics region, or `None` if the event is malformed:
    /// either it claims more topics than the protocol allows, or the claimed
    /// topics don't fit in `serialized_event`. The cap check also keeps the
    /// `num_topics * TOPIC_SIZE` multiplication from overflowing `usize` on
    /// 32-bit guest targets.
    fn topics_byte_len(&self) -> Option<usize> {
        let event = self.0;
        if event.num_topics > groundhog_xdr::event::kMaxTopics {
            return None;
        }
        let extent = TOPIC_SIZE * event.num_topics as usize;
        if extent > event.serialized_event.len() {
            return None;
        }
        Some(extent)
    }

    /// The XDR-encoded payload (everything after the topics region), or `None`
    /// if the event is malformed (see [`topics_byte_len`](Self::topics_byte_len)).
    ///
    /// Returning `None` rather than silently clamping to an empty slice keeps a
    /// corrupt event from decoding as a valid zero-field payload, and matches
    /// [`topic`](Self::topic)'s out-of-bounds behavior.
    pub fn payload(&self) -> Option<&'a [u8]> {
        let extent = self.topics_byte_len()?;
        Some(&self.0.serialized_event[extent..])
    }

    pub fn topic(&self, index: u32) -> Option<&'a Topic> {
        let event = self.0;

        // A malformed event has no valid topics (consistent with `payload`).
        // This also bounds `num_topics` to the cap, so the offset arithmetic
        // below cannot overflow.
        self.topics_byte_len()?;

        if index >= event.num_topics {
            return None;
        }

        let topic_offset = TOPIC_SIZE * index as usize;
        let topic_extent = topic_offset + TOPIC_SIZE;

        let topic_bytes = &event.serialized_event[topic_offset..topic_extent];

        let topic: &[u8; TOPIC_SIZE] = topic_bytes.try_into().unwrap();

        Some(topic)
    }

    pub fn num_topics(&self) -> u32 {
        self.0.num_topics
    }

    /// Access the underlying raw event.
    pub fn raw(&self) -> &'a TxEmittedEvent {
        self.0
    }
}

impl<'a> From<&'a TxEmittedEvent> for EmittedEvent<'a> {
    fn from(event: &'a TxEmittedEvent) -> Self {
        Self(event)
    }
}

/// A typed view over an emitted event, combining topic access with
/// a pre-decoded payload.
pub struct TypedEvent<'a, E: Event> {
    emitted: EmittedEvent<'a>,
    payload: E::Payload<'a>,
}

impl<'a, E: Event> TypedEvent<'a, E> {
    /// Decode a raw event into a typed view.
    ///
    /// When `E` declares a static topic-0 discriminant (via
    /// [`Event::signature`]), the raw event's topic 0 is verified against it
    /// first, so a payload that merely *shares a shape* with `E` cannot be
    /// silently mistyped as `E`. Panics on a discriminant mismatch or on
    /// payload decode failure.
    #[track_caller]
    pub fn new(raw: &'a TxEmittedEvent) -> Self {
        let emitted = EmittedEvent::from(raw);
        // Reject a malformed event up front, with a clear message, before the
        // signature/payload checks (which would otherwise report a misleading
        // "type mismatch" or decode a silently-empty payload).
        let payload_bytes = emitted.payload().unwrap_or_else(|| {
            panic!(
                "TypedEvent::new: malformed event — {} topics do not fit a \
                 {}-byte body",
                raw.num_topics,
                raw.serialized_event.len(),
            )
        });
        if let Some(expected) = E::signature() {
            match emitted.topic(0) {
                Some(actual) if actual == expected => {}
                other => panic!(
                    "TypedEvent::new: event type mismatch — topic 0 {:?} does \
                     not match the expected discriminant {:?} for {}",
                    other,
                    expected,
                    core::any::type_name::<E>(),
                ),
            }
        }
        let payload = E::decode_payload(payload_bytes).unwrap_or_else(|e| {
            panic!("failed to decode event payload: {}", e)
        });
        Self { emitted, payload }
    }

    /// The decoded payload.
    pub fn payload(&self) -> &E::Payload<'a> {
        &self.payload
    }

    /// Get topic at the given index.
    pub fn topic(&self, index: u32) -> Option<&'a Topic> {
        self.emitted.topic(index)
    }

    /// Number of topics.
    pub fn num_topics(&self) -> u32 {
        self.emitted.num_topics()
    }

    /// Access the underlying untyped emitted event.
    pub fn raw(&self) -> &EmittedEvent<'a> {
        &self.emitted
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use groundhog_xdr::event::TxEmittedEvent;
    use groundhog_xdr::types::ContractAddress;

    // `&'static [u8]` so this builds whether or not BytesM's `alloc` backing is
    // enabled for the standalone test build.
    fn raw_event(num_topics: u32, body: &'static [u8]) -> TxEmittedEvent {
        TxEmittedEvent {
            contract_address: ContractAddress::default(),
            num_topics,
            serialized_event: body.try_into().unwrap(),
        }
    }

    #[test]
    fn payload_is_none_when_topics_overrun_buffer() {
        // Claims 3 topics (96 bytes) but the body is only 40.
        static BODY: [u8; 40] = [0u8; 40];
        let raw = raw_event(3, &BODY);
        let ev = EmittedEvent::from(&raw);
        assert_eq!(ev.payload(), None);
        // topic() agrees — no silent disagreement between the two accessors.
        assert_eq!(ev.topic(0), None);
    }

    #[test]
    fn payload_is_none_when_num_topics_exceeds_cap() {
        // 4 > kMaxTopics (3): malformed regardless of body length.
        static BODY: [u8; 200] = [0u8; 200];
        let raw = raw_event(4, &BODY);
        let ev = EmittedEvent::from(&raw);
        assert_eq!(ev.payload(), None);
    }

    #[test]
    fn payload_is_some_for_well_formed_event() {
        // One topic (32 bytes) followed by a 4-byte payload.
        static BODY: [u8; 36] = {
            let mut b = [0u8; 36];
            b[32] = 1;
            b[33] = 2;
            b[34] = 3;
            b[35] = 4;
            b
        };
        let raw = raw_event(1, &BODY);
        let ev = EmittedEvent::from(&raw);
        assert_eq!(ev.payload(), Some(&[1u8, 2, 3, 4][..]));
        assert!(ev.topic(0).is_some());
        assert_eq!(ev.topic(1), None);
    }
}
