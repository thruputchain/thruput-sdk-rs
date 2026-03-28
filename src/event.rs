use crate::exn::*;
use crate::internal::encode_xdr;
use crate::sys::{self, SyscallNo};

// Re-export core event types from groundhog-event
pub use groundhog_event::{EmittedEvent, Event, IntoTopic, Topic, TopicSrc};

/// Maximum number of topics per event
pub const EVENT_MAX_TOPICS: u32 = groundhog_xdr::event::kMaxTopics;

/// Maximum size of serialized event data (including the topics)
pub const EVENT_MAX_DATA: u32 = groundhog_xdr::event::kMaxEventLength;

/// Convert any `WriteXDR` value into a topic by XDR-encoding and hashing.
pub fn hash_to_topic<T: crate::WriteXDR>(val: &T) -> Topic {
    let buf = crate::internal::encode_xdr(val).unwrap();
    crate::crypto::sha256(buf.as_ref()).into_inner()
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
