use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use futures::executor::block_on;
use rustak_io::layers::{DedupConfig, DedupLayer, MetricsLayer, RateLimitConfig, RateLimitLayer};
use rustak_io::{IoError, MessageEnvelope, MessageSink};

struct NoopSink;

impl MessageSink<String> for NoopSink {
    fn send(&self, _msg: String) -> futures::future::BoxFuture<'_, Result<(), IoError>> {
        Box::pin(async { Ok(()) })
    }

    fn send_envelope(
        &self,
        _env: MessageEnvelope<String>,
    ) -> futures::future::BoxFuture<'_, Result<(), IoError>> {
        Box::pin(async { Ok(()) })
    }
}

fn bench_layer_overhead(criterion: &mut Criterion) {
    let baseline = NoopSink;
    let rate = RateLimitLayer::new(
        NoopSink,
        RateLimitConfig {
            max_events: 100_000,
            per: Duration::from_secs(1),
        },
    )
    .expect("valid rate-limit config");
    let dedup = DedupLayer::new(
        rate,
        DedupConfig { max_keys: 200_000 },
        |env: &MessageEnvelope<String>| env.message.clone(),
    )
    .expect("valid dedup config");
    let layered = MetricsLayer::new(dedup);

    let mut group = criterion.benchmark_group("layer_overhead");
    let baseline_counter = AtomicU64::new(0);
    group.bench_function("baseline_send_envelope", |bench| {
        bench.iter(|| {
            let value = baseline_counter.fetch_add(1, Ordering::Relaxed);
            let env = MessageEnvelope::new(format!("msg-{value}"));
            block_on(baseline.send_envelope(black_box(env))).expect("baseline send should succeed");
        });
    });

    let layered_counter = AtomicU64::new(0);
    group.bench_function("layered_send_envelope", |bench| {
        bench.iter(|| {
            let value = layered_counter.fetch_add(1, Ordering::Relaxed);
            let env = MessageEnvelope::new(format!("msg-{value}"));
            block_on(layered.send_envelope(black_box(env))).expect("layered send should succeed");
        });
    });
    group.finish();
}

criterion_group!(benches, bench_layer_overhead);
criterion_main!(benches);
