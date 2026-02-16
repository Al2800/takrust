# RusTAK limits fuzz hooks

This crate provides non-interactive hook binaries intended for fuzz runners.
Each hook reads arbitrary bytes from `stdin`, maps them onto a config shape, and
executes validation to exercise fail-closed limits enforcement paths.

Available hooks:

- `wire_limits_hook`
- `transport_limits_hook`
- `sapient_limits_hook`

Example:

```bash
cat seed.bin | cargo run --manifest-path fuzz/Cargo.toml --bin wire_limits_hook
```
