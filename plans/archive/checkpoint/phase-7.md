# Phase 7 - Public Surface Cleanup And Release Audit

Status: Done.

## Implementation Notes

- This checkpoint phase did not remove active compatibility readers. The later lab replay finish
  cleanup removed legacy lab setup loading and replay schema 2 loading; current builds reject both
  with explicit compatibility errors.
- `server/crates/archcheck` now fails when a `GameState` or `DerivedState` registry row lacks a
  concrete checkpoint policy or evidence/notes cell, in addition to the existing missing/stale row
  and owner-category checks.
- The release audit in `docs/design/server-sim.md` records the final public surface policy:
  checkpoint payloads remain embeddable JSON text only, maps stay container-supplied map data or map
  asset references, schema 3 replays and checkpoint-backed lab setups are the defaults, and there
  is still no generic checkpoint upload/download product surface.
- Remaining follow-up plan needs are product decisions, not blockers for this checkpoint plan:
  whether to replace replay/lab keyframes with checkpoint keyframes and whether to design a public
  checkpoint save/load surface with rollout observability.

## Scope

Clean up obsolete setup, replay, and lab scenario paths after checkpoint-backed starts are proven as
the default. Tighten docs, guardrails, tests, compatibility messages, and operational notes so
future authoritative state must participate in the embeddable checkpoint payload contract.

This phase is primarily hardening and release audit. It should not expand product scope; if the
audit finds a missing migration or coverage gap too large for a small fix, record it as a follow-up
blocker instead of hiding it in cleanup.

Explicit non-goals:

- No new gameplay, balance, or UI feature work.
- No broad refactor unrelated to obsolete checkpoint/start/replay/lab paths.
- No deletion of compatibility code that still has a documented active caller.
- No direct `main` bypass.

## Expected Touch Points

- `docs/design/server-sim.md`, `docs/context/server-sim.md`, and any replay/lab design docs touched
  by earlier phases.
- `server/crates/archcheck` or related scripts if new guardrails are needed for checkpoint DTO
  coverage, schema version updates, hidden state owners, or map/checkpoint container separation.
- `server/crates/sim/src/game/**`, `server/src/lobby/**`, `server/src/lab_scenarios.rs`, and
  replay/lab compatibility modules: remove obsolete paths only where tests prove replacement.
- `plans/checkpoint/*`: mark completed phases done as each implementation lands and record any
  remaining follow-up plan needs.
- Focused release/audit tests for checkpoint import/export, normal starts, replay launch/seek, lab
  catalog import/export, and projection privacy.

## Verification

- Public `Game` APIs, replay launch, lab launch, and normal match launch all use checkpoint-backed
  starts where intended.
- Old compatibility paths either remain tested or fail with clear, documented messages.
- Architecture/docs checks fail when a new `GameState` field is added without checkpoint DTO policy
  or registry coverage.
- Architecture/docs checks or release-audit notes make clear that `GameCheckpointV1` is an
  embeddable payload, not a product-specific file format, and that maps remain normal map data
  supplied or referenced by outer artifacts.
- Replay and lab compatibility policies cover file/dev artifacts, crash artifacts, match-history DB
  rows, bundled scenarios, locally exported scenarios, and old-format fixtures.
- Projection privacy tests still cover checkpoint import/export boundaries for player, spectator,
  selected-player, and full-world diagnostic views.
- Suggested focused commands:

```bash
cargo fmt --manifest-path server/Cargo.toml
cargo test --manifest-path server/Cargo.toml -p rts-sim checkpoint
cargo test --manifest-path server/Cargo.toml -p rts-sim replay
cargo test --manifest-path server/Cargo.toml -p rts-sim lab
cargo test --manifest-path server/Cargo.toml -p rts-archcheck
cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture
node scripts/check-crate-boundaries.mjs
node scripts/check-docs-health.mjs
git diff --check -- server client docs scripts plans/checkpoint
```

Run broader local suites only if cleanup touches live server/client behavior beyond narrow replay
or lab paths; otherwise rely on the PR `./tests/run-all.sh` gate.

## Manual Testing Focus

Run one ordinary local match start, one replay capture and seek, and one lab catalog import/export
flow. Confirm there is no visible gameplay drift, old incompatible files produce useful errors, and
the checkpoint-backed paths are the defaults.

## Handoff

The handoff must name:

- obsolete paths removed or deliberately retained;
- final compatibility policy for checkpoint payloads, replay artifacts, lab scenarios, and debug
  documents;
- final map binding/container policy for match starts, replays, scenarios, and any debug wrappers;
- guardrails added or tightened;
- release-audit result for gameplay parity, replay, lab, privacy, persistence, and rollback;
- exact verification commands that passed;
- any remaining follow-up plan that should be created.
