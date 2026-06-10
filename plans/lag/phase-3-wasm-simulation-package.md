# Phase 3 - WASM Simulation Package

## Objective

Create a browser-loadable Rust simulation package that can run deterministic local prediction
without pulling in server transport, AI controllers, Axum, Tokio, SQL, or deployment code.

## Rust Work

- Add a new crate such as `server/crates/sim-wasm`.
- Depend on `rts-sim`, `rts-rules`, `rts-protocol`, and `rts-contract` only.
- Decide whether `sim-wasm` is a thin wrapper over feature-gated `rts-sim` or a narrower
  prediction facade. The default should be the narrowest API that can predict the enabled surface;
  browser code must not accidentally gain access to full-world snapshots, dev-watch helpers,
  server replay persistence, AI, transport, or database code.
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
  - gate `rayon`/thread assumptions out of browser builds or prove they are absent from the
    exported prediction path

## State Import Strategy

The current fog-filtered snapshot is a render projection, not full authoritative state. This phase
must choose and document one of these safe baselines:

- `OwnedPredictionBaseline`: owned entities, owned economy/production/order internals, visible
  enemy projections only as non-authoritative obstacles/targets.
- `SnapshotPredictionBaseline`: prediction limited to what can be reconstructed from the existing
  snapshot, with many systems disabled.

Default to `OwnedPredictionBaseline` for movement/order prediction. Use `SnapshotPredictionBaseline`
only as an explicit fallback with reduced claims about what can be predicted. Parity tests must
compare native and WASM behavior from the chosen owner-safe baseline, not against a full
authoritative server world that contains hidden entities or internal state the browser must never
receive. The baseline must not include hidden enemy ids, positions, orders, target ids, economy, or
production state.

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
- Baseline export/import tests that prove hidden enemy ids, positions, orders, target ids,
  production state, and economy state are absent while owned movement/order state is sufficient for
  the enabled prediction surface.
- Browser smoke test that loads the WASM module and runs 300 local ticks without leaking memory
  beyond a fixed threshold.
- Bundle-size check with an explicit maximum for initial rollout.
- Architecture check that `rts-sim-wasm` does not depend on `rts-server`, `rts-ai`, Tokio, Axum, or
  SQLx.

## Player-Facing Outcome

No visible prediction yet unless a developer flag is enabled. This phase proves the browser can run
the Rust simulation safely and repeatably.
