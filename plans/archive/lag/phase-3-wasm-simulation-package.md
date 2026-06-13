# Phase 3 - WASM Simulation Package

Status: Done ahead of harness backfill.

Implemented as `server/crates/sim-wasm`, a `wasm-bindgen` browser facade over a narrow
owner-safe prediction model. The crate depends only on `rts-sim`, `rts-rules`, `rts-protocol`,
`rts-contract`, and serialization/binding support; the boundary checker rejects server, AI, Tokio,
Axum, and SQL dependencies.

Phase 3.5 must still register this predictor as the tri-state local lane and convert the current
smoke/parity checks into artifact-backed scenario coverage.

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

## Tri-State Harness Integration

- Register the native reference or WASM predictor as the Phase 0 harness local lane.
- Keep scenario definitions unchanged: a scenario that previously ran remote/client should be able
  to opt into local-lane comparison by enabling a flag or assertion block.
- Export local-lane state summaries in the same domain shape as remote/client summaries: owned
  entity positions, active and queued order stages, resources when present in the owner-safe
  baseline, pending commands, correction metrics, and disabled reasons.
- Add an explicit unsupported/unknown field policy so the harness distinguishes "not predicted by
  this phase" from "predicted and divergent."

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
- Tri-state scenario proving the local lane can initialize, advance no-op ticks, and diff cleanly
  against remote/client summaries for the supported owner-safe surface.
- Bundle-size check with an explicit maximum for initial rollout.
- Architecture check that `rts-sim-wasm` does not depend on `rts-server`, `rts-ai`, Tokio, Axum, or
  SQLx.

## Manual Testing Focus

Load the client with the developer prediction flag enabled and confirm the WASM package initializes
without blocking normal match start or command execution. Inspect a tri-state artifact with all
three lanes and verify the local lane is present, named, and comparable even if visible prediction
is still disabled.

## Handoff Expectations

At handoff, include the WASM build command, generated artifact location, bundle-size delta, and the
baseline import limitations that Phase 4 must respect. Identify which state fields are parity-tested
and which are intentionally excluded because they are not owner-safe or not yet predicted.

## Phase 3 Handoff

- Build/check command: `cargo check --manifest-path server/Cargo.toml -p rts-sim-wasm --target wasm32-unknown-unknown`.
- Browser asset command: `scripts/build-sim-wasm.sh` after installing `wasm-bindgen-cli` version
  `0.2.123`.
- Generated browser assets: `client/vendor/sim-wasm/rts_sim_wasm.js` and
  `client/vendor/sim-wasm/rts_sim_wasm_bg.wasm`.
- Bundle-size check: `scripts/check-sim-wasm-size.sh`; current raw release artifact is
  `997716` bytes against the initial `1250000` byte ceiling. The generated
  `wasm-bindgen` browser `.wasm` is `514873` bytes.
- Baseline strategy: `OwnedPredictionBaseline`. It imports owned entities, owner economy/supply
  fields, and visible enemy obstacles without enemy ids. Hidden enemy ids, positions, orders,
  target ids, economy, production, fog reconstruction, resource node state, abilities, combat, and
  construction are intentionally excluded or marked unsupported.
- Parity-tested fields: local construction from the same start payload/baseline, no-op ticks,
  simple movement, queued movement order stages, pending command diagnostics, correction magnitude,
  owner-safe baseline export, and invalid build-command unsupported reporting.
  These tests compare the native and WASM-facing JSON facade behavior from the same owner-safe
  baseline, not hidden full-world server state.
- Phase 4 should connect this package to the client only for owned unit movement/order prediction
  first. Any broader prediction must add a new owner-safe baseline field and a test proving it does
  not expose hidden enemy or internal production/economy state.

## Player-Facing Outcome

No visible prediction yet unless a developer flag is enabled. This phase proves the browser can run
the Rust simulation safely and repeatably.
