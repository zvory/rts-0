# Protocol Boundary Refactor Plan

## Purpose

Tighten the Rust-to-JS protocol and config mirror boundary so the authoritative Rust crates remain
clear and client mirrors are mechanically checked. `rts-protocol` should own wire DTOs and compact
transport codes, `rts-rules` should own balance/catalog facts, and adapter modules should own
domain conversion.

## Overall Constraints

- No gameplay, balance, or wire-shape changes unless a phase explicitly updates Rust, JS, tests,
  and docs together.
- Preserve `rts-protocol` dependency limits: it may depend on `rts-contract`, not `rts-rules`,
  `rts-sim`, `rts-ai`, or `rts-server`.
- Keep `server/src/protocol.rs`, `server/src/config.rs`, `server/crates/sim/src/protocol.rs`, and
  `server/crates/sim/src/config.rs` as compatibility adapters until call-site migration is isolated.
- Treat `client/src/protocol.js` and `client/src/config.js` as mirrors, not authorities, except for
  explicitly client-only presentation data.
- After each phase, provide a handoff naming verification results, remaining manually mirrored data,
  and the core start/snapshot/command-card behavior that should be manually tested if touched.
- Implement, commit, merge to `main`, and push each phase before starting the next phase.

## Boundary Invariants

- `rts-protocol` owns wire DTO shape, message tags, compact code tables, compact snapshot version,
  prediction protocol version, and unknown/sentinel policy.
- `rts-rules::EntityKind::stable_id()` owns domain identity strings; protocol kind constants must
  equal those stable ids.
- `client/src/protocol.js` mirrors protocol vocabulary and compact decode tables only.
- `client/src/config.js` is mixed: Rust-owned gameplay-visible data plus client-owned rendering,
  camera, colors, and local affordances. Every mirrored field must be classified before enforcement.
- `255` remains reserved as the unknown compact-code sentinel; no real compact code may use it.
- Compact slot order is append-only unless `COMPACT_SNAPSHOT_VERSION` is bumped and Rust DTOs, JS
  decoders, parity tests, and docs change in the same phase.

## Phase Summaries

### [Phase 1 - Boundary Inventory](phase-1.md)

Classify the current mirrored values before changing code. Each value should be labeled as wire DTO,
compact transport code, domain adapter mapping, balance scalar, faction catalog fact, UI-only
presentation data, or server-only constant. This creates the checklist later phases use.

### [Phase 2 - Protocol Adapter Consolidation](phase-2.md)

Remove duplicated entity-kind wire conversion logic by introducing one rules-aware adapter path
usable by server shell and sim without adding lower-crate dependency violations. Public imports
should remain stable through re-exports where practical. This is a mechanical consolidation, not a
protocol shape change.

### [Phase 3 - Structured Protocol Parity Export](phase-3.md)

Add a Rust-owned structured dump for protocol constants and compact codes. Migrate parity checks
away from source-text scraping where practical while keeping JS constants unchanged. The result
should make future drift failures precise and easier to diagnose.

### [Phase 4 - Balance Mirror Parity Expansion](phase-4.md)

Extend structured parity checks to client-visible balance and render/fog data mirrored in
`client/src/config.js`. Rust rules should remain authoritative for costs, supply, sight, body sizes,
durations, ability timing, range, and cooldown data. Client-only labels, icons, colors, and visual
presentation fields must stay explicitly excluded.

### [Phase 5 - Config Shim Cleanup](phase-5.md)

Narrow server and sim config shims to intentional compatibility exports. Move sim-only constants
that should not look like mirrored balance into clearly named sim-local modules. Avoid broad import
churn unless focused tests make it safe.

### [Phase 6 - Docs And Guardrails](phase-6.md)

Update design/context docs and final parity guardrails after the boundary is clearer. Future
implementers should be able to tell which Rust file owns a value and which command proves the JS
mirror agrees. This phase should be documentation and lightweight checks, not new behavior.

## Non-Goals

- Do not generate the whole JS client or add a build step.
- Do not make UI-only labels, icons, colors, or layout constants Rust-authoritative.
- Do not treat command-card descriptors exported by Rust faction/catalog registries as UI-only just
  because they include labels, icons, or hotkeys; those remain Rust-owned catalog facts when present
  in the Rust dump.
- Do not collapse rules-aware domain conversion into `rts-protocol`.
- Do not rewrite every config import in one phase.

## Handoff Rules

Each phase handoff must name every protocol field, compact code, constant, or parity surface touched.
If compact snapshot version changes, say so explicitly; if it does not, say that explicitly too.
