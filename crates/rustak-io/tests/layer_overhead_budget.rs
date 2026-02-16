use std::time::{Duration, Instant};

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

#[test]
fn layered_send_path_stays_within_reasonable_debug_budget() {
    const OPS: u32 = 2_000;

    let baseline = NoopSink;
    let baseline_elapsed = run_loop(&baseline, OPS);
    let baseline_per_op = nanos_per_op(baseline_elapsed, OPS);

    let rate = RateLimitLayer::new(
        NoopSink,
        RateLimitConfig {
            max_events: 20_000,
            per: Duration::from_secs(1),
        },
    )
    .expect("valid rate-limit config");
    let dedup = DedupLayer::new(
        rate,
        DedupConfig {
            max_keys: OPS as usize + 8,
        },
        |env: &MessageEnvelope<String>| env.message.clone(),
    )
    .expect("valid dedup config");
    let metrics = MetricsLayer::new(dedup);

    let layered_elapsed = run_loop(&metrics, OPS);
    let layered_per_op = nanos_per_op(layered_elapsed, OPS);

    let allowed = baseline_per_op
        .saturating_mul(200)
        .saturating_add(2_000_000);
    assert!(
        layered_per_op <= allowed,
        "layer overhead budget exceeded: baseline={}ns/op layered={}ns/op allowed={}ns/op",
        baseline_per_op,
        layered_per_op,
        allowed
    );
}

fn run_loop<S: MessageSink<String>>(sink: &S, ops: u32) -> Duration {
    let start = Instant::now();
    for index in 0..ops {
        let env = MessageEnvelope::new(format!("message-{index}"));
        block_on(sink.send_envelope(env)).expect("send should succeed");
    }
    start.elapsed()
}

fn nanos_per_op(duration: Duration, ops: u32) -> u128 {
    duration.as_nanos() / u128::from(ops)
}
