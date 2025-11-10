use crate::WriteXDR;
use crate::hash::*;
use crate::internal::encode_xdr;
use crate::sys;
use crate::sysresult::Result;

pub const EVENT_MAX_TOPICS: usize = 3;

pub type Topic = [u8; 32];

pub enum TopicSrc<'a> {
    Hashed(&'a dyn Hash),
    Owned(Topic),
    Shared(&'a Topic),
}

impl<'a> From<TopicSrc<'a>> for Topic {
    fn from(src: TopicSrc<'a>) -> Topic {
        match src {
            TopicSrc::Hashed(hashable) => hashable.hash().into_inner(),
            TopicSrc::Owned(topic) => topic,
            TopicSrc::Shared(topic) => topic.clone(),
        }
    }
}

pub trait Event: WriteXDR {
    fn topic<'a>(&'a self, index: u32) -> Option<TopicSrc<'a>> {
        let _ = index;

        None
    }
}

pub fn log_event(event: &impl Event) -> Result<()> {
    let topic0: Option<Topic> = event.topic(0).map(|t| t.into());
    let topic1: Option<Topic> = event.topic(1).map(|t| t.into());
    let topic2: Option<Topic> = event.topic(2).map(|t| t.into());

    let data = encode_xdr(event)?;

    // TODO: change syscall signature to take [u8; u32]
    sys::uninit_LogEvent(
        topic0.map(Into::into).as_ref(),
        topic1.map(Into::into).as_ref(),
        topic2.map(Into::into).as_ref(),
        data.as_ref(),
    )?;

    Ok(())
}

// This should go in a test-utils area
mod emitted_event {
    use crate::event::Topic;
    use crate::types::event::TxEmittedEvent;

    pub const TOPIC_SIZE: usize = 32;

    pub fn event_payload(event: &TxEmittedEvent) -> Option<&[u8]> {
        let topics_len = TOPIC_SIZE * event.num_topics as usize;

        if event.serialized_event.len() < topics_len {
            return None;
        }

        let payload = &event.serialized_event[topics_len..];

        Some(payload)
    }

    pub fn event_topic(event: &TxEmittedEvent, index: u32) -> Option<&Topic> {
        if index > event.num_topics {
            return None;
        }
        let topic_offset = TOPIC_SIZE * index as usize;
        let topic_bytes =
            &event.serialized_event[topic_offset..topic_offset + TOPIC_SIZE];
        let topic: &[u8; TOPIC_SIZE] = topic_bytes.try_into().unwrap();

        Some(topic)
    }
}
pub use emitted_event::*;
