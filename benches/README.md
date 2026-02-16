# RusTAK benchmark scaffolding

This directory contains Criterion benchmark entrypoints for the architecture
targets tracked in `bd-2ue.4.5`.

- `serialisation.rs`
  - measures CoT payload encode/decode round-trips for XML passthrough and TAK
    Protocol v1 protobuf framing paths.
- `transport_throughput.rs`
  - measures bounded queue enqueue/dequeue behavior under a mixed-priority load
    profile.
- `sim_track_generation.rs`
  - measures deterministic track generation throughput across a parameter sweep
    matrix built from `rustak-sim` sweep/truth contracts.

Run all benchmark targets from repository root:

```bash
cargo bench --manifest-path crates/rustak/Cargo.toml
```
