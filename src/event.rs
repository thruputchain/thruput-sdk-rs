use crate::WriteXDR;
use crate::hash::*;
use crate::internal::encode_xdr;
use crate::sys;
use crate::sysresult::Result;

/// Maximum number of topics per event
pub const EVENT_MAX_TOPICS: u32 = groundhog_xdr::event::kMaxTopics;

/// Maximum size of serialized event data (including the topics)
pub const EVENT_MAX_DATA: u32 = groundhog_xdr::event::kMaxEventLength;

/// A topic is a fixed-length, opaque array
pub type Topic = [u8; 32];

/// An `impl Event` publishes each topic as a `TopicSrc` value
pub enum TopicSrc<'a> {
    /// The topic is this owned value
    Owned(Topic),
    /// The topic is available via a reference into the event
    /// (or a `'static` reference).
    Shared(&'a Topic),
    /// The topic is derived by hashing something in the event
    /// (or something behind a `'static` reference).
    Hashed(&'a dyn Hash),
}

// A `TopicSrc` can be converted to an owned `Topic`
impl<'a> From<TopicSrc<'a>> for Topic {
    fn from(src: TopicSrc<'a>) -> Topic {
        match src {
            TopicSrc::Hashed(hashable) => hashable.hash().into_inner(),
            TopicSrc::Owned(topic) => topic,
            TopicSrc::Shared(topic) => *topic,
        }
    }
}

pub trait Event: WriteXDR {
    /// Number of topics
    fn num_topics(&self) -> u32;

    /// Retrieve the topic at a given index
    fn topic<'a>(&'a self, index: u32) -> Option<TopicSrc<'a>> {
        let _ = index;

        None
    }
}

/// Emit an event
pub fn log_event(event: &impl Event) -> Result<()> {
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

    let data = encode_xdr(event)?;

    sys::uninit_LogEvent(
        topic0.as_ref(),
        topic1.as_ref(),
        topic2.as_ref(),
        data.as_ref(),
    )?;

    Ok(())
}

// This should go in a test-utils area
mod emitted_event {
    use crate::event::Topic;
    use crate::types::event::TxEmittedEvent;
    use core::cmp::min;

    // Size of each topic in serialized event data
    pub const TOPIC_SIZE: usize = 32;

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

            let topic_bytes =
                &event.serialized_event[topic_offset..topic_extent];

            let topic: &[u8; TOPIC_SIZE] = topic_bytes.try_into().unwrap();

            Some(topic)
        }

        pub fn num_topics(&self) -> u32 {
            self.0.num_topics
        }
    }

    impl<'a> From<&'a TxEmittedEvent> for EmittedEvent<'a> {
        fn from(event: &'a TxEmittedEvent) -> Self {
            Self(event)
        }
    }
}
pub use emitted_event::*;
