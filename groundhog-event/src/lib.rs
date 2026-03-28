#![no_std]

mod emitted_event;
pub use emitted_event::{EmittedEvent, TOPIC_SIZE, TypedEvent};

/// A topic is a fixed-length, opaque 32-byte array.
pub type Topic = [u8; 32];

/// An `impl Event` publishes each topic as a `TopicSrc` value.
pub enum TopicSrc<'a> {
    /// The topic is this owned value.
    Owned(Topic),
    /// The topic is available via a reference into the event
    /// (or a `'static` reference).
    Shared(&'a Topic),
}

// A `TopicSrc` can be converted to an owned `Topic`.
impl<'a> From<TopicSrc<'a>> for Topic {
    fn from(src: TopicSrc<'a>) -> Topic {
        match src {
            TopicSrc::Owned(topic) => topic,
            TopicSrc::Shared(topic) => *topic,
        }
    }
}

/// Trait for types that can be converted into an event topic.
///
/// Types that are already 32 bytes (like `ContractAddress`) implement
/// this as a zero-copy borrow. Other types should XDR-encode and hash
/// to produce a 32-byte topic.
pub trait IntoTopic {
    fn into_topic(&self) -> TopicSrc<'_>;
}

impl IntoTopic for [u8; 32] {
    fn into_topic(&self) -> TopicSrc<'_> {
        TopicSrc::Shared(self)
    }
}

/// Trait implemented by event types that can be emitted by contracts.
///
/// Provides the number of topics and their values. Topic 0 is
/// conventionally the event-type discriminant (e.g. a hash of the
/// event name). The XDR-serialized payload is everything *after*
/// the topics in the on-chain representation.
pub trait Event: xdrs::WriteXDR {
    /// The decoded payload type. Uses a GAT so it can borrow from
    /// the input bytes (e.g. `&'a [u8]` fields stay borrowed).
    type Payload<'a>;

    /// Number of topics this event carries (0..=3).
    fn num_topics(&self) -> u32;

    /// Retrieve the topic at the given index.
    fn topic<'a>(&'a self, index: u32) -> Option<TopicSrc<'a>> {
        let _ = index;
        None
    }

    /// Decode the payload portion of a serialized event.
    fn decode_payload<'a>(
        bytes: &'a [u8],
    ) -> Result<Self::Payload<'a>, xdrs::ReadError>;
}
