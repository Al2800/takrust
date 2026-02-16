use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rustak_wire::{decode_payload_for_format, encode_payload_for_format, WireFormat};

fn bench_serialisation(criterion: &mut Criterion) {
    let payload = b"<event uid=\"bench-serialisation\" type=\"a-f-G-U-C\"/>".to_vec();
    let mut group = criterion.benchmark_group("serialisation");

    group.bench_function("xml_round_trip_passthrough", |bench| {
        bench.iter(|| {
            let encoded =
                encode_payload_for_format(black_box(&payload), WireFormat::Xml).expect("encode");
            let decoded =
                decode_payload_for_format(black_box(&encoded), WireFormat::Xml).expect("decode");
            black_box(decoded);
        });
    });

    group.bench_function("tak_v1_proto_round_trip", |bench| {
        bench.iter(|| {
            let encoded = encode_payload_for_format(black_box(&payload), WireFormat::TakProtocolV1)
                .expect("encode");
            let decoded = decode_payload_for_format(black_box(&encoded), WireFormat::TakProtocolV1)
                .expect("decode");
            black_box(decoded);
        });
    });

    group.finish();
}

criterion_group!(benches, bench_serialisation);
criterion_main!(benches);
