use core::cmp::min;
use groundhog_event::Topic;
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

        if index > event.num_topics {
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
