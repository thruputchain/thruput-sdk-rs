#![no_std]

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
    /// Number of topics this event carries (0..=3).
    fn num_topics(&self) -> u32;

    /// Retrieve the topic at the given index.
    fn topic<'a>(&'a self, index: u32) -> Option<TopicSrc<'a>> {
        let _ = index;
        None
    }
}
