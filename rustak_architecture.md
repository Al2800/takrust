# RusTAK: A Complete Rust Workspace for TAK

## Project Brief

**RusTAK** is an open-source Rust library for the Tactical Awareness Kit (TAK) ecosystem and a first-class bridge between TAK and SAPIENT-based C-UAS sensor networks. TAK communicates via Cursor on Target (CoT), an XML and TAK Protocol v1 (Protobuf payload) family used for real-time geospatial event sharing. Clients include ATAK (Android), WinTAK (Windows), iTAK (iOS), and TAK Server.

**The problem:** Existing TAK libraries are fragmented, incomplete, or built in memory-unsafe languages. In parallel, SAPIENT adoption in C-UAS creates a practical integration demand: sensor networks speak SAPIENT while COP systems speak TAK, and many deployments rely on ad hoc glue that is hard to audit, hard to reproduce, and brittle under real tactical network conditions.

**What this project delivers:** A modular Cargo workspace of crates covering:
1) the TAK integration surface (CoT XML, TAK Protocol v1 payloads, mesh and streaming framing/negotiation, UDP/TCP/TLS transport, mTLS certificate tooling, TAK Server client);
2) SAPIENT integration (versioned Protobuf schemas, TCP framing, session helpers, conformance tooling);
3) a deterministic, configurable TAK <-> SAPIENT mapping/bridge (tracks, detections, alerts, and tasking);
4) simulation and record/replay with ground-truth and audit-focused analysis.

**Who is building it:** Alastair runs a technical consultancy specialising in Counter-Unmanned Aircraft Systems (C-UAS) and defence technology evaluation. He coordinates testing programmes for government clients, manages vendor relationships, and provides technical due diligence. This library is being built to serve both as the foundation of his consultancy's evaluation tooling and as an open-source contribution to the TAK ecosystem.

**Commercial model:** Open-core. Protocol model, parsing, framing, transport, crypto, and developer tooling are MIT/Apache-2.0 dual-licensed open source. Advanced scenario packs and C-UAS evaluation/reporting tooling live in a separate private workspace overlay (so the open-source workspace remains buildable by external contributors and CI).

**Current status:** Architecture and planning phase. This document is the comprehensive technical specification and project plan.

---

## Architecture & Project Structure

---

## 1. Vision & Design Principles

**Goal:** The first production-grade, fully memory-safe Rust workspace for the TAK ecosystem, with a first-class TAK <-> SAPIENT bridge suitable for audit and repeatable evaluation. Designed for defence and security applications where reliability, performance, and auditability are non-negotiable.

### Core Principles

- **Memory safety without compromise** — zero `unsafe` blocks in the public API surface; all `unsafe` (if any) isolated, documented, and audited in internal modules
- **Layered architecture** — protocol model, wire framing, transport, and higher-level clients are separate layers with one-way dependencies
- **Async-first, sync-friendly** — async implementations (`tokio`) live above the model layer; blocking wrappers are optional adapters
- **Protocol completeness** — full CoT XML, TAK Protocol v1, and SAPIENT framing/codec support (versioned), not a subset
- **Interop-first** — conformance and fixtures against upstream harnesses are part of the definition of “done”
- **Defence-grade transport** — mTLS, certificate rotation, FIPS-capable crypto options
- **Modular workspace** — use only what you need; the core model crate has minimal dependencies
- **Scenario simulation built-in** — synthetic track generation as a first-class capability, not an afterthought
- **Extensively tested** — property-based testing, fuzzing targets, integration tests against real TAK Server, and conformance fixtures/harnesses for SAPIENT
- **Well-documented** — every public type has doc comments, examples, and cross-references to the CoT specification

### Scope, Compatibility, and Conformance Targets

The repository defines explicit non-goals, a compatibility matrix, and conformance criteria so “complete” has an enforceable meaning.

#### Explicit non-goals for initial releases
- Building ATAK/WinTAK/iTAK plugins or UI components
- TAK Server deployment/scaling/hardening guidance beyond what is required for integration tests
- Classified-only message sets or proprietary extensions in the open repository

#### Compatibility matrix (tracked in CI)
| Surface | Minimum support | Stretch support | Conformance strategy |
|---|---|---|---|
| CoT XML | Core event model used by TAK clients/servers | Incrementally add detail extensions as fixtures grow | Golden fixtures + fuzzing + semantic round-trip checks |
| TAK Protocol | Version 1 framing for mesh and streaming, including negotiation | Additional versions behind feature flags | State-machine tests + captured traces + fuzzing |
| SAPIENT | BSI Flex 335 v2.0 wrapper and framing | Additional published proto versions | Upstream proto round-trips + framing fuzzing |

#### Definition of done for a supported protocol surface
- At least one known-good interop trace captured from a real client/server pair and replayed in CI where licensing permits
- Bounded-memory parsing under adversarial inputs (fuzzed), with explicit size limits and timeouts
- Downgrade and failure modes documented (fail-open vs fail-closed choices are explicit per feature)

### Foundational Hard-Correctness Surfaces (Release Gates)

The plan is intentionally sequenced around four non-negotiable correctness surfaces. No higher-level feature is considered production-ready until these pass.

1) **Wire framing correctness** (`rustak-wire`, `rustak-sapient`)
   - TAK stream vs mesh framing implemented per spec
   - SAPIENT 4-byte little-endian length framing enforced with strict bounds
2) **Negotiation correctness** (`rustak-wire`, `rustak-commo`)
   - Streaming XML->TAK upgrade state machine with explicit timeout behavior
   - Mesh TakControl version convergence with deterministic per-contact policy
3) **Bounded parsing and memory behavior** (`rustak-cot`, `rustak-wire`, `rustak-sapient`)
   - Maximum frame sizes, bounded scans, varint bounds, and parser limits
   - Fuzzing and adversarial fixtures demonstrate no unbounded buffering paths
4) **Time semantics and replay fidelity** (`rustak-io`, `rustak-record`)
   - Unified `ObservedTime` + `CotEnvelope` model across receive paths
   - Replay uses monotonic offsets for deterministic timing; wall time retained for audit

---

## 2. Open-Source Strategy & Licensing

### 2.1 Open-Core Model

The library follows an **open-core** approach — the foundation is fully open source, while advanced defence/evaluation tooling is proprietary.

**Open Source (MIT / Apache-2.0 dual licence):**
- `rustak` — facade crate (re-exports, ergonomic builders, unified app-facing error type)
- `rustak-core` — foundation data model (no runtime, no transport)
- `rustak-io` — runtime-agnostic async traits/adapters shared across sim/record/transport
- `rustak-limits` — shared, validated resource limits and parsing budgets (frame sizes, queue bounds, timeouts)
- `rustak-cot` — CoT XML engine
- `rustak-wire` — TAK wire framing + protocol negotiation (legacy XML stream + TAK Protocol v1)
- `rustak-proto` — TAK Protocol v1 payload (Protobuf) codec
- `rustak-net` — shared async networking primitives (retry/backoff/timeouts + framed IO glue)
- `rustak-transport` — UDP, TCP, TLS transport
- `rustak-commo` — TAK comms core: mesh presence, contact tracking, mesh version selection, send policies
- `rustak-crypto` — certificate management and TLS configuration helpers
- `rustak-sim` — deterministic simulation primitives and scenario runner (open core)
- `rustak-record` — record/replay container and timing fidelity (open core)
- `rustak-config` — config loading, validation, redaction, and schema generation
- `rustak-admin` — optional admin endpoints (health, metrics, reload) for long-running services
- `rustak-cli` — command-line diagnostics and utilities
- `rustak-server` — TAK Server client API
- `rustak-sapient` — SAPIENT Protobuf schemas + codec + TCP framing (versioned modules)
- `rustak-bridge` — configurable TAK <-> SAPIENT semantic mapping and bridge runner
- `rustak-ffi` — stable C ABI and language bindings support (JNI/.NET examples)
- `rustak-geo` — pure-Rust geodesy utilities and coordinate helpers (optional)

Dual licence is deliberate: defence contractors' legal teams often prefer Apache-2.0, while the broader Rust community expects MIT.

Third-party inputs: upstream SAPIENT `.proto` files are Apache-2.0 licensed; they are pinned (vendored or mirrored) with an explicit license notice and used to generate the `rustak-sapient` schema modules.

**Commercial / Proprietary:**
- Proprietary crates are maintained in a separate private repository/workspace overlay
  to keep the open-source workspace buildable by external contributors and CI.
- `rustak-sim-pro` — advanced scenario packs and threat libraries (private)
- `rustak-record-pro` — C-UAS evaluation metrics, reporting, and data products (private)
- `rustak-bridge-pro` — advanced correlation, taxonomy packs, and client-specific mapping rulesets (private)
- Pre-built scenario packs (swarm, GPS spoofing, mixed-threat environments)
- Standardised evaluation report templates

This mirrors proven models: PostGIS (open core, commercial extensions), Grafana (open source, paid enterprise), and how defence companies build on open standards while monetising the analytical layer.

### 2.2 Target Users & Ecosystem

| User Category | What They Build | Why They Use This Library |
|---|---|---|
| **C-UAS vendors** | Detection system → TAK integrations | Type-safe CoT, mTLS out of the box, saves months of integration work |
| **Defence primes / SIs** | Sensor-to-TAK bridges for radar, RF, optical systems | Protocol completeness, auditability, memory safety for classified environments |
| **Drone / UAS operators** | Fleet telemetry, mission planning, GCS tools | Reliable transport, protobuf performance, async-first architecture |
| **Government test ranges** | Evaluation infrastructure, synthetic environments | Simulation engine, record/replay, truth-data analysis |
| **Emergency services** | Public safety TAK integrations (fire, police, SAR) | Lower barrier to entry, good documentation, CLI tooling |
| **Hobbyists / researchers** | CivTAK/ATAK plugins, academic projects | Open source, well-documented, Rust learning resource |

Community contributors provide: bug fixes, protocol edge cases, real-world interop testing, and broader ecosystem adoption that drives commercial demand.

### 2.3 Commercial Revenue Model

| Revenue Stream | Description |
|---|---|
| **Testing-as-a-Service** | Run standardised synthetic evaluation scenarios against client C-UAS systems; deliver performance reports |
| **Licensed tooling** | Defence primes and test ranges licence the simulation and analysis crates for internal use |
| **Integration consulting** | Vendors building on the open-source library hire you to integrate with TAK or prepare for government evaluations |
| **Training & demonstration** | Run simulated threat scenarios on ATAK tablets for clients evaluating C-UAS investment — no drones, no airspace permissions |
| **Scenario pack subscriptions** | Regularly updated threat profile libraries reflecting emerging drone platforms and tactics |

The open-source base makes all commercial offerings credible — you're not just selling consulting, you're the author of the tool the community uses.

---

## 3. Simulation Use Cases

The simulation engine (`rustak-sim`) serves four distinct operational purposes:

### 3.1 System Stress Testing (Pre-Event)

Before a live evaluation event (e.g., CORE), verify whether a vendor's system handles realistic load without flying real drones.

**How it works:** Define a scenario (e.g., 50 simultaneous multi-rotor tracks, staggered spawn, varied altitudes and speeds). The scenario runner generates unique synthetic drone tracks with realistic flight dynamics and GPS noise, pushing them into TAK Server as CoT events. The vendor's system, consuming from TAK Server or multicast, sees 50 simultaneous targets.

**What you measure:** Display responsiveness, track maintenance across all targets, alert system accuracy, performance degradation thresholds.

**Key advantage: Repeatability.** The exact same scenario runs against Vendor A, B, and C for fair comparison. Real drone tests vary due to environmental conditions, pilot behaviour, and battery states.

### 3.2 Sensor Fusion Validation (During Event)

During live testing with real drones and sensors, inject known-truth synthetic tracks alongside real detections.

**How it works:** Synthetic tracks with calibrated noise models run concurrently with real sensor feeds. Truth data is logged for every synthetic entity.

**What you measure:**
- Whether the system distinguishes real vs synthetic tracks (it shouldn't — validates noise model realism)
- Performance against threat types you can't physically create (fixed-wing at 200 knots, 20-drone swarm, 500m altitude target)
- Continuous accuracy baseline even during real-drone battery swap downtime

### 3.3 Scenario Replay & Regression Testing (Post-Event)

After a live evaluation, replay recorded CoT traffic against updated vendor software without organising another live test.

**How it works:** Recorded session data replays through `rustak-record` at original timing. Vendor's updated system sees identical track data. Compare new results against original baseline.

**What you measure:** Detection rate changes, new false alarm introduction, latency differences, classification accuracy shifts.

**Key advantage:** This is regression testing for C-UAS systems — a structured capability nobody is offering today.

### 3.4 Training & Client Demonstration

For clients evaluating C-UAS investment, run realistic threat simulations on TAK Server connected to ATAK tablets.

**How it works:** Simulation generates proper CoT events that display identically to real data on ATAK. Clients see what a drone incursion looks like on the common operating picture.

**Key advantage:** No drones needed, no airspace permissions, no risk. Demonstrations can be run in a boardroom, on a client site, or at a conference.

---

## 4. Workspace Structure

Note: proprietary crates live in a separate private workspace overlay; the structure below is the open-source workspace.

```
rustak/
├── Cargo.toml                    # Workspace root
├── rust-toolchain.toml           # Pinned toolchain for reproducible builds
├── LICENSE-MIT
├── LICENSE-APACHE
├── README.md
├── ARCHITECTURE.md               # This document
├── CHANGELOG.md
├── CONTRIBUTING.md
├── SECURITY.md
├── CODE_OF_CONDUCT.md
├── deny.toml                     # cargo-deny config (licence/advisory auditing)
├── clippy.toml                   # Strict linting configuration
├── rustfmt.toml                  # Consistent formatting
│
├── crates/
│   ├── rustak/                   # Facade crate (re-exports + app-facing API)
│   ├── rustak-core/              # Core data model (no runtime, no transport)
│   ├── rustak-io/                # Runtime-agnostic async traits/adapters (Sink/Source, streams)
│   ├── rustak-limits/            # Shared validated limits (DoS hardening contract)
│   ├── rustak-cot/               # CoT XML serialisation/deserialisation
│   ├── rustak-proto/             # TAK Protocol v1 payload (Protobuf) codec
│   ├── rustak-wire/              # TAK wire framing + protocol negotiation
│   ├── rustak-net/               # Shared tokio networking primitives (retry/timeouts/framed IO)
│   ├── rustak-transport/         # Network transport (UDP, TCP, TLS, WebSocket)
│   ├── rustak-commo/             # TAK comms core: mesh presence + version selection policies
│   ├── rustak-server/            # TAK Server client (auth, data packages, channels)
│   ├── rustak-crypto/            # Certificate management + rustls provider selection
│   ├── rustak-sapient/           # SAPIENT Protobuf codec + TCP framing + session helpers
│   ├── rustak-bridge/            # TAK <-> SAPIENT mapping + bridge runner
│   ├── rustak-sim/               # Scenario simulation & synthetic track generation
│   ├── rustak-record/            # Record/replay engine & truth data logging
│   ├── rustak-config/            # Config types + schema + validation + redaction
│   ├── rustak-admin/             # Optional admin server (health/metrics/reload)
│   ├── rustak-ffi/               # Stable C ABI + bindings scaffolding
│   ├── rustak-geo/               # Pure-Rust geodesy helpers (optional dependency surface)
│   └── rustak-cli/               # CLI tool for testing, simulation, diagnostics
├── xtask/                        # Workspace automation (lint, test, release, fixtures)
│
├── examples/
│   ├── basic_sender.rs           # Send a single CoT position
│   ├── drone_tracker.rs          # Track a moving drone via TAK Server
│   ├── swarm_sim.rs              # Simulate a drone swarm
│   ├── cuas_stress_test.rs       # Stress test a C-UAS system
│   ├── tak_server_connect.rs     # Connect to TAK Server with mTLS
│   ├── replay_scenario.rs        # Replay a recorded scenario
│   ├── sapient_listener.rs       # Listen for SAPIENT messages over TCP length-prefixed framing
│   ├── tak_sapient_bridge.rs     # Run a TAK <-> SAPIENT bridge process
│   └── ffi_roundtrip.rs          # Demonstrate C ABI: parse/encode, frame/unframe, map
│
├── tests/
│   ├── integration/              # Integration tests (require TAK Server)
│   ├── protocol_conformance/     # Round-trip CoT XML/TAK v1 conformance tests
│   ├── sapient_conformance/      # Encode/decode/framing tests vs upstream SAPIENT proto versions
│   ├── interop_harness/          # Optional integration: Dstl harness + TAK Server docker compose
│   └── fixtures/                 # Sample CoT messages, certificates, scenarios
│
├── fuzz/                         # cargo-fuzz targets for protocol parsing
│   ├── fuzz_cot_xml.rs
│   ├── fuzz_wire_frames.rs
│   ├── fuzz_proto_v1.rs
│   └── fuzz_sapient_frames.rs
│
├── benches/                      # Criterion benchmarks
│   ├── serialisation.rs
│   ├── transport_throughput.rs
│   └── sim_track_generation.rs
│
├── third_party/
│   └── sapient/                  # Pinned upstream SAPIENT .proto sources + license notices
│
└── docs/
    ├── cot_type_reference.md     # Complete MIL-STD-2525 CoT type mapping
    ├── tak_server_api.md         # TAK Server API documentation
    ├── deployment_guide.md       # How to deploy in classified environments
    ├── security_audit.md         # Security considerations & audit log
    ├── sapient_reference.md      # SAPIENT framing + version support notes
    ├── tak_sapient_mapping.md    # Mapping rules, taxonomy translation, and correlation strategy
    └── conformance.md            # What we test, against which harnesses/fixtures
```

---

## 5. Crate Architecture — Dependency Graph

One-way layering (no dependency cycles):

- `rustak-core` is the protocol data model (no async runtime, no transport, no TLS).
- `rustak-limits` defines validated global limits/budgets used at every boundary (frames, scans, queues, parsing caps).
- `rustak-io` defines runtime-agnostic async traits/adapters used by sim/record/transport.
- `rustak-cot` and `rustak-proto` encode/decode between wire payloads and `rustak-core` types.
- `rustak-wire` implements framing + negotiation; depends on `rustak-{cot,proto,core}`.
- `rustak-net` provides shared connection management (TCP/TLS/WebSocket/UDP), reconnection/backoff, socket options, and generic framed IO primitives; it has no protocol knowledge.
- `rustak-transport` is the TAK-aware transport: it uses `rustak-net` + `rustak-wire` to send/receive `CotEvent` with bounded queues and capture hooks.
- `rustak-commo` builds on `rustak-transport` and `rustak-wire` to implement mesh semantics (TakControl, SA cadence, per-contact version windows).
- `rustak-server` builds on `rustak-transport` and adds server capability discovery and API calls.
- `rustak-sapient` implements SAPIENT schemas, codec, and session helpers; it uses `rustak-net` for TCP framing and reconnect behavior.
- `rustak-bridge` provides configurable, stateful TAK <-> SAPIENT mapping and a bridge runner (depends on `rustak-{core,io,sapient}` plus transport adapters).
- `rustak-sim` and `rustak-record` depend on `rustak-{io,core}` and can run without any networking.
- `rustak-config` centralizes config loading/validation/schema/redaction for both CLI and long-running services.
- `rustak-admin` is an optional HTTP admin surface (health/metrics/reload), feature-gated for service deployments.
- `rustak-geo` provides pure-Rust geodesy helpers used by sim and bridge (optional).
- `rustak` is a facade crate that re-exports common types and provides an application-facing unified error type.
- `rustak-ffi` exposes a stable C ABI for core codec/bridge functions (parse, encode, frame, map).
- `rustak-cli` depends on `rustak` + `rustak-config` and can optionally enable `rustak-admin` in service-like modes.

---

## 6. Crate-by-Crate Deep Dive

---

### 6.1 `rustak` — Facade Crate

**Purpose:** Stable, ergonomic public API. Re-exports the common types from the workspace and provides an application-facing unified error type, while keeping lower layers free of transport/runtime dependencies.

Key responsibilities:
- Re-export the “default” surface area (`prelude`) for application code
- Provide ergonomic builders/constructors that can evolve without breaking the internal crate layout
- Define `RustakError` (or `Error`) that wraps per-crate error types

API sketch:
```rust
pub mod prelude {
    pub use rustak_core::{CotEvent, CotType, Position, Uid};
    pub use rustak_io::{CotSink, CotSource, IoError};
    pub use rustak_limits::Limits;
    pub use rustak_wire::{WireFormat, TakProtocolVersion};
}

/// Unified application-facing error (facade-level).
#[derive(Debug, thiserror::Error)]
pub enum RustakError {
    #[error(transparent)]
    Core(#[from] rustak_core::CoreError),
    #[error(transparent)]
    Cot(#[from] rustak_cot::CotError),
    #[error(transparent)]
    Wire(#[from] rustak_wire::WireError),
    #[error(transparent)]
    Proto(#[from] rustak_proto::ProtoError),
    #[error(transparent)]
    Crypto(#[from] rustak_crypto::CryptoError),
    #[error(transparent)]
    Transport(#[from] rustak_transport::TransportError),
    #[error(transparent)]
    Server(#[from] rustak_server::ServerError),
    #[error(transparent)]
    Commo(#[from] rustak_commo::CommoError),
    #[error(transparent)]
    Sapient(#[from] rustak_sapient::SapientError),
    #[error(transparent)]
    Config(#[from] rustak_config::ConfigError),
    #[error(transparent)]
    Admin(#[from] rustak_admin::AdminError),
    #[error(transparent)]
    Bridge(#[from] rustak_bridge::BridgeError),
    #[error(transparent)]
    Sim(#[from] rustak_sim::SimError),
    #[error(transparent)]
    Record(#[from] rustak_record::RecordError),
}
```

---

### 6.2 `rustak-core` — Foundation Data Model

**Purpose:** Shared protocol data model used across all crates. Zero network and runtime dependencies. Minimal allocations.

```rust
// ── Core position types ──────────────────────────────────────────────

/// WGS84 position with altitude and accuracy estimates.
/// Invariants are enforced at construction (lat/lon ranges, NaN checks).
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Position {
    pub latitude: f64,           // Decimal degrees, WGS84
    pub longitude: f64,          // Decimal degrees, WGS84
    pub hae: Option<f64>,        // Height above ellipsoid (metres)
    pub ce: Option<f64>,         // Circular error (metres, 95% confidence)
    pub le: Option<f64>,         // Linear error (metres, 95% confidence)
}

impl Position {
    pub fn new(latitude: f64, longitude: f64) -> Result<Self, CoreError>;
    pub fn with_hae(self, hae_m: f64) -> Result<Self, CoreError>;
}

/// Course and speed information for moving entities.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Kinematics {
    pub speed: Option<f64>,      // Metres per second
    pub course: Option<f64>,     // True heading, degrees (0-360)
    pub vertical_rate: Option<f64>, // Metres per second (positive = climbing)
}

// ── CoT type system ──────────────────────────────────────────────────

/// Strongly-typed CoT type string with validation.
/// Encodes the MIL-STD-2525 hierarchy: affiliation-battle_dimension-function.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CotType {
    inner: Arc<str>,  // e.g., "a-h-A-M-F-Q" (hostile, air, military, fixed-wing, UAV)
}

/// Affiliation dimension per MIL-STD-2525.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Affiliation {
    Pending,    // p
    Unknown,    // u
    Friend,     // f
    Neutral,    // n
    Hostile,    // h
    AssumedFriend, // a
    Suspect,    // s
    Joker,      // j
    Faker,      // k
}

/// Battle dimension for atoms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BattleDimension {
    Space,          // P
    Air,            // A
    Ground,         // G
    Surface,        // S
    Subsurface,     // U
    Other,          // X
}

/// Builder for constructing CoT types with validation.
/// Example: CotType::atom(Affiliation::Hostile, BattleDimension::Air)
///            .function("M-F-Q")  // Military, fixed-wing, UAV
///            .build()
impl CotType {
    pub fn atom(affil: Affiliation, dimension: BattleDimension) -> CotTypeBuilder;
    pub fn bits() -> CotTypeBuilder;    // b-* types
    pub fn parse(raw: &str) -> Result<Self, CotTypeError>;
    pub fn as_str(&self) -> &str;
    pub fn affiliation(&self) -> Option<Affiliation>;
    pub fn battle_dimension(&self) -> Option<BattleDimension>;
    pub fn is_atom(&self) -> bool;
}

/// Compile-time helper macro for static CoT types in code.
/// Fails compilation for invalid strings.
/// Example: `let t = cot_type!(\"a-h-A-M-F-Q\");`

// ── Event model ──────────────────────────────────────────────────────

/// The core CoT event, protocol-agnostic.
/// This is the central data structure that all serialisation formats
/// convert to and from.
#[derive(Debug, Clone, PartialEq)]
pub struct CotEvent {
    pub version: CotVersion,      // CoT version (avoid per-event allocation for the common case)
    pub uid: Uid,                 // Validated UID (non-empty, length-bounded)
    pub cot_type: CotType,        // MIL-STD-2525 type string
    pub how: How,                 // How the position was derived
    pub time: TimestampUtc,       // When the event was generated
    pub start: TimestampUtc,      // When the event becomes valid
    pub stale: TimestampUtc,      // When the event expires
    pub point: Position,          // Geographic position
    pub detail: CotDetail,        // Extensible detail payload
}

/// Core timestamp type. Keep UTC-only semantics in core and provide chrono
/// interop behind an optional feature for edge integrations.
pub type TimestampUtc = time::OffsetDateTime;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CotVersion {
    V2_0,
    Other(Arc<str>),
}

/// Validated CoT UID.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Uid(Arc<str>);

impl Uid {
    /// Generate a UUID-based UID (common default).
    pub fn new() -> Self;

    /// Parse and validate a UID string (non-empty, length-bounded).
    pub fn parse(s: &str) -> Result<Self, CoreError>;
    pub fn as_str(&self) -> &str;
}

/// How the position was derived (CoT 'how' field).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum How {
    MachineGps,             // m-g
    MachineGpsDifferential, // m-g-d
    MachineGpsEstimated,    // m-g-e
    MachineSimulated,       // m-s
    MachineRadar,           // m-r
    MachineInertial,        // m-i
    HumanInput,             // h-e
    HumanGps,               // h-g-i-g-o
    Custom(String),         // Arbitrary how string
}

/// Extensible detail section.
/// Contract: preserves semantic structure of unknown elements and
/// deterministic ordering of detail children.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CotDetail {
    pub elements: Vec<DetailElement>,     // Ordered representation of <detail> children
}

#[derive(Debug, Clone, PartialEq)]
pub enum DetailElement {
    Contact(Contact),
    Group(Group),
    Track(Track),
    Status(Status),
    TakVersion(TakVersion),
    Sensor(Sensor),
    Link(Link),
    Remarks(Remarks),
    Shape(Shape),
    Geofence(Geofence),
    Drone(DroneDetail),
    Provenance(Provenance),
    Unknown(XmlElement),                  // Includes unknown attributes and namespaces
    Extension(ExtensionBlob),             // Opaque payload for registered codecs
}

pub struct ExtensionBlob { pub key: String, pub bytes: Vec<u8> }

/// Typed extension registry used by XML codecs and bridge logic.
pub trait ExtensionRegistry: Send + Sync {
    fn decode(&self, key: &str, bytes: &[u8]) -> Option<DetailElement>;
    fn encode(&self, element: &DetailElement) -> Option<(String, Vec<u8>)>;
}

#[derive(Debug, Clone, PartialEq)]
pub struct Provenance {
    pub source_id: Option<Id>,          // UUID/ULID/string
    pub source_kind: Option<Arc<str>>,  // "SAPIENT", "RADAR", "RF", etc
    pub confidence: Option<f64>,        // 0.0 - 1.0
    pub classifications: Vec<ClassProb>,
    pub behaviours: Vec<BehaviourProb>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClassProb { pub label: Arc<str>, pub p: f64 }

#[derive(Debug, Clone, PartialEq)]
pub struct BehaviourProb { pub label: Arc<str>, pub p: f64 }

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Id {
    Uuid(uuid::Uuid),
    Ulid(ulid::Ulid),
    String(Arc<str>),
}

/// Contact information for the entity.
#[derive(Debug, Clone, PartialEq)]
pub struct Contact {
    pub callsign: Option<String>,
    pub endpoint: Option<String>,     // IP:port for direct messaging
    pub phone: Option<String>,
    pub email: Option<String>,
}

/// Track/kinematics detail.
#[derive(Debug, Clone, PartialEq)]
pub struct Track {
    pub kin: Kinematics,
}

/// UAS/Drone-specific detail extension.
/// This is the key differentiator for C-UAS applications.
#[derive(Debug, Clone, PartialEq)]
pub struct DroneDetail {
    pub platform_type: Option<DronePlatform>,
    pub propulsion: Option<Propulsion>,
    pub rcs: Option<f64>,             // Radar cross-section (dBsm)
    pub max_speed: Option<f64>,       // m/s
    pub endurance: Option<Duration>,
    pub payload: Option<String>,
    pub threat_level: Option<ThreatLevel>,
    pub detection_source: Option<String>,  // Which sensor detected it
    pub classification_confidence: Option<f64>, // 0.0 - 1.0
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DronePlatform {
    MultiRotor,
    FixedWing,
    Hybrid,         // VTOL fixed-wing
    SingleRotor,    // Helicopter-style
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreatLevel {
    None,       // Confirmed friendly/authorised
    Low,        // Detected, likely benign
    Medium,     // Unidentified, possible threat
    High,       // Confirmed hostile behaviour
    Critical,   // Active threat, immediate response required
}

// ── Error types ──────────────────────────────────────────────────────

/// Core-layer error type (model validation, parsing of core primitives).
/// Higher layers define their own error types; the `rustak` facade provides an
/// application-facing unified error enum that wraps all crate errors.
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("Invalid CoT type string: {0}")]
    InvalidCotType(String),
    #[error("Invalid UID: {0}")]
    InvalidUid(String),
    #[error("Invalid value: {0}")]
    InvalidValue(String),
}

// Async traits (Sink/Source/stream adapters) live in `rustak-io`.
```

**Key dependencies:** `time`, `uuid`, `ulid`, `thiserror` (plus optional `serde`, optional `chrono` interop feature)

---

### 6.2a `rustak-limits` — Shared Resource Budgets

**Purpose:** Single, audited source of truth for frame, parsing, and queue budgets to prevent drift and unbounded code paths across TAK, SAPIENT, bridge, and replay.

```rust
#[derive(Debug, Clone)]
pub struct Limits {
    pub max_frame_bytes: usize,
    pub max_xml_scan_bytes: usize,
    pub max_protobuf_bytes: usize,
    pub max_queue_messages: usize,
    pub max_queue_bytes: usize,
    pub max_detail_elements: usize,
}

impl Limits {
    pub fn conservative_defaults() -> Self;
    pub fn validate(&self) -> Result<(), LimitsError>;
}
```

Design note:
- Boundary-facing configs (`WireConfig`, `TransportConfig`, `SapientConfig`, bridge emitters) take `Limits` instead of independent ad-hoc numeric fields.

---

### 6.3 `rustak-io` — Async Traits & Adapters

**Purpose:** Runtime-agnostic async interfaces shared across transport, simulation, and record/replay. This keeps “send/receive CoT” plumbing out of `rustak-core` and prevents crate boundary violations.

Design notes:
- Prefer `futures` traits over `tokio` traits so this crate stays runtime-agnostic.
- Keep IO contracts generic (`MessageSink<T>`, `MessageSource<T>`) so CoT, SAPIENT, replay, and bridge pipelines share middleware.
- Keep traits object-safe so they can be used behind `Box<dyn ...>`.
- Avoid hidden boxing costs in public traits; use explicit boxed futures for dyn compatibility.

API sketch:
```rust
use bytes::Bytes;
use futures::future::BoxFuture;

#[derive(Debug, thiserror::Error)]
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

/// Wall and monotonic timestamps for stable replay and audit correlation.
#[derive(Debug, Clone)]
pub struct ObservedTime {
    pub wall: std::time::SystemTime,
    pub monotonic: std::time::Instant,
}

/// Standard metadata wrapper for received messages.
#[derive(Debug, Clone)]
pub struct MessageEnvelope<T> {
    pub observed: ObservedTime,
    pub peer: Option<std::net::SocketAddr>,
    pub raw_frame: Option<Bytes>,   // Present when the source can provide it
    pub message: T,
}

pub type CotEnvelope = MessageEnvelope<CotEvent>;

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
    fn into_stream(self: Box<Self>) -> Pin<Box<dyn Stream<Item = Result<MessageEnvelope<T>, IoError>> + Send>>;
}

pub type CotSink = dyn MessageSink<CotEvent>;
pub type CotSource = dyn MessageSource<CotEvent>;

/// Shared network impairment model for stress tests and deterministic regressions.
#[derive(Debug, Clone)]
pub struct NetworkImpairment {
    pub loss_probability: f64,        // 0.0-1.0
    pub duplicate_probability: f64,   // 0.0-1.0
    pub min_latency: Duration,
    pub max_latency: Duration,
    pub reorder_probability: f64,     // 0.0-1.0
}

/// IO middleware adapters (stackable and reusable across CoT/SAPIENT/replay).
pub mod layers {
    use super::*;

    pub struct TapLayer { /* capture raw_frame or decoded messages */ }
    pub struct RateLimitLayer { /* token bucket */ }
    pub struct DedupLayer<K> { /* windowed dedup */ }
    pub struct CoalesceLatestLayer<K> { /* coalesce by key (e.g., UID) */ }
    pub struct ImpairmentLayer { /* loss, duplicate, latency, reorder */ }
    pub struct MetricsLayer { /* counters and histograms */ }
}
```

**Key dependencies:** `futures`, `bytes`, `thiserror`

---

### 6.4 `rustak-cot` — CoT XML Engine

**Purpose:** Bounded-memory CoT XML parsing and serialization with strong semantic fidelity. Lossless replay is achieved via raw-frame capture in `rustak-record`, not by promising byte-for-byte XML round-trips.

```rust
/// Parse raw XML bytes into a CotEvent.
pub fn parse_xml(xml: &[u8]) -> Result<CotEvent, CotError>;

/// Serialise a CotEvent to CoT XML bytes.
pub fn to_xml(event: &CotEvent) -> Result<Vec<u8>, CotError>;

/// Serialise with pretty-printing (useful for debugging/logging).
pub fn to_xml_pretty(event: &CotEvent) -> Result<String, CotError>;

/// Validate XML against the CoT schema without full deserialisation.
pub fn validate_xml(xml: &[u8]) -> Result<(), Vec<ValidationError>>;

/// Round-trip test helper: parse then re-serialise and compare.
pub fn round_trip_check(xml: &[u8]) -> Result<RoundTripResult, CotError>;

/// Optional: parse while retaining the raw event bytes for audit-grade recording.
pub fn parse_xml_with_raw(xml: &[u8]) -> Result<(CotEvent, Vec<u8>), CotError>;
```

**Design decisions:**
- Uses `quick-xml` for parsing (fast, zero-copy where possible)
- Preserves unknown XML elements/attributes in ordered `CotDetail::elements` for forward compatibility (semantic preservation)
- Supports a pluggable extension registry (typed known extensions + opaque pass-through)
- XML namespace handling for TAK-specific extensions
- Wire framing (delimiter/length scanning, max-frame limits) is handled by `rustak-wire`
- Byte-exact replay is provided by storing raw frames in `rustak-record` when configured

---

### 6.5 `rustak-wire` — TAK Protocol Framing & Negotiation

**Purpose:** Implement TAK wire framing rules for both legacy XML streaming and TAK Protocol v1 streaming/mesh frames. Provide a negotiation state machine to upgrade from legacy XML to TAK Protocol when supported.

Key responsibilities:
- Varint codec (unsigned varint; bounded to 10 bytes; strict overflow handling)
- Legacy XML stream framing (delimiter/bounded scan; no unbounded buffering)
- TAK Protocol streaming framing (TAK server connections):
  - Header: `[0xBF][varint payload_length][payload_bytes...]`
  - Version is not included in the streaming header; it is negotiated out-of-band via CoT XML control events.
- TAK Protocol mesh framing (UDP datagrams):
  - Header: `[0xBF][varint protocol_version][payload_bytes...]`
- Streaming negotiation via CoT XML control events (TakProtocolSupport, TakRequest, TakResponse, protouid correlation)
- Mesh negotiation via TakControl messages and per-contact version tracking
- Hard limits (max frame size, max varint length) to prevent memory DoS
- Negotiation invariants and timeouts are part of the implementation contract:
  - Streaming: support offer is sent at most once per connection; peer response is awaited for a bounded timeout before reconnect logic applies
  - Mesh: TakControl advertisement cadence and contact staleness windows are configurable (defaults align with TAK commoncommo guidance)
- Negotiation telemetry events are emitted for audit/replay so downgrade decisions are explainable post-incident

API sketch:
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WireFormat {
    Xml,
    TakProtocolV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TakProtocolVersion {
    V1,
}

pub enum WireFrame<'a> {
    LegacyXml(&'a [u8]),   // bytes for one <event ...>...</event>
    TakStream { payload: &'a [u8] },             // streaming payload; version known from negotiation
    TakMesh { version: u32, payload: &'a [u8] }, // mesh payload; version carried in header
}

pub struct WireConfig {
    pub limits: rustak_limits::Limits,
    pub negotiation: NegotiationConfig,
}

pub struct NegotiationConfig {
    pub streaming_timeout: Duration,         // Default: 60s
    pub mesh_takcontrol_interval: Duration,  // Default: 60s
    pub mesh_contact_stale_after: Duration,  // Default: 120s
    pub downgrade_policy: DowngradePolicy,   // Explicit security posture
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DowngradePolicy {
    FailOpen,   // Allow fallback to legacy XML if upgrade fails
    FailClosed, // Refuse fallback; treat as suspicious/misconfigured peer
}

pub struct Negotiator { /* states */ }
impl Negotiator {
    pub fn observe_legacy_event(&mut self, event: &CotEvent) -> NegotiationEvent;
    pub fn observe_mesh_control(&mut self, event: &CotEvent) -> NegotiationEvent;
    pub fn next_action(&mut self) -> Option<NegotiationAction>;
}

#[derive(Debug, Clone)]
pub struct NegotiationEvent {
    pub kind: NegotiationEventKind,
    pub reason: Option<NegotiationReason>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NegotiationReason {
    Timeout,
    MalformedControl,
    UnsupportedVersion,
    PolicyDenied,
}
```

---

### 6.6 `rustak-proto` — TAK Protocol v1 Payload (Protobuf) Support

**Purpose:** Encode/decode TAK Protocol v1 payloads (Protobuf) to/from `CotEvent`. Wire framing and protocol negotiation are handled by `rustak-wire`.

```rust
/// TAK Protocol v1 payload is a Protobuf message.
/// Wire framing and negotiation are handled by `rustak-wire`:
/// - Streaming: [0xBF][varint length][payload]
/// - Legacy XML remains delimiter-framed until negotiated upgrade

/// Decode TAK Protocol v1 payload bytes into a CotEvent.
pub fn decode_v1_payload(bytes: &[u8]) -> Result<CotEvent, ProtoError>;

/// Encode a CotEvent into TAK Protocol v1 payload bytes.
pub fn encode_v1_payload(event: &CotEvent) -> Result<Vec<u8>, ProtoError>;

/// Decode payload bytes when the negotiated version is known.
pub fn decode_payload(bytes: &[u8], version: TakProtocolVersion) -> Result<CotEvent, ProtoError>;
```

**Key dependency:** `prost` for protobuf code generation from `.proto` files

---

### 6.7 `rustak-crypto` — Certificate & Crypto Management

**Purpose:** Defence-grade TLS configuration, certificate management, and TAK Server trust store handling.

```rust
/// TAK certificate/trust store manager.
/// Handles the .p12/.pem certificates that TAK Server generates.
pub struct TakCertStore {
    // ...
}

impl TakCertStore {
    /// Load from a TAK Server-generated truststore (.p12).
    /// Default TAK password is "atakatak".
    pub fn from_p12(path: impl AsRef<Path>, password: &str) -> Result<Self, CryptoError>;

    /// Load from separate PEM files (CA cert, client cert, client key).
    pub fn from_pem(
        ca_cert: impl AsRef<Path>,
        client_cert: impl AsRef<Path>,
        client_key: impl AsRef<Path>,
    ) -> Result<Self, CryptoError>;

    /// Build a rustls ClientConfig for mTLS connections.
    pub fn to_tls_config(&self) -> Result<rustls::ClientConfig, CryptoError>;

    /// Build a rustls ServerConfig (for acting as a TAK Server).
    pub fn to_server_tls_config(&self) -> Result<rustls::ServerConfig, CryptoError>;

    /// Validate certificate chain and expiry.
    pub fn validate(&self) -> Result<CertValidation, CryptoError>;

    /// Check if certificates expire within the given duration.
    pub fn expires_within(&self, duration: Duration) -> bool;

    /// Optional: pin server certificate SPKI hash for high-assurance deployments.
    pub fn with_server_spki_pin(self, spki_sha256: [u8; 32]) -> Self;

    /// Optional: enable revocation checking hooks (CRL / OCSP) when available.
    pub fn with_revocation(self, mode: RevocationMode) -> Self;
}

#[derive(Debug, Clone, Copy)]
pub enum RevocationMode {
    Off,
    Prefer,   // best-effort: validate when stapled/available
    Require,  // fail handshake if revocation status cannot be determined
}

/// Crypto provider selection for rustls.
/// Note: "FIPS-capable" means the library can be configured to use FIPS-approved cryptography.
/// Actual compliance depends on the validated module, build, and deployment environment.
#[derive(Debug, Clone, Copy)]
pub enum CryptoProviderMode {
    Ring,
    AwsLcRs,
    AwsLcRsFips,
}
```

**Key dependencies:** `rustls`, `webpki`, `rcgen` (plus optional `aws-lc-rs` provider)

---

### 6.7a `rustak-net` — Shared Networking Primitives

**Purpose:** Consolidate connection management concerns that would otherwise be duplicated in TAK transport and SAPIENT session code.

Key responsibilities:
- `tokio` TCP/TLS/WebSocket connection wrappers with consistent timeout and backoff behavior
- UDP socket helpers (unicast, multicast, broadcast) with platform-specific socket options isolated in one place
- Generic framed IO primitives (length-prefix, delimiter, bounded reader); protocol-specific codecs live in `rustak-wire`/`rustak-sapient`
- Optional tap hooks for capturing raw frames for record/audit without leaking protocol details into IO primitives

**Non-goals:**
- No knowledge of CoT, TAK negotiation, or SAPIENT semantics; this crate is purely resilient async IO and generic framing glue

**Key dependencies:** `tokio`, `tokio-util`, `bytes`, `tracing`

---

### 6.8 `rustak-transport` — Network Layer

**Purpose:** All transport protocols TAK uses, with a unified async interface and deterministic overload behavior (priority lanes, coalescing, MTU-safe UDP policy).
Note: mesh semantics (contact tracking, TakControl cadence, mesh version selection) live in `rustak-commo` above this layer.

```rust
/// Transport configuration builder.
pub struct TransportConfig {
    pub protocol: Protocol,
    pub wire_format: WireFormat,        // XML or TAK Protocol v1 (negotiated upgrade)
    pub limits: rustak_limits::Limits,  // Single source of truth for frame/queue/parser bounds
    pub read_timeout: Duration,         // Fail fast on dead peers
    pub write_timeout: Duration,
    pub keepalive: Option<Keepalive>,   // Optional heartbeat/ping for persistent connections
    pub reconnect_policy: ReconnectPolicy,
    pub metrics: bool,                  // Enable transport metrics
    pub mtu_safety: Option<MtuSafety>,  // Avoid IP fragmentation on UDP links
    pub send_queue: SendQueueConfig,    // Priority and coalescing behavior
}

#[derive(Debug, Clone)]
pub struct Keepalive {
    pub interval: Duration,
    pub timeout: Duration,
}

#[derive(Debug, Clone)]
pub enum Protocol {
    /// UDP unicast or multicast. Standard TAK multicast: 239.2.3.1:6969
    Udp {
        bind_addr: SocketAddr,
        target: UdpTarget,
    },
    /// Plain TCP (not recommended for production).
    Tcp {
        addr: SocketAddr,
    },
    /// TCP with mTLS — the standard TAK Server connection method.
    Tls {
        addr: SocketAddr,
        cert_store: TakCertStore,
        server_name: String,
    },
    /// WebSocket (used by TAK Server WebAPI).
    WebSocket {
        url: String,
        cert_store: Option<TakCertStore>,
    },
}

#[derive(Debug, Clone)]
pub enum UdpTarget {
    Unicast(SocketAddr),
    Multicast { group: Ipv4Addr, port: u16 },
    Broadcast { port: u16 },
}

/// Reconnection policy for persistent connections.
#[derive(Debug, Clone)]
pub struct ReconnectPolicy {
    pub enabled: bool,
    pub initial_delay: Duration,      // Default: 1s
    pub max_delay: Duration,          // Default: 60s
    pub backoff_factor: f64,          // Default: 2.0
    pub jitter: f64,                  // 0.0-1.0 randomization to avoid reconnect storms
    pub max_retries: Option<u32>,     // None = infinite
}

#[derive(Debug, Clone)]
pub struct MtuSafety {
    pub max_udp_payload_bytes: usize,   // Conservative default; configurable per environment
    pub drop_oversize: bool,            // If false, attempts best-effort split policy where possible
}

#[derive(Debug, Clone)]
pub struct SendQueueConfig {
    pub max_messages: usize,
    pub max_bytes: usize,
    pub mode: SendQueueMode,
}

#[derive(Debug, Clone)]
pub enum SendQueueMode {
    Fifo,
    Priority { classifier: EventPriorityClassifier },
    CoalesceLatestByUid { classifier: EventPriorityClassifier },
}

pub type EventPriorityClassifier = Box<dyn Fn(&CotEvent) -> u8 + Send + Sync>;

#[derive(Debug, Clone, Copy)]
pub enum OverloadPolicy {
    DropOldest,
    DropNewest,
    ShedByType, // e.g., drop noisy chat/heartbeat events first
    CoalesceLatestByUid,
}

// ── Connection types ─────────────────────────────────────────────────

/// A connected TAK transport that can send and receive events.
pub struct TakConnection {
    // Internal: tokio channels + transport task handle
    // Internal: bounded channels + framing codec based on rustak-wire
    // - Legacy XML delimiter framing with bounded scan
    // - TAK Protocol streaming framing using varint length frames
}

/// Split API: sender is cloneable; receiver is single-consumer.
pub struct TakSender { /* ... */ }
pub struct TakReceiver { /* ... */ }

impl TakConnection {
    /// Establish a connection with the given config.
    pub async fn connect(config: TransportConfig) -> Result<Self, TransportError>;

    /// Send a single CoT event.
    pub async fn send(&self, event: CotEvent) -> Result<(), TransportError>;

    /// Send multiple events.
    pub async fn send_batch(&self, events: Vec<CotEvent>) -> Result<(), TransportError>;

    /// Receive the next CoT event.
    pub async fn recv(&mut self) -> Result<CotEnvelope, TransportError>;

    /// Get a stream of incoming events.
    pub fn incoming(&self) -> impl Stream<Item = Result<CotEnvelope, TransportError>> + '_;

    /// Split into independently usable halves (typical for production pipelines).
    pub fn split(self) -> (TakSender, TakReceiver);

    /// Get transport metrics (bytes sent/recv, message counts, latency).
    pub fn metrics(&self) -> TransportMetrics;

    /// Apply a filter on incoming events at the connection boundary to reduce downstream load.
    pub fn set_incoming_filter(&self, filter: Box<dyn Fn(&CotEvent) -> bool + Send + Sync>);

    /// Drop policy for overload (drop-oldest, drop-newest, shed-by-type, coalesce-by-UID).
    pub fn set_overload_policy(&self, policy: OverloadPolicy);

    /// Gracefully disconnect.
    pub async fn disconnect(self) -> Result<(), TransportError>;
}

/// Convenience: listen on UDP multicast (the simplest TAK setup).
pub async fn listen_multicast(
    group: Ipv4Addr,
    port: u16,
) -> Result<TakConnection, TransportError>;

/// Convenience: connect to TAK Server with mTLS.
pub async fn connect_tak_server(
    addr: SocketAddr,
    cert_store: TakCertStore,
) -> Result<TakConnection, TransportError>;
```

**Key dependencies:** `tokio`, `tokio-rustls`, `tokio-tungstenite`

---

### 6.8a `rustak-commo` — TAK Comms Core

**Purpose:** Provide the “TAK common comms” behaviors required for real mesh interoperability (inspired by ATAK common commo patterns, but with a Rust-first, testable implementation).

Key responsibilities:
- Track known contacts and their supported TAK protocol versions (min/max windows).
- Emit periodic TakControl and SA presence messages as configured.
- Select the transmit protocol version for mesh sends as the highest version supported by all known contacts (or per-policy fallback).
- Provide bounded queues, drop policies, and rate limiting tuned for constrained tactical networks.

API sketch:
```rust
pub struct CommoConfig {
    pub control_interval: Duration,
    pub sa_interval: Duration,
    pub rate_limit: Option<u32>, // messages/sec
    pub overload_policy: OverloadPolicy,
}

pub struct CommoNode { /* transport + wire + contact table */ }
impl CommoNode {
    pub async fn run(&mut self) -> Result<(), CommoError>;
    pub fn contacts(&self) -> Vec<ContactInfo>;
}
```

---

### 6.9 `rustak-server` — TAK Server Client

**Purpose:** Higher-level TAK Server interactions beyond raw transport: authentication, data packages, channels, mission management.

```rust
/// TAK Server client with full API support.
pub struct TakServerClient {
    // ...
}

impl TakServerClient {
    /// Connect to a TAK Server instance.
    pub async fn connect(config: TakServerConfig) -> Result<Self, ServerError>;

    // ── Streaming CoT (separate from management API) ─────────────────
    /// Get a handle for sending/receiving CoT on the streaming channel.
    pub fn cot_channel(&self) -> &TakConnection;

    /// Query server capabilities discovered during handshake and/or via API.
    pub fn capabilities(&self) -> &ServerCapabilities;

    // ── Data Packages ────────────────────────────────────────────────
    /// Upload a data package (.zip) to the server.
    pub async fn upload_data_package(&self, pkg: &DataPackage) -> Result<String, ServerError>;

    /// Download a data package by hash.
    pub async fn download_data_package(&self, hash: &str) -> Result<DataPackage, ServerError>;

    /// List available data packages.
    pub async fn list_data_packages(&self) -> Result<Vec<DataPackageMeta>, ServerError>;

    // ── Missions ─────────────────────────────────────────────────────
    /// List missions on the server.
    pub async fn list_missions(&self) -> Result<Vec<Mission>, ServerError>;

    /// Subscribe to a mission feed.
    pub async fn subscribe_mission(&self, name: &str) -> Result<MissionSubscription, ServerError>;

    /// Create a new mission.
    pub async fn create_mission(&self, config: MissionConfig) -> Result<Mission, ServerError>;

    // ── Contacts ─────────────────────────────────────────────────────
    /// List connected clients/contacts on the server.
    pub async fn list_contacts(&self) -> Result<Vec<TakContact>, ServerError>;

    // ── Federation ───────────────────────────────────────────────────
    /// List federated servers.
    pub async fn list_federates(&self) -> Result<Vec<Federate>, ServerError>;

    // ── Health ────────────────────────────────────────────────────────
    /// Check server health/status.
    pub async fn health_check(&self) -> Result<ServerHealth, ServerError>;
}

/// TAK Server configuration.
pub struct TakServerConfig {
    pub host: String,
    pub streaming_port: u16,        // Default: 8089 (TLS)
    pub api_port: u16,              // Default: 8443 (HTTPS)
    pub cert_store: TakCertStore,
    pub wire_format: WireFormat,    // XML (legacy) or TAK Protocol v1 (after negotiation)
    pub protocol_negotiation: NegotiationMode, // auto, force-xml, force-v1
    pub reconnect: ReconnectPolicy,
}

pub enum NegotiationMode {
    Auto,      // start legacy XML, upgrade if supported
    ForceXml,
    ForceTakV1,
}
```

---

### 6.10 `rustak-sim` — Scenario Simulation Engine

**Purpose:** The key differentiator. Generate realistic synthetic drone tracks, define threat scenarios, and stress-test C-UAS systems programmatically.

```rust
// ── Flight path primitives ───────────────────────────────────────────

/// A waypoint in a flight path.
#[derive(Debug, Clone)]
pub struct Waypoint {
    pub position: Position,
    pub speed: f64,                  // Target speed at waypoint (m/s)
    pub altitude_agl: Option<f64>,   // Altitude above ground level
    pub loiter: Option<Duration>,    // Time to loiter at waypoint
}

/// Flight path behaviour between waypoints.
#[derive(Debug, Clone)]
pub enum PathInterpolation {
    Linear,                         // Straight lines between waypoints
    GreatCircle,                    // Geodesic interpolation
    CubicSpline,                    // Smooth curves through waypoints
    Dubins { turn_radius: f64 },    // Realistic fixed-wing turning
}

/// Predefined flight patterns.
#[derive(Debug, Clone)]
pub enum FlightPattern {
    /// Orbit a fixed point at given radius and altitude.
    Orbit {
        centre: Position,
        radius: f64,                // metres
        altitude_agl: f64,          // metres AGL
        speed: f64,                 // m/s
        clockwise: bool,
        entry_heading: Option<f64>, // degrees
    },
    /// Linear transit between two points.
    Transit {
        from: Position,
        to: Position,
        altitude_agl: f64,
        speed: f64,
    },
    /// Racetrack/figure-8 pattern (common ISR pattern).
    Racetrack {
        centre: Position,
        length: f64,                // metres
        width: f64,                 // metres
        altitude_agl: f64,
        speed: f64,
        heading: f64,               // degrees, orientation of the long axis
    },
    /// Grid/lawnmower search pattern.
    GridSearch {
        bounds: GeoBounds,
        altitude_agl: f64,
        speed: f64,
        line_spacing: f64,          // metres between grid lines
    },
    /// Hover at a fixed position (multi-rotor).
    Hover {
        position: Position,
        altitude_agl: f64,
        duration: Duration,
    },
    /// Waypoint sequence.
    Waypoints {
        points: Vec<Waypoint>,
        interpolation: PathInterpolation,
    },
    /// Random/erratic movement within a bounding box.
    /// Useful for simulating an uncooperative/evasive drone.
    Erratic {
        bounds: GeoBounds,
        altitude_range: (f64, f64),
        max_speed: f64,
        direction_change_interval: Duration,
    },
}

// ── Noise & realism ──────────────────────────────────────────────────

/// Configurable noise model applied to synthetic tracks to simulate
/// real-world sensor imperfections.
#[derive(Debug, Clone)]
pub struct NoiseModel {
    pub position_sigma: f64,          // metres (GPS-like horizontal noise)
    pub altitude_sigma: f64,          // metres (vertical noise)
    pub speed_sigma: f64,             // m/s
    pub heading_sigma: f64,           // degrees
    pub dropout_probability: f64,     // Probability of missing an update (0.0-1.0)
    pub drift_rate: Option<f64>,      // Systematic drift in m/s (GPS drift)
    pub jitter: Option<Duration>,     // Timing jitter on update intervals
}

impl NoiseModel {
    pub fn gps_realistic() -> Self;   // Typical consumer GPS noise
    pub fn radar_track() -> Self;     // Typical radar tracker noise
    pub fn optical_track() -> Self;   // Typical optical/EO tracker noise
    pub fn rf_detection() -> Self;    // RF-based detection noise profile
    pub fn perfect() -> Self;         // Zero noise (for truth data)
    pub fn degraded() -> Self;        // Intentionally poor quality
}

// ── Network impairment (applies to any IO boundary) ────────────────

/// Reuse the shared IO impairment model so replay, transport, and simulation
/// apply identical fault semantics.
pub type NetworkImpairment = rustak_io::NetworkImpairment;

// ── SAPIENT feed simulation (bridge-focused) ───────────────────────

/// Generate SAPIENT detection streams from truth tracks + sensor models.
pub struct SapientSimNode {
    pub node_id: String,
    pub impairment: Option<NetworkImpairment>,
    // sensor model, detection latency, classification churn, etc.
}

impl SapientSimNode {
    pub fn from_scenario(scenario: &Scenario) -> Self;
    pub fn stream(&self) -> impl Stream<Item = v2_0::SapientMessage>;
}

// ── Entity definition ────────────────────────────────────────────────

/// A simulated entity (drone, aircraft, vehicle, etc.).
#[derive(Debug, Clone)]
pub struct SimEntity {
    pub uid: Uid,
    pub callsign: String,
    pub cot_type: CotType,
    pub affiliation: Affiliation,
    pub platform: DronePlatform,
    pub flight_pattern: FlightPattern,
    pub noise: NoiseModel,
    pub update_rate: Duration,          // How often to emit CoT updates
    pub start_delay: Option<Duration>,  // Delay before entity appears
    pub duration: Option<Duration>,     // How long entity is active
    pub detail: Option<DroneDetail>,    // Additional metadata
}

// ── Scenario definition ──────────────────────────────────────────────

/// A complete test scenario with multiple entities and timing.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone)]
pub struct Scenario {
    pub name: String,
    pub description: String,
    pub schema_version: u32,             // Scenario format versioning for long-term stability
    pub seed: Option<u64>,               // Deterministic RNG seed; None => generated and recorded
    pub includes: Vec<PathBuf>,          // Optional reusable scenario snippets/overlays
    pub duration: Duration,
    pub time_scale: f64,                 // 1.0 = real-time, 2.0 = double speed
    pub entities: Vec<SimEntity>,
    pub triggers: Vec<ScenarioTrigger>,  // Conditional events
}

/// Terrain model for AGL -> HAE conversion and realism (optional).
pub trait TerrainModel: Send + Sync {
    fn elevation_m(&self, lat: f64, lon: f64) -> Option<f64>;
}

/// Conditional triggers that modify the scenario at runtime.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone)]
pub enum ScenarioTrigger {
    /// Spawn new entities after a delay.
    TimedSpawn {
        delay: Duration,
        entities: Vec<SimEntity>,
    },
    /// Change an entity's flight pattern at a given time.
    PatternChange {
        entity_uid: Uid,
        at: Duration,
        new_pattern: FlightPattern,
    },
    /// Simulate entity loss (goes dark).
    TrackLoss {
        entity_uid: Uid,
        at: Duration,
        resume_after: Option<Duration>,
    },
    /// Simulate a swarm dispersal event.
    SwarmDisperse {
        entity_uids: Vec<Uid>,
        at: Duration,
        disperse_radius: f64,
    },

    /// Trigger based on a condition (geofence entry, distance threshold, time window).
    Condition {
        when: TriggerCondition,
        actions: Vec<TriggerAction>,
    },
}

pub enum TriggerCondition { /* geofence entry/exit, distance-to-point, speed threshold */ }
pub enum TriggerAction { /* spawn, pattern change, track loss, metadata change */ }

/// Load a scenario from a YAML/JSON file.
pub fn load_scenario(path: impl AsRef<Path>) -> Result<Scenario, SimError>;

/// Save a scenario to a YAML/JSON file.
pub fn save_scenario(scenario: &Scenario, path: impl AsRef<Path>) -> Result<(), SimError>;

// ── Scenario runner ──────────────────────────────────────────────────

/// Runs a scenario, generating CoT events and sending them
/// to one or more sinks (TAK Server, file, analysis pipeline).
pub struct ScenarioRunner {
    // ...
}

impl ScenarioRunner {
    pub fn new(scenario: Scenario) -> Self;
    pub fn with_terrain_model(self, terrain: Box<dyn TerrainModel>) -> Self;
    pub fn with_seed(self, seed: u64) -> Self; // override scenario seed for sweeps

    /// Add a CoT output sink (e.g., TakConnection, file writer).
    pub fn add_sink(&mut self, sink: Box<CotSink>);

    /// Add a truth data recorder (logs ground-truth positions).
    pub fn add_truth_recorder(&mut self, recorder: Box<dyn TruthRecorder>);

    /// Run the scenario to completion.
    pub async fn run(&mut self) -> Result<ScenarioReport, SimError>;

    /// Run with real-time control (pause, resume, adjust time scale).
    pub async fn run_interactive(&mut self) -> Result<ScenarioHandle, SimError>;

    /// Get current state of all entities.
    pub fn entity_states(&self) -> Vec<EntityState>;
}

/// Separates deterministic ground-truth evolution from protocol-specific observation.
pub struct TruthEngine { /* deterministic state evolution */ }

/// Sensor model consumes truth and emits observations (CoT or SAPIENT).
pub trait SensorModel: Send + Sync {
    fn observe(&mut self, truth: &EntityTruthState) -> Vec<SensorObservation>;
}

/// Parameter sweeps for evaluation work: run scenario grids and aggregate reports.
pub struct SweepRunner { /* scenario template + parameter grid */ }
impl SweepRunner {
    pub async fn run(&mut self) -> Result<Vec<ScenarioReport>, SimError>;
}

/// Handle for controlling a running scenario.
pub struct ScenarioHandle {
    // ...
}

impl ScenarioHandle {
    pub async fn pause(&self);
    pub async fn resume(&self);
    pub async fn set_time_scale(&self, scale: f64);
    pub async fn inject_entity(&self, entity: SimEntity);
    pub async fn remove_entity(&self, uid: Uid);
    pub async fn stop(self) -> ScenarioReport;
}

// ── Pre-built scenarios ──────────────────────────────────────────────

pub mod scenarios {
    /// Single drone transiting an area.
    pub fn single_transit(from: Position, to: Position) -> Scenario;

    /// Multiple drones orbiting a target.
    pub fn multi_orbit(centre: Position, count: u32) -> Scenario;

    /// Swarm attack scenario with configurable swarm size.
    pub fn swarm_attack(target: Position, swarm_size: u32) -> Scenario;

    /// Low-slow-small target (typical C-UAS challenge).
    pub fn low_slow_small(area: GeoBounds) -> Scenario;

    /// Mixed threat environment (fixed-wing + multi-rotor + decoys).
    pub fn mixed_threat(area: GeoBounds) -> Scenario;

    /// Stress test: maximum entity count for performance testing.
    pub fn stress_test(centre: Position, entity_count: u32) -> Scenario;

    /// GPS spoofing scenario (tracks that jump/drift unrealistically).
    pub fn gps_spoofing(target: Position) -> Scenario;
}
```

---

### 6.11 `rustak-record` — Record & Replay

**Purpose:** Capture, store, and replay event streams with timing fidelity across multiple protocols (TAK XML, TAK Protocol v1, SAPIENT), including optional raw-frame retention for audit and reproducibility.

Container contract (`.takrec`):
- Versioned header with tool/version metadata, protocol hints, and limits profile
- Chunked append format with per-chunk checksums and crash-safe flush semantics
- Streaming writer with rebuildable index for recovery when index sidecar is missing
- Optional integrity chain/signing metadata for tamper-evident workflows

```rust
/// Record envelopes to a file with precise timing information.
pub struct CotRecorder {
    // ...
}

impl CotRecorder {
    /// Create a recorder writing to the given path.
    /// Supports:
    /// - .takrec (chunked binary container: versioned, self-describing, crash-safe, zstd-compressed, indexed, checksummed)
    /// - .jsonl (newline-delimited JSON, streaming-friendly)
    /// - .csv (analysis-friendly, lossy for nested detail)
    pub fn new(path: impl AsRef<Path>, format: RecordFormat) -> Result<Self, RecordError>;

    /// Record a single received envelope (preferred; carries monotonic and wall time).
    pub async fn record_received(&mut self, env: &CotEnvelope) -> Result<(), RecordError>;

    /// Record a single sent event (timestamp is assigned at call time).
    pub async fn record_sent(&mut self, event: &CotEvent) -> Result<(), RecordError>;

    /// Record ground-truth data alongside the CoT event.
    pub async fn record_with_truth(
        &mut self,
        event: &CotEvent,
        truth: &TruthData,
    ) -> Result<(), RecordError>;

    /// Flush and close the recording.
    pub async fn close(self) -> Result<RecordingSummary, RecordError>;
}

/// Replay recorded CoT events with original timing.
pub struct CotReplayer {
    // ...
}

impl CotReplayer {
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, RecordError>;

    /// Seek to an offset using the recording index (takrec only).
    pub fn seek(&mut self, t: Duration) -> Result<(), RecordError>;

    /// Replay to a sink, preserving original timing intervals.
    pub async fn replay_realtime(&mut self, sink: &CotSink) -> Result<(), RecordError>;

    /// Replay at a different speed.
    pub async fn replay_scaled(&mut self, sink: &CotSink, time_scale: f64) -> Result<(), RecordError>;

    /// Get all events as a vector (no timing, for analysis).
    pub fn all_events(&self) -> Result<Vec<TimestampedEvent>, RecordError>;
}

/// Recorded envelope for audit-grade capture.
pub struct RecordedEnvelope {
    pub direction: Direction,          // Inbound | Outbound
    pub observed_at: TimestampUtc,     // Wall time for audit correlation
    pub t_offset_ns: u64,              // Monotonic offset from recording start for stable replay
    pub protocol: ProtocolKind,        // TakXml | TakProtoV1Stream | TakProtoV1Mesh | SapientV2
    pub raw_frame: Option<Vec<u8>>,    // optional raw bytes for exact replay and audit
    pub event: Option<CotEvent>,       // decoded CoT event when applicable
    pub sapient: Option<Vec<u8>>,      // decoded SAPIENT message bytes (or a typed struct behind a feature)
    pub metadata: Vec<(String, String)>,
}

pub enum Direction {
    Inbound,
    Outbound,
}

pub enum ProtocolKind {
    TakXml,
    TakProtoV1Stream,
    TakProtoV1Mesh,
    SapientV2,
    Other(String),
}

/// Optional: tamper-evident mode for .takrec recordings.
pub struct IntegrityConfig {
    pub hash_chain: bool,
    pub signing_key: Option<PathBuf>,
}

/// Interop helpers for audit and debugging.
pub mod interop {
    /// Export a recording to PCAP with per-packet annotations (direction, protocol, peer).
    pub fn export_pcap(input: impl AsRef<Path>, output: impl AsRef<Path>) -> Result<(), RecordError>;

    /// Import PCAP and attempt to decode frames into a takrec stream (best-effort).
    pub fn import_pcap(input: impl AsRef<Path>, output: impl AsRef<Path>) -> Result<(), RecordError>;
}

/// Ground truth data for accuracy assessment.
#[derive(Debug, Clone)]
pub struct TruthData {
    pub true_position: Position,
    pub true_speed: f64,
    pub true_heading: f64,
    pub entity_uid: Uid,
    pub timestamp: TimestampUtc,
}

/// Analysis utilities for comparing detected tracks against truth.
pub mod analysis {
    /// Calculate position error between detected and true positions.
    pub fn position_error(detected: &Position, truth: &Position) -> f64; // metres

    /// Calculate track accuracy metrics over a recording.
    pub fn track_accuracy(
        detected: &[TimestampedEvent],
        truth: &[TruthData],
    ) -> TrackAccuracyReport;

    /// Detection rate: what percentage of truth updates had a corresponding detection?
    pub fn detection_rate(
        detected: &[TimestampedEvent],
        truth: &[TruthData],
        max_association_distance: f64,
    ) -> f64;

    /// Latency analysis: time between truth event and detection event.
    pub fn detection_latency(
        detected: &[TimestampedEvent],
        truth: &[TruthData],
    ) -> LatencyReport;

    /// Export derived metrics and event/truth streams to columnar formats.
    pub fn export_parquet(
        detected: &[TimestampedEvent],
        truth: &[TruthData],
        path: impl AsRef<Path>,
    ) -> Result<(), RecordError>;
}
```

---

### 6.11a `rustak-config` — Configuration Contracts

**Purpose:** Centralize config loading, validation, schema generation, and redaction so services and CLI share a single audited config surface.

Key responsibilities:
- Define typed workspace config structs with strict validation (`limits`, negotiation policies, bridge mappings).
- Generate JSON schema for operator tooling and CI checks.
- Redact sensitive fields for logs and diagnostics.
- Provide compatibility/migration helpers for versioned config files.

API sketch:
```rust
pub struct RustakConfig { /* transport + sapient + bridge + admin + logging */ }

impl RustakConfig {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, ConfigError>;
    pub fn validate(&self) -> Result<(), ConfigError>;
    pub fn to_redacted_yaml(&self) -> Result<String, ConfigError>;
    pub fn json_schema() -> serde_json::Value;
}
```

---

### 6.11b `rustak-admin` — Optional Admin Endpoints

**Purpose:** Optional HTTP admin surface for long-running services (health, metrics, and controlled reload hooks) without coupling those concerns to the CLI crate.

Key responsibilities:
- Expose `/healthz`, `/metrics`, and optional `/reload` endpoints.
- Integrate with `rustak-config` validation and redaction.
- Keep endpoint exposure explicit with bind-address controls and feature-gated build inclusion.

Design note:
- Disabled by default for library consumers; enable via a crate feature in service binaries.

---

### 6.12 `rustak-cli` — Command-Line Tool

**Purpose:** A single binary for testing, diagnostics, simulation, and integration work. Your Swiss army knife for TAK work.

```
rustak-cli

USAGE:
    rustak <SUBCOMMAND>

SUBCOMMANDS:
    listen      Listen for CoT messages on a transport
    send        Send a CoT message (position, alert, etc.)
    connect     Connect to a TAK Server with mTLS
    sim         Run a simulation scenario
    replay      Replay a recorded CoT stream
    record      Record incoming CoT messages to file
    validate    Validate CoT XML, TAK Protocol v1, and SAPIENT frames
    convert     Convert between CoT XML and TAK Protocol v1
    certs       Certificate management utilities
    scenario    Create/edit/list scenario files
    stress      Run a stress test against a TAK endpoint
    health      Check TAK Server health
    sapient     Listen/send/validate SAPIENT messages (status, detection, alert, task)
    bridge      Run TAK <-> SAPIENT bridge (bidirectional mapping, correlation, policy)

EXAMPLES:
    # Listen on standard TAK multicast
    rustak listen --udp 239.2.3.1:6969

    # Send a hostile drone track
    rustak send --type "a-h-A-M-F-Q" --lat 51.5 --lon -0.1 --alt 100 \
        --callsign "THREAT-01" --udp 239.2.3.1:6969

    # Connect to TAK Server
    rustak connect --host tak.example.com --port 8089 \
        --cert client.p12 --password atakatak

    # Run a swarm scenario against TAK Server
    rustak sim --scenario scenarios/swarm_attack.yaml \
        --target tak.example.com:8089 --cert client.p12

    # Stress test: 500 simultaneous tracks
    rustak stress --count 500 --duration 300s \
        --target tak.example.com:8089 --cert client.p12

    # Record and replay
    rustak record --source 239.2.3.1:6969 --output session.takrec
    rustak replay --input session.takrec --target 239.2.3.1:6969 --speed 2.0

    # Validate a CoT message
    echo '<event ...>' | rustak validate --format xml

    # Run a bridge: SAPIENT TCP feed -> TAK Server TLS stream
    rustak bridge \
        --sapient 10.0.0.10:19000 \
        --tak tak.example.com:8089 --cert client.p12 --password atakatak \
        --config mapping.yaml
```

---

### 6.13 `rustak-sapient` — SAPIENT Protobuf + TCP Framing

**Purpose:** Implement SAPIENT message encoding/decoding and TCP framing according to BSI Flex 335 (using the upstream Dstl reference `.proto` files). SAPIENT is its own message family (registration, status, detections, alerts, tasking) and does not map 1:1 to CoT.

Key responsibilities:
- Versioned schema modules generated via `prost-build` from pinned upstream `.proto` sources (Apache-2.0)
- TCP framing: 4-byte little-endian length prefix followed by a serialized Protobuf message
- Enforce shared `Limits` bounds for max message/frame size and queue behavior
- Session helpers for registration, acknowledgement, timeouts, reconnect, and TCP options (e.g., `TCP_NODELAY`)
- Conformance fixtures and codec round-trips against known-good messages

API sketch:
```rust
pub mod v2_0 {
    pub use crate::generated::bsi_flex_335_v2_0::*;
}

#[derive(Debug, Clone)]
pub struct SapientConfig {
    pub limits: rustak_limits::Limits,
    pub read_timeout: Duration,
    pub write_timeout: Duration,
    pub tcp_nodelay: bool,
}

pub struct SapientConnection { /* tokio TcpStream + codec */ }

impl SapientConnection {
    pub async fn connect(addr: SocketAddr, cfg: SapientConfig) -> Result<Self, SapientError>;
    pub async fn send(&mut self, msg: &v2_0::SapientMessage) -> Result<(), SapientError>;
    pub async fn recv(&mut self) -> Result<v2_0::SapientMessage, SapientError>;
}
```

---

### 6.14 `rustak-bridge` — TAK <-> SAPIENT Mapping and Bridge Runner

**Purpose:** Provide a configurable, stateful mapping between:
- SAPIENT detections/tracks/alerts/tasks and CoT events suitable for ATAK/WinTAK display and TAK Server ingestion
- CoT-derived tasking or operator actions back into SAPIENT Task messages when required

Key responsibilities:
- Correlate SAPIENT detection IDs/object IDs/node IDs into stable CoT UIDs (policy-driven)
- Map SAPIENT classification and behaviour probabilities into CoT detail extensions (semantic preservation)
- Apply policy for CoT `stale` windows, update cadence, de-duplication, and track promotion/demotion
- Apply explicit time policy (`message_time`, `observed_time`, skew-clamped hybrid) for `time/start/stale`
- Provide mapping tables from SAPIENT taxonomy outputs to CoT types (MIL-STD-2525 strings) and to `How` values
- Handle duplicate and out-of-order messages with explicit windows and deterministic tie-break rules
- Enforce idempotence keys for replay/reconnect safety (at-most-once within policy windows)
- Support multi-sensor correlation strategies with deterministic tie-breakers
- Optional track smoothing for display stability and downstream analytics (policy-configurable)
- Optional persistence for UID mappings and correlation state (restart-stable behavior)
- Policy-driven rate limiting and backpressure behavior to protect TAK Server and clients

Configuration sketch:
```yaml
bridge:
  sapient_version: "bsi_flex_335_v2_0"
  uid_policy: "stable_per_object"        # stable_per_object | stable_per_detection | custom
  cot_stale_seconds: 15
  time_policy: "observed_with_skew_clamp"  # message_time | observed_time | observed_with_skew_clamp
  max_clock_skew_seconds: 5
  correlation:
    cache_ttl_seconds: 600
    persist_path: "/var/lib/rustak/bridge_state.sqlite"   # optional
  dedup:
    window_ms: 500
    key: "object_id+node_id+timestamp"
  smoothing:
    enabled: true
    model: "alpha_beta"                  # alpha_beta | kalman (future)
    alpha: 0.35
    beta: 0.05
  emission:
    max_updates_per_second: 20
    min_separation_ms: 100
    priority:
      alerts: 10
      tracks: 5
      status: 1
  classification_mapping:
    "UAS/Multirotor": "a-h-A-M-F-Q"
    "UAS/FixedWing":  "a-h-A-M-F-Q"
  behaviour_mapping:
    "Loitering": { detail_key: "sapient.behaviour", severity: "warning" }
  validation:
    strict_startup: true
    unknown_class_fallback: "a-u-A-M-F-Q"
```

API sketch:
```rust
pub struct Bridge {
    pub async fn run(&mut self) -> Result<(), BridgeError>;
}

pub struct BridgeConfig { /* validated config */ }
pub struct Correlator { /* object_id<->uid mapping + eviction */ }
pub struct Deduplicator { /* windowed dedup */ }
pub struct Smoother { /* optional smoothing */ }
pub struct Emitter { /* rate limiting, stale window, cadence */ }
```

Bridge correctness gates (release blockers):
- Idempotence under reconnect replay
- Deterministic UID mapping with and without persisted state
- Mapping-table coverage in strict mode before startup

---

### 6.15 `rustak-ffi` — Stable C ABI and Bindings

**Purpose:** Provide a stable FFI boundary for Android (JNI) and Windows (.NET/PInvoke) consumers so they can reuse codecs, framing, recording, and bridge logic without rewriting their ecosystem in Rust.

Key responsibilities:
- Versioned C ABI (opaque handles + byte buffers; no Rust structs exposed)
- Functions for parse/encode/frame/unframe, plus bridge mapping entrypoints
- Examples and build scaffolding for JNI and P/Invoke wrappers (no ATAK/WinTAK SDK code in-tree)
- Explicit ownership rules for returned buffers, plus a single `rustak_free` entrypoint
- Stable error model (numeric codes + optional UTF-8 error message getter; no panics across FFI)
- ABI version negotiation (`rustak_abi_version()` + capability flags)
- Fuzz targets that exercise the FFI surface (bytes in, bytes out) as a hardened boundary

---

### 6.16 `rustak-geo` — Pure-Rust Geodesy Helpers (Optional)

**Purpose:** Pure-Rust geodesy utilities used by simulation and mapping (distance/bearing, great-circle interpolation, bounds helpers). Native dependencies (e.g., PROJ) are optional behind feature flags for environments that want them.

---

## 7. Cross-Cutting Concerns

### 7.1 Logging & Observability

```rust
// All crates use the `tracing` ecosystem for structured logging.
// Users can plug in any subscriber (stdout, file, OpenTelemetry, etc.)
// The default logging policy includes redaction for configured sensitive fields.
// Metrics avoid high-cardinality labels (for example raw UID values).

// Example log output:
// 2026-02-13T14:30:00Z INFO  rustak_transport: connected to TAK Server
//   addr=tak.example.com:8089 tls=mTLS wire_format=tak_v1
// 2026-02-13T14:30:01Z DEBUG rustak_sim: entity update
//   uid=THREAT-01 lat=51.50032 lon=-0.10014 alt=102.3 speed=15.2
// 2026-02-13T14:30:01Z WARN  rustak_transport: send retry
//   attempt=2 delay=2s error="connection reset"
```

### 7.2 Configuration

```yaml
# rustak.yaml — loaded and validated via rustak-config
transport:
  protocol: tls
  host: tak.example.com
  port: 8089
  wire_format: auto                # xml | tak_v1 | auto
  protocol_negotiation: auto       # auto | force_xml | force_tak_v1
  downgrade_policy: fail_closed    # fail_open | fail_closed
  limits:
    max_frame_bytes: 1048576
    max_xml_scan_bytes: 1048576
    max_protobuf_bytes: 1048576
    max_queue_messages: 1024
    max_queue_bytes: 8388608
    max_detail_elements: 512
  read_timeout: 15s
  write_timeout: 15s
  keepalive:
    interval: 10s
    timeout: 3s
  reconnect:
    enabled: true
    max_delay: 60s
    backoff_factor: 2.0
    jitter: 0.2
  send_queue:
    mode: coalesce_latest_by_uid    # fifo | priority | coalesce_latest_by_uid
    max_messages: 1024
    max_bytes: 8388608
  mtu_safety:
    max_udp_payload_bytes: 1200
    drop_oversize: true

crypto:
  provider: ring                   # ring | aws_lc_rs | aws_lc_rs_fips
  revocation: prefer               # off | prefer | require
  server_spki_pin: null            # base64-encoded SHA-256 SPKI hash, optional

certificates:
  ca_cert: /etc/rustak/ca.pem
  client_cert: /etc/rustak/client.pem
  client_key: /etc/rustak/client-key.pem

sapient:
  addr: 10.0.0.10:19000
  version: bsi_flex_335_v2_0
  limits_ref: transport.limits
  read_timeout: 15s
  write_timeout: 15s
  tcp_nodelay: true

bridge:
  enabled: true
  uid_policy: stable_per_object
  cot_stale_seconds: 15
  time_policy: observed_with_skew_clamp
  max_clock_skew_seconds: 5
  validation:
    strict_startup: true
    unknown_class_fallback: a-u-A-M-F-Q
  raw_frame_capture: true

admin:
  enabled: true
  bind: 127.0.0.1:9091
  health_path: /healthz
  metrics_path: /metrics
  reload_path: /reload            # optional live reload trigger

logging:
  level: info
  format: json         # json | pretty | compact
  redact:
    - certificates.client_key
    - certificates.client_cert
    - crypto.server_spki_pin

metrics:
  enabled: true
  export: prometheus   # prometheus | stdout | none
  port: 9090

config:
  validate_on_startup: true
  schema_output: "./target/rustak.schema.json"   # generated schema (optional)
```

Design note:
- Admin endpoints are optional and off by default in library builds; enable via a `rustak-admin` feature in service binaries.

### 7.3 Feature Flags

```toml
# rustak-core/Cargo.toml
[features]
default = ["std"]
std = []
serde = ["dep:serde"]               # Optional derives on core model types

# rustak-crypto/Cargo.toml (sketch)
[features]
default = ["ring"]
ring = []
aws_lc_rs = []
aws_lc_rs_fips = []                 # FIPS-capable configuration (deployment-dependent)

# rustak-cli/Cargo.toml (sketch)
[features]
default = []
config_schema = ["dep:schemars"]    # Generate JSON schema for config/mapping files
```

### 7.4 Safety & Compliance

```
┌─────────────────────────────────────────────────────────────────┐
│                     SAFETY CONSIDERATIONS                       │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  • #![forbid(unsafe_code)] on all public-facing crates          │
│  • #![deny(clippy::all, clippy::pedantic)] across workspace     │
│  • cargo-deny for licence auditing (no GPL in dependency tree)  │
│  • cargo-audit in CI for known vulnerabilities                  │
│  • cargo-vet (or equivalent) to explicitly vet dependency risk  │
│  • cargo-nextest for deterministic parallel test execution       │
│  • coverage reporting via llvm-cov                              │
│  • semver checks for public crate APIs                          │
│  • SBOM generation (CycloneDX or SPDX) as a CI artifact         │
│  • Reproducible builds policy (locked deps; documented toolchain)│
│  • Secrets policy: no key material/passwords in logs;           │
│    explicit redaction + optional zeroization for in-memory keys │
│  • Safe-default limits/timeouts + explicit fail-closed toggles  │
│    for downgrade-sensitive paths                                 │
│  • Fuzz targets for parsing/framing (CoT XML, TAK wire/proto,   │
│    SAPIENT TCP frames + protobuf)                               │
│  • loom tests for transport concurrency invariants              │
│  • MRSOP: No panics in library code; all errors are Results     │
│  • TLS via rustls; crypto provider is explicit (ring or aws-lc)  │
│  • Optional FIPS-capable configuration using rustls fips mode    │
│    and a validated crypto module where available                │
│  • Minimum Supported Rust Version (MSRV) policy documented     │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

---

### 7.5 Workspace Automation (`xtask`)

`xtask` provides one-command workflows to keep contributor and CI behavior aligned:
- `cargo xtask ci` runs fmt, clippy, nextest, doc checks, and schema validation.
- `cargo xtask fuzz-smoke` runs bounded fuzz smoke tests for parser/framing targets.
- `cargo xtask release-check` runs semver + advisory/licence checks and emits release artifacts metadata.

---

## 8. Example: Complete Synthetic C-UAS Test

```rust
use rustak::RustakError;
use rustak_core::*;
use rustak_sim::*;
use rustak_transport::*;
use rustak_record::*;

#[tokio::main]
async fn main() -> Result<(), RustakError> {
    // 1. Connect to TAK Server
    let certs = TakCertStore::from_p12("client.p12", "atakatak")?;
    let conn = connect_tak_server("tak.example.com:8089".parse()?, certs).await?;

    // 2. Set up recording with truth data
    let recorder = CotRecorder::new("test_session.takrec", RecordFormat::Takrec)?;

    // 3. Define a mixed-threat scenario
    let scenario = Scenario {
        name: "CUAS_EVAL_001".into(),
        description: "Mixed threat: 3x multi-rotor + 1x fixed-wing + 2x decoys".into(),
        schema_version: 1,
        seed: Some(42),
        duration: Duration::from_secs(600),
        time_scale: 1.0,
        entities: vec![
            // Primary threats: 3 multi-rotors approaching from the north
            SimEntity {
                uid: Uid::parse("THREAT-MR-01")?,
                callsign: "DRONE-01".into(),
                cot_type: CotType::parse("a-h-A-M-F-Q")?,
                affiliation: Affiliation::Hostile,
                platform: DronePlatform::MultiRotor,
                flight_pattern: FlightPattern::Transit {
                    from: Position { latitude: 51.52, longitude: -0.10, hae: None, ce: None, le: None },
                    to: Position { latitude: 51.50, longitude: -0.10, hae: None, ce: None, le: None },
                    altitude_agl: 80.0,
                    speed: 12.0,
                },
                noise: NoiseModel::radar_track(),
                update_rate: Duration::from_secs(1),
                start_delay: None,
                duration: None,
                detail: Some(DroneDetail {
                    platform_type: Some(DronePlatform::MultiRotor),
                    propulsion: None,
                    rcs: Some(-15.0),  // dBsm
                    max_speed: None,
                    endurance: None,
                    payload: None,
                    threat_level: Some(ThreatLevel::High),
                    detection_source: None,
                    classification_confidence: None,
                }),
            },
            // ... additional entities ...
        ],
        triggers: vec![
            // After 120s, the fixed-wing starts evasive manoeuvres
            ScenarioTrigger::PatternChange {
                entity_uid: Uid::parse("THREAT-FW-01")?,
                at: Duration::from_secs(120),
                new_pattern: FlightPattern::Erratic {
                    bounds: GeoBounds::from_centre(
                        Position { latitude: 51.50, longitude: -0.10, hae: None, ce: None, le: None },
                        1000.0,
                    ),
                    altitude_range: (30.0, 150.0),
                    max_speed: 35.0,
                    direction_change_interval: Duration::from_secs(5),
                },
            },
        ],
    };

    // 4. Run the scenario
    let mut runner = ScenarioRunner::new(scenario);
    runner.add_sink(Box::new(conn));
    runner.add_truth_recorder(Box::new(recorder));

    let report = runner.run().await?;

    // 5. Analyse results
    println!("Scenario complete:");
    println!("  Events sent: {}", report.events_sent);
    println!("  Duration: {:?}", report.actual_duration);
    println!("  Entities simulated: {}", report.entity_count);

    Ok(())
}
```

---

## 9. Development Roadmap

MVP definition (to control scope): a robust, bounded-memory SAPIENT -> TAK bridge that can ingest SAPIENT detections over TCP length-prefixed framing, map them to stable CoT tracks with a documented policy, stream to TAK Server, and record both sides for replayable regression tests. Bidirectional tasking/alert translation and full TAK mesh comms semantics can iterate after MVP interop is proven.

MVP phase gates (must pass before moving on):
- **Gate A:** Framing + negotiation conformance tests passing with bounded-memory guarantees
- **Gate B:** Unified envelope/time semantics (`ObservedTime`/`CotEnvelope`) integrated in transport and record paths
- **Gate C:** End-to-end SAPIENT->TAK bridge replay test deterministic across repeated runs

### Phase 0 — Interop Foundations (Weeks 1-3)
- [ ] Fix TAK Protocol framing details (mesh vs stream) and implement negotiation rules
- [ ] `rustak-limits`: shared `Limits` contract validated at startup and wired into all boundary crates
- [ ] `rustak-io`: envelope/time semantics (`ObservedTime`, `CotEnvelope`) as the common receive contract
- [ ] `rustak-sapient`: schema generation + TCP length-prefix framing + basic session helpers
- [ ] `rustak-bridge`: minimal SAPIENT DetectionReport -> CoT mapping with stable UID policy
- [ ] `rustak-record`: minimal JSONL capture for inbound SAPIENT and outbound CoT (enables early regression tests)
- [ ] Conformance harness scaffolding (fixtures + CI wiring)

### Phase 1 — Model, Codecs, Correct Framing (Weeks 4-7)
- [ ] `rustak-core`: Data model + validation + pass-through extension blobs
- [ ] `rustak-cot`: XML payload encode/decode (single-event; no framing)
- [ ] `rustak-proto`: TAK Protocol v1 payload (protobuf) encode/decode
- [ ] `rustak-wire`: Framing + negotiation state machine (bounded, hardened)
- [ ] `rustak-net`: shared connection/retry/timeouts/generic framed-IO primitives (no protocol semantics)
- [ ] `rustak`: Facade crate (re-exports + unified app-facing error type)
- [ ] Conformance tests (round-trip with real CoT samples)
- [ ] Fuzz targets for XML, TAK wire/proto, and SAPIENT framing

### Phase 2 — Transport, Comms Core, Crypto (Weeks 8-11)
- [ ] `rustak-crypto`: Provider selection (ring/aws-lc), pinning, revocation hooks
- [ ] `rustak-transport`: UDP/TCP/TLS/WebSocket with bounded queues, limits, keepalive, jitter
- [ ] `rustak-commo`: Mesh presence + per-contact version tracking + TakControl cadence
- [ ] Integration tests against a real TAK Server (Docker)
- [ ] Metrics + tracing integration

### Phase 3 — Server, Bridge Ops, CLI, FFI (Weeks 12-15)
- [ ] `rustak-server`: Streaming channel client (management API deferred until after bridge hardening)
- [ ] `rustak-config`: typed config loading/validation/schema/redaction shared by CLI + services
- [ ] `rustak-admin`: optional admin endpoints (`/healthz`, `/metrics`, optional `/reload`)
- [ ] `rustak-cli`: Add `sapient` and `bridge` operations (debuggable bridge as a product)
- [ ] `rustak-ffi`: C ABI for parse/encode/frame/map (JNI/.NET examples)
- [ ] End-to-end negotiation/interoperability tests

### Phase 4 — Simulation & Record/Replay (Weeks 16-19)
- [ ] `rustak-sim`: Determinism contract (schema_version + seed) + terrain-aware AGL
- [ ] `rustak-record`: `takrec` container (versioning, compression, indexing, seek) as an upgrade path from Phase 0 capture
- [ ] Bridge-aware capture and replay (record inbound SAPIENT, outbound TAK, and tasking loops)
- [ ] Basic analysis hooks; advanced reporting lives in `rustak-record-pro`

### Phase 5 — Hardening & Release (Weeks 20-24)
- [ ] Optional FIPS-capable configurations and deployment documentation
- [ ] loom tests for transport concurrency invariants
- [ ] cargo-deny/audit/vet in CI + supply-chain policy
- [ ] nextest + llvm-cov + semver checks integrated in CI via `xtask`
- [ ] SECURITY.md + SemVer stability tiers + MSRV policy
- [ ] Performance benchmarks and optimisation
- [ ] Publish open crates to crates.io

---

## 10. Technology Choices

| Concern                | Choice           | Rationale                                            |
|------------------------|------------------|------------------------------------------------------|
| Async runtime          | `tokio`          | Industry standard, mature, required by rustls        |
| XML parsing            | `quick-xml`      | Fast, zero-copy, streaming support                   |
| Protobuf               | `prost`          | Pure Rust, well-maintained, good codegen             |
| Protobuf well-known types | `prost-types` | Timestamp interoperability for SAPIENT and TAK payload models |
| Wire framing/codecs    | `bytes` + `tokio-util` | Efficient incremental parsing and codecs        |
| Async traits           | `futures`        | Runtime-agnostic `Stream`/`Sink` interfaces          |
| TLS                    | `rustls`         | Provider-selectable TLS (ring/aws-lc), no OpenSSL     |
| Serialisation          | `serde`          | Standard ecosystem, YAML/JSON/TOML support           |
| Logging                | `tracing`        | Structured, async-aware, OpenTelemetry compatible    |
| CLI                    | `clap`           | Derive macros, shell completions, mature             |
| Error handling         | `thiserror`      | Derive macros for clean error enums                  |
| IDs                    | `uuid` + `ulid`  | SAPIENT uses UUID/ULID patterns; bridge benefits from typed IDs |
| Time representation    | `time`           | Lightweight UTC core timestamps with optional chrono interop |
| Geo calculations       | pure Rust by default + optional native | Default is cross-platform and memory-safe; native libs behind feature flags |
| Noise/randomness       | `rand`           | Reproducible RNG with seed support                   |
| Benchmarking           | `criterion`      | Statistical benchmarking                             |
| Fuzzing                | `cargo-fuzz`     | Coverage-guided fuzzing via libfuzzer                 |
| Licence auditing       | `cargo-deny`     | Ensures no licence contamination                     |

---

## 11. Competitive Positioning

| Feature                        | Existing Rust TAK libs | cot_publisher | cottak   | **RusTAK** |
|--------------------------------|------------------------|---------------|----------|------------|
| CoT XML support                | ✅ Basic          | ✅ Basic      | ✅ Good  | ✅ Complete       |
| TAK Protocol v1 support        | ❌ Planned        | ❌            | ✅ Yes   | ✅ Complete       |
| mTLS / certificate management  | ✅ Basic          | ✅ Basic      | ❌       | ✅ Full (FIPS-capable) |
| TAK Server API client          | ❌                | ❌            | ❌       | ✅ Full           |
| Simulation / synthetic tracks  | ❌                | ❌            | ❌       | ✅ Full           |
| Record / replay                | ❌                | ❌            | ❌       | ✅ Full           |
| SAPIENT codec + framing        | ❌                | ❌            | ❌       | ✅ Planned        |
| TAK <-> SAPIENT bridge         | ❌                | ❌            | ❌       | ✅ Planned        |
| Stable FFI boundary            | ❌                | ❌            | ❌       | ✅ Planned        |
| Drone/UAS-specific types       | ❌                | ❌            | ❌       | ✅ Full           |
| CLI tool                       | ❌                | ❌            | ❌       | ✅ Full           |
| Fuzz testing                   | ❌                | ❌            | ❌       | ✅ Yes            |
| Documentation quality          | Basic README      | Good          | Good     | Comprehensive     |
| Defence-grade focus            | ❌                | ❌            | ❌       | ✅ Core mission   |

---

## 12. Security & Governance

- Threat model document (`docs/threat_model.md`) covering malformed input DoS, downgrade attempts, secret leakage, and malicious peer behavior
- Security policy and vulnerability disclosure process (`SECURITY.md`)
- SBOM/provenance requirements documented for releases (CycloneDX/SPDX + reproducible build inputs)
- SemVer policy per crate; stability tiers for APIs (experimental, stable)
- MSRV policy pinned and enforced in CI
- Open-core boundary enforced via a separate private workspace overlay for proprietary crates/packs
