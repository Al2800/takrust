# RusTAK Integration Test Contract

This directory defines integration-test execution contracts that require
external services (for example TAK Server or dockerized interoperability
harnesses).

## Execution Paths

- **Local deterministic smoke path**
  - `cargo test --manifest-path tests/interop_harness/Cargo.toml`
  - `cargo test --manifest-path crates/rustak-server/Cargo.toml --test connection_contract`
  - Uses repository fixtures and must run without external credentials.
- **CI baseline path**
  - `cargo test --manifest-path tests/release_profiles/Cargo.toml`
  - `cargo test --manifest-path tests/interop_harness/Cargo.toml`
  - `cargo test --manifest-path crates/rustak-server/Cargo.toml --test connection_contract`
- **Extended environment-dependent path**
  - Reserved for TAK Server/docker orchestration flows that need networked
    dependencies and explicit environment setup.
  - TAK Server docker smoke harness:
    - `export RUSTAK_TAK_SERVER_IMAGE=<tak-server-image>`
    - `tests/integration/run_tak_server_smoke.sh`
  - Smoke harness executes:
    - `cargo test --manifest-path tests/interop_harness/Cargo.toml --test tak_server_docker_smoke -- --nocapture`

## Authoring Rules

- Keep fixtures under `tests/fixtures/**` and reference by relative path.
- Add new integration suites under this directory with a short README that
  states prerequisites, command, and expected pass signal.
- Mark environment-dependent tests as opt-in so baseline CI remains stable.
- Keep docker harness entrypoints deterministic (fixed host/port/path defaults).
