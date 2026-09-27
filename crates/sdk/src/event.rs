use crate::call::with_scratch;
use crate::exn::*;
use crate::internal::encode_scratch;
use crate::re_throw::OrSyscallThrow;
use crate::sys::{self, SyscallNo};

// Re-export core event types from groundhog-event
pub use groundhog_event::{EmittedEvent, Event, Topic, TopicSrc, TypedEvent};

/// Trait for types that can be converted into an event topic.
///
/// `u32`, `u64`, `u128` and `U256` use zero-extended, 32-byte big-endian encoding.
/// `i32`, `i64`, `i128` and `I256` use sign-extended, 32-byte big-endian encoding.
/// `bool` uses the same representation for 0 or 1. Other primitive integer types
/// are not supported as topics.
/// Byte arrays of length 32, addresses and hashes borrow their bytes directly.
/// Strings and byte slices are always XDR-encoded and SHA-256 hashed.
#[diagnostic::on_unimplemented(
    message = "`{Self}` cannot be an event topic",
    // No list of accepted types: the impls are explicit, so rustc prints them
    // itself. A hand-kept copy only drifts.
    note = "either widen the field to a listed type, or drop `#[topic]` and let it ride in the event payload, where it stays readable"
)]
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
    with_scratch(|buf| {
        encode_scratch(buf, val).unwrap_or_else(|e| encoding_event_throw(e));
        crate::crypto::sha256(buf.as_slice()).into_inner()
    })
}

// Widen before encoding to reuse U256/I256's zero/sign extension and byte order.
macro_rules! impl_into_topic_direct {
    ($wide:ty; $($ty:ty),*) => {
        $(
            impl IntoTopic for $ty {
                fn into_topic(&self) -> TopicSrc<'_> {
                    let value: $wide = (*self).into();
                    TopicSrc::Owned(value.to_be_bytes())
                }
            }
        )*
    };
}

impl_into_topic_direct!(crate::U256; u128, u64, u32, bool);
impl_into_topic_direct!(crate::I256; i128, i64, i32);

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

// The note above names `ContractAddress` and `SHA256Hash` as valid topics, but
// they only reached an impl through auto-deref on the method call, so a
// `T: IntoTopic` bound rejected them. Real impls make the documented set true
// and let the derive state the bound.
impl IntoTopic for crate::types::ContractAddress {
    fn into_topic(&self) -> TopicSrc<'_> {
        (**self).into_topic()
    }
}

impl IntoTopic for crate::types::SHA256Hash {
    fn into_topic(&self) -> TopicSrc<'_> {
        (**self).into_topic()
    }
}

impl IntoTopic for crate::U256 {
    fn into_topic(&self) -> TopicSrc<'_> {
        TopicSrc::Owned(self.to_be_bytes())
    }
}

impl IntoTopic for crate::I256 {
    fn into_topic(&self) -> TopicSrc<'_> {
        TopicSrc::Owned(self.to_be_bytes())
    }
}

impl<T: IntoTopic + ?Sized> IntoTopic for &T {
    fn into_topic(&self) -> TopicSrc<'_> {
        (**self).into_topic()
    }
}

/// Emit an event.
///
/// Groundhog currently supports up to three total topics per event. Derived
/// events typically reserve topic 0 for the event signature, leaving up to two
/// additional user-declared `#[topic]` fields.
///
/// Topic fields use their [`IntoTopic`] representation: supported scalar types
/// are encoded directly, while strings and byte slices are hashed.
pub fn log_event<E: Event>(event: &E) {
    let n = E::num_topics();

    if n > EVENT_MAX_TOPICS {
        encoding_event_throw(format_args!(
            "event declared {n} topics, but the maximum is {EVENT_MAX_TOPICS}"
        ));
    }

    let topic = |i| {
        if i < n {
            let t = event.topic(i);
            debug_assert!(
                t.is_some(),
                "event declared {n} topics, but topic({i}) returned None"
            );
            Some(t?.into())
        } else {
            None
        }
    };

    let topic0: Option<Topic> = topic(0);
    let topic1: Option<Topic> = topic(1);
    let topic2: Option<Topic> = topic(2);

    with_scratch(|buf| {
        encode_scratch(buf, event).unwrap_or_else(|e| encoding_event_throw(e));
        sys::uninit_LogEvent(
            topic0.as_ref(),
            topic1.as_ref(),
            topic2.as_ref(),
            buf.as_slice(),
        )
        .or_syscall_throw(SyscallNo::kLogEvent);
    });
}
