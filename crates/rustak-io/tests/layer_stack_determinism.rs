use std::sync::Mutex;
use std::time::Duration;

use futures::executor::block_on;
use rustak_io::layers::{
    DedupConfig, DedupLayer, MetricsLayer, RateLimitConfig, RateLimitLayer, TapLayer,
};
use rustak_io::{IoError, MessageEnvelope, MessageSink};

#[derive(Default)]
struct CollectSink {
    sent: Mutex<Vec<String>>,
}

impl MessageSink<String> for CollectSink {
    fn send(&self, msg: String) -> futures::future::BoxFuture<'_, Result<(), IoError>> {
        Box::pin(async move {
            self.sent.lock().expect("collect mutex poisoned").push(msg);
            Ok(())
        })
    }

    fn send_envelope(
        &self,
        env: MessageEnvelope<String>,
    ) -> futures::future::BoxFuture<'_, Result<(), IoError>> {
        Box::pin(async move {
            self.sent
                .lock()
                .expect("collect mutex poisoned")
                .push(env.message);
            Ok(())
        })
    }
}

#[test]
fn stacked_layers_are_deterministic_for_repeat_sequences() {
    let sink = CollectSink::default();
    let tapped = Mutex::new(Vec::new());
    let tap = TapLayer::new(sink, |env: &MessageEnvelope<String>| {
        tapped
            .lock()
            .expect("tap mutex poisoned")
            .push(env.message.clone());
    });

    let rate = RateLimitLayer::new(
        tap,
        RateLimitConfig {
            max_events: 100,
            per: Duration::from_secs(1),
        },
    )
    .expect("rate config should be valid");
    let dedup = DedupLayer::new(
        rate,
        DedupConfig { max_keys: 64 },
        |env: &MessageEnvelope<String>| env.message.clone(),
    )
    .expect("dedup config should be valid");
    let metrics = MetricsLayer::new(dedup);

    for message in ["alpha", "alpha", "beta", "beta", "gamma"] {
        block_on(metrics.send_envelope(MessageEnvelope::new(message.to_string())))
            .expect("send should succeed");
    }

    let snapshot = metrics.snapshot();
    assert_eq!(snapshot.attempted, 5);
    assert_eq!(snapshot.sent, 5);
    assert_eq!(snapshot.errors, 0);

    let tap_observed = tapped.lock().expect("tap mutex poisoned").clone();
    assert_eq!(tap_observed, vec!["alpha", "beta", "gamma"]);
}
