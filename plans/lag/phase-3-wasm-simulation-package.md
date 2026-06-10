# Phase 3 - WASM Simulation Package

## Objective

Create a browser-loadable Rust simulation package that can run deterministic local prediction
without pulling in server transport, AI controllers, Axum, Tokio, SQL, or deployment code.

## Rust Work

- Add a new crate such as `server/crates/sim-wasm`.
- Depend on `rts-sim`, `rts-rules`, `rts-protocol`, and `rts-contract` only.
- Expose a narrow browser API:
  - construct from start payload and player id
  - import an owner-safe authoritative baseline
  - enqueue local command with `clientSeq`
  - advance N ticks
  - export predicted render snapshot for the local player
  - export diagnostics: tick, entity counts, pending commands, correction magnitude
- Keep the ordinary `Game` API intact unless a specific import/export seam is needed.
- Fix WASM portability:
  - resolve `rand/getrandom` target support or remove runtime entropy from browser prediction
  - gate any filesystem map loading behind server/native features
  - gate or no-op perf tracing that depends on native-only timing assumptions
  - avoid `rayon`/thread assumptions in browser builds

## State Import Strategy

The current fog-filtered snapshot is a render projection, not full authoritative state. This phase
must choose and document one of these safe baselines:

- `OwnedPredictionBaseline`: owned entities, owned economy/production/order internals, visible
  enemy projections only as non-authoritative obstacles/targets.
- `SnapshotPredictionBaseline`: prediction limited to what can be reconstructed from the existing
  snapshot, with many systems disabled.

Prefer `OwnedPredictionBaseline` if owned movement/order prediction needs internal state that
ordinary snapshots intentionally omit. The baseline must not include hidden enemy state.

## Build and Loading Work

- Add a reproducible build command for the WASM package.
- Decide whether to use `wasm-bindgen`, a minimal JS glue layer, or another explicit browser
  loading path.
- Keep the existing no-JS-build-step development loop unless the team explicitly accepts a WASM
  asset build step.
- Serve the generated WASM asset from the Rust server's static path or an equivalent checked-in
  development location.

## Verification

- `cargo check -p rts-sim-wasm --target wasm32-unknown-unknown`.
- Native-vs-WASM deterministic parity test for:
  - match construction from the same start payload/baseline
  - no-op ticks
  - simple move command
  - queued move commands
  - build command rejected by invalid placement
- Browser smoke test that loads the WASM module and runs 300 local ticks without leaking memory
  beyond a fixed threshold.
- Bundle-size check with an explicit maximum for initial rollout.
- Architecture check that `rts-sim-wasm` does not depend on `rts-server`, `rts-ai`, Tokio, Axum, or
  SQLx.

## Player-Facing Outcome

No visible prediction yet unless a developer flag is enabled. This phase proves the browser can run
the Rust simulation safely and repeatably.
