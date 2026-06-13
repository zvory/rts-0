# Phase 13 - End-to-End Hardening and Release Audit

Status: planned.

## Goal

Finish teams as a robust end-to-end feature with automated coverage as the main confidence source.
Manual testing should be a short sanity check, not the primary way to validate team games.

## Scope

- Finalize `tests/team_integration.mjs` as the canonical multi-client team suite.
- Add or update regression tests for malicious team/lobby/combat/fog inputs.
- Audit raw owner comparisons and document intentional own-control checks.
- Audit compact snapshot encoding/decoding and protocol parity for all new fields.
- Audit docs and context capsules so future agents know where team relationships live.
- Ensure selector rules include the right team tests for server, client, protocol, AI, replay, and
  map changes.
- Add a scriptable dev scenario or test-only endpoint if needed for visual team checks without
  manual room setup.
- Remove any temporary gates or TODOs that were only meant to protect intermediate phases.

## Expected Touch Points

- `docs/context/*.md`
- `docs/design/*.md`
- `tests/team_integration.mjs`
- `tests/regression.mjs`
- `tests/server_integration.mjs`
- `tests/client_contracts.mjs`
- `tests/client_smoke.mjs`
- `tests/select-suites.mjs`
- `tests/run-all.sh`
- `scripts/check-client-architecture.mjs` if client module boundaries changed
- `server/crates/archcheck/` baseline only if architectural growth is intentional and justified

## Verification

Run focused suites first, then the final broad pass:

```bash
node tests/team_integration.mjs
node tests/regression.mjs
node tests/client_contracts.mjs
node tests/select-suites.mjs --verify
cd server && cargo fmt && cargo clippy && cargo test
cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture
node tests/server_integration.mjs
node tests/ai_integration.mjs
node tests/sim_wasm_smoke.mjs
tests/run-all.sh --no-rust
```

If `tests/run-all.sh` includes the team suite by this phase, run it as the final live-server/client
wrapper instead of duplicating equivalent Node commands. Do not use ad hoc `npm install`; rely on the
repo's shared Node dependency flow.

Required end-to-end scenarios:

- Solo sandbox still starts and does not resolve to a winner.
- FFA remains default and reports singleton teams.
- 1v2, 1v3, and 2v2 start from scripted lobby setup and UI-backed setup.
- Team victory resolves correctly for all supported team shapes.
- Allies share vision and see allied support-fire markers.
- Allied units are inspectable, not commandable, and not attackable from normal UI.
- Non-host and malicious clients cannot mutate teams or attack allies.
- Replays and match history preserve team ids and winner team.

## Acceptance Criteria

- Team games work end to end for supported short-run presets.
- Automated tests cover lobby setup, combat safety, fog/projection privacy, client command safety,
  AI safety, replay preservation, and score outcomes.
- Manual testing burden is reduced to one short browser pass using scripted setup.
- Documentation describes the final team relationship model and testing workflow.

## Manual Testing Focus

Use scripted setup, not hand-built multi-tab rooms, to check:

- lobby preset controls
- allied single-click inspection
- right-click allied unit behavior
- shared mortar/artillery markers
- score screen team column and winning-team highlight

## Handoff Requirements

The final handoff must summarize the automated coverage, list any manual checks performed, name any
known follow-up work, and explain the player-facing gameplay impact.
