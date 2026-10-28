//! Event types for Thruput smart contracts.
//!
//! An event is a list of up to three 32-byte topics followed by an
//! XDR-encoded payload. Event types implement [`Event`]; [`EmittedEvent`]
//! is an event as read back from a transaction.
//!
//! Contract authors normally derive [`Event`] through `thruput-sdk`.

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

    /// Number of topics this event type carries (0..=3), including the
    /// signature topic when present.
    ///
    /// This is a static property of the type: every emission carries the
    /// same count, and typed-event matching ([`TypedEvent::try_new`] /
    /// [`TypedEvent::new`]) rejects a raw event whose `num_topics` differs —
    /// so a matching topic 0 plus a coincidentally decodable payload cannot
    /// pass for an `E` emitted with a different topic count.
    fn num_topics() -> u32
    where
        Self: Sized;

    /// The static topic-0 discriminant for this event type, if it has one.
    ///
    /// Topic 0 is conventionally the event-type discriminant (see the trait
    /// docs). Return `Some(sig)` when topic 0 is a fixed per-type constant:
    /// [`TypedEvent::new`](crate::TypedEvent::new) then rejects a raw event
    /// whose topic 0 belongs to a different type before trusting the decoded
    /// payload. Return `None` only when topic 0 is data-dependent (computed
    /// from payload fields, or absent), which opts out of that check.
    ///
    /// This is deliberately not defaulted: a silent `None` would let an impl
    /// with a fixed discriminant skip the check by omission. `#[derive(Event)]`
    /// implements it as `Some(Self::SIGNATURE)`.
    fn signature() -> Option<&'static Topic>
    where
        Self: Sized;

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
