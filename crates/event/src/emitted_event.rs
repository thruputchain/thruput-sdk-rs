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
    /// if the event is malformed (claims more topics than the protocol allows,
    /// or the claimed topics don't fit in `serialized_event`).
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

/// Why constructing a `TypedEvent` from a raw event failed. Internal:
/// surfaced only through [`TypedEvent::new`]'s panic message (the `Display`
/// texts below are that message and must not change) or discarded by
/// [`TypedEvent::try_new`].
enum BuildError {
    /// The event's claimed topics don't fit its body.
    Malformed { num_topics: u32, body_len: usize },
    /// Topic 0 doesn't match `E`'s static discriminant.
    SignatureMismatch {
        actual: Option<Topic>,
        expected: &'static Topic,
        type_name: &'static str,
    },
    /// The payload bytes don't decode as `E::Payload`.
    Decode(xdrs::ReadError),
}

impl core::fmt::Display for BuildError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            BuildError::Malformed {
                num_topics,
                body_len,
            } => write!(
                f,
                "TypedEvent::new: malformed event — {} topics do not fit a \
                 {}-byte body",
                num_topics, body_len,
            ),
            BuildError::SignatureMismatch {
                actual,
                expected,
                type_name,
            } => write!(
                f,
                "TypedEvent::new: event type mismatch — topic 0 {:?} does \
                 not match the expected discriminant {:?} for {}",
                actual, expected, type_name,
            ),
            BuildError::Decode(e) => {
                write!(f, "failed to decode event payload: {}", e)
            }
        }
    }
}

/// A typed view over an emitted event, combining topic access with
/// a pre-decoded payload.
pub struct TypedEvent<'a, E: Event> {
    emitted: EmittedEvent<'a>,
    payload: E::Payload<'a>,
}

impl<'a, E: Event> TypedEvent<'a, E> {
    /// Decode a raw event into a typed view, or say why it isn't an `E`.
    /// Shared engine behind [`new`](Self::new) (panics with the error's
    /// `Display` text) and [`try_new`](Self::try_new) (discards it).
    fn build(raw: &'a TxEmittedEvent) -> Result<Self, BuildError> {
        let emitted = EmittedEvent::from(raw);
        // Reject a malformed event up front, before the signature/payload
        // checks (which would otherwise report a misleading "type mismatch"
        // or decode a silently-empty payload).
        let payload_bytes =
            emitted.payload().ok_or(BuildError::Malformed {
                num_topics: raw.num_topics,
                body_len: raw.serialized_event.len(),
            })?;
        if let Some(expected) = E::signature() {
            match emitted.topic(0) {
                Some(actual) if actual == expected => {}
                other => {
                    return Err(BuildError::SignatureMismatch {
                        actual: other.copied(),
                        expected,
                        type_name: core::any::type_name::<E>(),
                    });
                }
            }
        }
        let payload =
            E::decode_payload(payload_bytes).map_err(BuildError::Decode)?;
        Ok(Self { emitted, payload })
    }

    /// Decode a raw event into a typed view.
    ///
    /// When `E` declares a static topic-0 discriminant (via
    /// [`Event::signature`]), the raw event's topic 0 is verified against it
    /// first, so a payload that merely *shares a shape* with `E` cannot be
    /// silently mistyped as `E`. Panics on a discriminant mismatch or on
    /// payload decode failure.
    #[track_caller]
    pub fn new(raw: &'a TxEmittedEvent) -> Self {
        match Self::build(raw) {
            Ok(typed) => typed,
            Err(e) => panic!("{}", e),
        }
    }

    /// Non-panicking twin of [`new`](Self::new): `Some` only if the event is
    /// well-formed, its topic 0 matches `E`'s declared discriminant (when
    /// one exists), and its payload decodes as `E::Payload`. The probe for
    /// scanning event streams that contain foreign events.
    ///
    /// When [`Event::signature`] is `None`, matching is payload-decode-only:
    /// with no decoded `E` instance there is nothing to cross-check topic
    /// counts against.
    pub fn try_new(raw: &'a TxEmittedEvent) -> Option<Self> {
        Self::build(raw).ok()
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

    const SIG: Topic = [0xAB; 32];

    /// Minimal Event impl with a static signature and a 4-byte big-endian
    /// u32 payload. Decode is hand-rolled so these no_std tests don't need
    /// cxs encode (which requires alloc).
    struct SigEvent;

    impl xdrs::WriteXDR for SigEvent {
        fn write_xdr<F, W: xdrs::Writer<F>>(
            &self,
            _w: &mut W,
        ) -> xdrs::WriteResult<()> {
            Ok(())
        }
    }

    impl Event for SigEvent {
        type Payload<'a> = u32;

        fn num_topics(&self) -> u32 {
            1
        }

        fn signature() -> Option<&'static Topic> {
            Some(&SIG)
        }

        fn decode_payload(bytes: &[u8]) -> Result<u32, xdrs::ReadError> {
            match <[u8; 4]>::try_from(bytes) {
                Ok(b) => Ok(u32::from_be_bytes(b)),
                Err(_) => Err(xdrs::ReadError::SourceEmpty {
                    requested: 4,
                    remaining: bytes.len(),
                }),
            }
        }
    }

    #[test]
    fn try_new_decodes_matching_event() {
        // Topic 0 = SIG (32 × 0xAB), payload = 7u32 big-endian.
        static BODY: [u8; 36] = {
            let mut b = [0xABu8; 36];
            b[32] = 0;
            b[33] = 0;
            b[34] = 0;
            b[35] = 7;
            b
        };
        let raw = raw_event(1, &BODY);
        let typed =
            TypedEvent::<SigEvent>::try_new(&raw).expect("should decode");
        assert_eq!(*typed.payload(), 7);
        assert_eq!(typed.topic(0), Some(&SIG));
    }

    #[test]
    fn try_new_rejects_signature_mismatch() {
        // Topic 0 is 32 × 0xCD ≠ SIG; payload bytes are valid u32.
        static BODY: [u8; 36] = [0xCD; 36];
        let raw = raw_event(1, &BODY);
        assert!(TypedEvent::<SigEvent>::try_new(&raw).is_none());
    }

    #[test]
    fn try_new_rejects_malformed_event() {
        // Claims 3 topics (96 bytes) but the body is only 40.
        static BODY: [u8; 40] = [0u8; 40];
        let raw = raw_event(3, &BODY);
        assert!(TypedEvent::<SigEvent>::try_new(&raw).is_none());
    }

    #[test]
    fn try_new_rejects_undecodable_payload() {
        // Topic 0 = SIG but only 3 payload bytes — decode_payload fails.
        static BODY: [u8; 35] = [0xAB; 35];
        let raw = raw_event(1, &BODY);
        assert!(TypedEvent::<SigEvent>::try_new(&raw).is_none());
    }

    #[test]
    #[should_panic(expected = "event type mismatch")]
    fn new_still_panics_on_signature_mismatch() {
        static BODY: [u8; 36] = [0xCD; 36];
        let raw = raw_event(1, &BODY);
        let _ = TypedEvent::<SigEvent>::new(&raw);
    }
}
