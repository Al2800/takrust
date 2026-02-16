use std::net::SocketAddr;
use std::pin::Pin;
use std::time::{Duration, Instant, SystemTime};

use bytes::Bytes;
use futures::future::BoxFuture;
use futures::Stream;
use thiserror::Error;

pub mod layers;

#[derive(Debug, Error)]
pub enum IoError {
    #[error("closed")]
    Closed,

    #[error("timeout after {0:?}")]
    Timeout(Duration),

    #[error("overloaded")]
    Overloaded,

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("other: {0}")]
    Other(String),
}

/// Wall and monotonic timestamps for replay fidelity and audit correlation.
#[derive(Debug, Clone)]
pub struct ObservedTime {
    pub wall: SystemTime,
    pub monotonic: Instant,
}

impl ObservedTime {
    #[must_use]
    pub fn now() -> Self {
        Self {
            wall: SystemTime::now(),
            monotonic: Instant::now(),
        }
    }

    #[must_use]
    pub fn new(wall: SystemTime, monotonic: Instant) -> Self {
        Self { wall, monotonic }
    }
}

impl Default for ObservedTime {
    fn default() -> Self {
        Self::now()
    }
}

/// Standard metadata wrapper for received messages.
#[derive(Debug, Clone)]
pub struct MessageEnvelope<T> {
    pub observed: ObservedTime,
    pub peer: Option<SocketAddr>,
    pub raw_frame: Option<Bytes>,
    pub message: T,
}

impl<T> MessageEnvelope<T> {
    #[must_use]
    pub fn new(message: T) -> Self {
        Self {
            observed: ObservedTime::now(),
            peer: None,
            raw_frame: None,
            message,
        }
    }

    #[must_use]
    pub fn with_observed(mut self, observed: ObservedTime) -> Self {
        self.observed = observed;
        self
    }

    #[must_use]
    pub fn with_peer(mut self, peer: SocketAddr) -> Self {
        self.peer = Some(peer);
        self
    }

    #[must_use]
    pub fn with_raw_frame(mut self, raw_frame: Bytes) -> Self {
        self.raw_frame = Some(raw_frame);
        self
    }

    #[must_use]
    pub fn map_message<U>(self, map: impl FnOnce(T) -> U) -> MessageEnvelope<U> {
        MessageEnvelope {
            observed: self.observed,
            peer: self.peer,
            raw_frame: self.raw_frame,
            message: map(self.message),
        }
    }
}

/// Generic trait for message sinks (transports, recorders, multiplexers).
pub trait MessageSink<T>: Send + Sync {
    fn send(&self, msg: T) -> BoxFuture<'_, Result<(), IoError>>;

    fn send_envelope(&self, env: MessageEnvelope<T>) -> BoxFuture<'_, Result<(), IoError>> {
        self.send(env.message)
    }
}

/// Generic trait for message sources (transports, replayers, generators).
pub trait MessageSource<T>: Send + Sync {
    fn recv(&mut self) -> BoxFuture<'_, Result<MessageEnvelope<T>, IoError>>;

    /// Object-safe stream adapter.
    fn into_stream(
        self: Box<Self>,
    ) -> Pin<Box<dyn Stream<Item = Result<MessageEnvelope<T>, IoError>> + Send>>;
}

/// CoT payload alias used for backwards-compatible naming on generic IO traits.
pub type CotMessage = Bytes;
pub type CotEnvelope = MessageEnvelope<CotMessage>;
pub type CotSink = dyn MessageSink<CotMessage>;
pub type CotSource = dyn MessageSource<CotMessage>;

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::Mutex;
    use std::time::{Duration, Instant, SystemTime};

    use bytes::Bytes;
    use futures::executor::block_on;
    use futures::stream;
    use futures::StreamExt;

    use crate::{
        CotSink, CotSource, IoError, MessageEnvelope, MessageSink, MessageSource, ObservedTime,
    };

    #[derive(Default)]
    struct VecSink {
        sent: Mutex<Vec<Bytes>>,
    }

    impl MessageSink<Bytes> for VecSink {
        fn send(&self, msg: Bytes) -> futures::future::BoxFuture<'_, Result<(), IoError>> {
            Box::pin(async move {
                let mut guard = self.sent.lock().expect("mutex should be available");
                guard.push(msg);
                Ok(())
            })
        }
    }

    struct VecSource {
        queue: VecDeque<MessageEnvelope<Bytes>>,
    }

    impl VecSource {
        fn new(items: Vec<MessageEnvelope<Bytes>>) -> Self {
            Self {
                queue: items.into(),
            }
        }
    }

    impl MessageSource<Bytes> for VecSource {
        fn recv(
            &mut self,
        ) -> futures::future::BoxFuture<'_, Result<MessageEnvelope<Bytes>, IoError>> {
            Box::pin(async move { self.queue.pop_front().ok_or(IoError::Closed) })
        }

        fn into_stream(
            self: Box<Self>,
        ) -> std::pin::Pin<
            Box<dyn futures::Stream<Item = Result<MessageEnvelope<Bytes>, IoError>> + Send>,
        > {
            Box::pin(stream::unfold(self, |mut source| async move {
                match source.recv().await {
                    Ok(envelope) => Some((Ok(envelope), source)),
                    Err(IoError::Closed) => None,
                    Err(error) => Some((Err(error), source)),
                }
            }))
        }
    }

    #[test]
    fn envelope_map_message_preserves_metadata() {
        let observed = ObservedTime::new(SystemTime::UNIX_EPOCH, Instant::now());
        let peer = "127.0.0.1:4242"
            .parse()
            .expect("socket address should parse");
        let envelope = MessageEnvelope::new(Bytes::from_static(b"frame"))
            .with_observed(observed.clone())
            .with_peer(peer)
            .with_raw_frame(Bytes::from_static(b"<event/>"));

        let mapped = envelope.map_message(|frame| frame.len());
        assert_eq!(mapped.message, 5);
        assert_eq!(mapped.peer, Some(peer));
        assert_eq!(mapped.raw_frame, Some(Bytes::from_static(b"<event/>")));
        assert_eq!(
            mapped
                .observed
                .wall
                .duration_since(SystemTime::UNIX_EPOCH)
                .expect("UNIX_EPOCH should be representable"),
            Duration::ZERO
        );
    }

    #[test]
    fn send_envelope_delegates_to_send() {
        let sink = VecSink::default();
        let envelope = MessageEnvelope::new(Bytes::from_static(b"cot-message"));

        block_on(sink.send_envelope(envelope)).expect("send_envelope should succeed");

        let sent = sink.sent.lock().expect("mutex should be available");
        assert_eq!(sent.as_slice(), &[Bytes::from_static(b"cot-message")]);
    }

    #[test]
    fn cot_aliases_are_object_safe_and_streamable() {
        let sink = VecSink::default();
        let sink_obj: &CotSink = &sink;
        block_on(sink_obj.send(Bytes::from_static(b"cot"))).expect("send should succeed");

        let envelope = MessageEnvelope::new(Bytes::from_static(b"one"));
        let source: Box<CotSource> = Box::new(VecSource::new(vec![envelope]));
        let mut stream = source.into_stream();
        let first = block_on(stream.next())
            .expect("stream should produce one item")
            .expect("item should be Ok");
        assert_eq!(first.message, Bytes::from_static(b"one"));
    }
}
