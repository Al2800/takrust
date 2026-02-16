use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rustak_transport::{
    OutboundSendQueue, QueuePriority, SendQueueClassifier, SendQueueConfig, SendQueueMode,
};

#[derive(Clone)]
struct BenchPacket {
    uid: String,
    payload_bytes: usize,
    priority: QueuePriority,
}

#[derive(Clone, Copy)]
struct BenchClassifier;

impl SendQueueClassifier<BenchPacket> for BenchClassifier {
    fn byte_size(&self, item: &BenchPacket) -> usize {
        item.payload_bytes
    }

    fn priority(&self, item: &BenchPacket) -> QueuePriority {
        item.priority
    }

    fn coalesce_key(&self, item: &BenchPacket) -> Option<String> {
        Some(item.uid.clone())
    }
}

fn make_queue() -> OutboundSendQueue<BenchPacket, BenchClassifier> {
    OutboundSendQueue::new(
        SendQueueConfig {
            max_messages: 8_192,
            max_bytes: 16 * 1024 * 1024,
            mode: SendQueueMode::Priority,
        },
        BenchClassifier,
    )
    .expect("queue config should be valid")
}

fn bench_transport_throughput(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("transport_throughput");
    group.bench_function("enqueue_then_drain_priority_queue", |bench| {
        bench.iter(|| {
            let mut queue = make_queue();

            for index in 0..1_024usize {
                let priority = if index % 16 == 0 {
                    QueuePriority::High
                } else if index % 2 == 0 {
                    QueuePriority::Normal
                } else {
                    QueuePriority::Low
                };
                let packet = BenchPacket {
                    uid: format!("track-{}", index % 128),
                    payload_bytes: 256 + (index % 64),
                    priority,
                };
                queue.enqueue(packet);
            }

            let mut drained = 0usize;
            while queue.dequeue().is_some() {
                drained = drained.saturating_add(1);
            }

            black_box(drained);
        });
    });
    group.finish();
}

criterion_group!(benches, bench_transport_throughput);
criterion_main!(benches);
