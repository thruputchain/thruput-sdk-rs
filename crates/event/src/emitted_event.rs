use core::cmp::min;

use crate::{Event, Topic};
use groundhog_xdr::event::TxEmittedEvent;

/// Size of each topic in serialized event data
pub const TOPIC_SIZE: usize = 32;

/// A view over a raw `TxEmittedEvent` providing typed access to
/// topics and the XDR-encoded payload.
pub struct EmittedEvent<'a>(&'a TxEmittedEvent);

impl<'a> EmittedEvent<'a> {
    pub fn payload(&self) -> &'a [u8] {
        let event = self.0;

        let topics_extent = min(
            TOPIC_SIZE * event.num_topics as usize,
            event.serialized_event.len(),
        );

        &event.serialized_event[topics_extent..]
    }

    pub fn topic(&self, index: u32) -> Option<&'a Topic> {
        let event = self.0;

        if index >= event.num_topics {
            return None;
        }

        let topic_offset = TOPIC_SIZE * index as usize;
        let topic_extent = topic_offset + TOPIC_SIZE;

        if topic_extent > event.serialized_event.len() {
            return None;
        }

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
    pub fn new(raw: &'a TxEmittedEvent) -> Self {
        let emitted = EmittedEvent::from(raw);
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
        let payload_bytes = emitted.payload();
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
