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

    /// The static topic-0 discriminant for this event type, if it has one.
    ///
    /// Topic 0 is conventionally the event-type discriminant (see the trait
    /// docs). `#[derive(Event)]` returns `Some(Self::SIGNATURE)`, which lets
    /// [`TypedEvent::new`](crate::TypedEvent::new) reject a raw event whose
    /// topic 0 belongs to a different type before trusting the decoded
    /// payload. Hand-written impls whose topic 0 is data-dependent (computed
    /// from payload fields rather than fixed per type) should leave this as
    /// the default `None`, which opts out of that check.
    fn signature() -> Option<&'static Topic>
    where
        Self: Sized,
    {
        None
    }

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
