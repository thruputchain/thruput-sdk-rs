use crate::exn::*;
use crate::internal::encode_xdr;
use crate::sys::{self, SyscallNo};

// Re-export core event types from groundhog-event
pub use groundhog_event::{EmittedEvent, Event, Topic, TopicSrc, TypedEvent};

/// Trait for types that can be converted into an event topic.
///
/// Types that are already 32 bytes (like `ContractAddress`) implement
/// this as a zero-copy borrow. Other types are XDR-encoded and SHA-256
/// hashed to produce a 32-byte topic.
pub trait IntoTopic {
    #[allow(clippy::wrong_self_convention)]
    fn into_topic(&self) -> TopicSrc<'_>;
}

impl IntoTopic for [u8; 32] {
    fn into_topic(&self) -> TopicSrc<'_> {
        TopicSrc::Shared(self)
    }
}

/// Maximum number of topics per event
pub const EVENT_MAX_TOPICS: u32 = groundhog_xdr::event::kMaxTopics;

/// Maximum size of serialized event data (including the topics)
pub const EVENT_MAX_DATA: u32 = groundhog_xdr::event::kMaxEventLength;

/// Convert any `WriteXDR` value into a topic by XDR-encoding and hashing.
pub fn hash_to_topic<T: crate::WriteXDR>(val: &T) -> Topic {
    let buf = crate::internal::encode_xdr(val).unwrap();
    crate::crypto::sha256(buf.as_ref()).into_inner()
}

// --- IntoTopic impls for common types ---
// These XDR-encode the value and SHA-256 hash it to produce a 32-byte topic.

macro_rules! impl_into_topic_via_hash {
    ($($ty:ty),*) => {
        $(
            impl IntoTopic for $ty {
                fn into_topic(&self) -> TopicSrc<'_> {
                    TopicSrc::Owned(hash_to_topic(self))
                }
            }
        )*
    };
}

impl_into_topic_via_hash!(u128, i128, u64, i64, u32, i32, bool);

impl IntoTopic for str {
    fn into_topic(&self) -> TopicSrc<'_> {
        TopicSrc::Owned(hash_to_topic(&self))
    }
}

impl IntoTopic for [u8] {
    fn into_topic(&self) -> TopicSrc<'_> {
        TopicSrc::Owned(hash_to_topic(&self))
    }
}

/// Emit an event
pub fn log_event(event: &impl Event) {
    let n = event.num_topics();

    let topic = |i| {
        if i < n {
            let t = event.topic(i)?;
            Some(t.into())
        } else {
            None
        }
    };

    let topic0: Option<Topic> = topic(0);
    let topic1: Option<Topic> = topic(1);
    let topic2: Option<Topic> = topic(2);

    let data = encode_xdr(event).unwrap(); // XXX

    sys::uninit_LogEvent(
        topic0.as_ref(),
        topic1.as_ref(),
        topic2.as_ref(),
        data.as_ref(),
    )
    .unwrap_or_else(|errno| {
        syscall_throw(SyscallNo::kLogEvent, errno);
    });
}
