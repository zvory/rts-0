# Phase 4 - Hardening, Smoke, And Documentation

## Phase Status

- [x] Done.

## Objective

Close the collaborative lab and visible-debug-entry migration with focused guardrails,
source-of-truth docs, and a manual two-user smoke.

## Work

- Review the final collaborative lab contract against `plans/lab/architecture.md`,
  `docs/design/protocol.md`, `docs/design/client-ui.md`, and `docs/design/server-sim.md`.
- Update source-of-truth docs for any stable role, lab state, operation log, start payload, or UI
  affordance changes that previous phases introduced.
- Add or refresh focused tests that prove:
  - later lab joiners can operate when expected;
  - read-only viewers, if still exposed, cannot operate;
  - normal rooms reject lab requests;
  - normal spectators and replay viewers remain passive;
  - the normal lobby no longer exposes the old visible Debug mode workflow;
  - quickstart compatibility remains available only where intentionally preserved.
- Run a manual two-browser lab smoke on a local server:
  - both collaborators join the same room;
  - each collaborator spawns units;
  - each collaborator issues real command-card orders through issue-as;
  - selected-entity setup actions still work;
  - vision switching behaves as documented;
  - scenario export/import still works after collaborative operations.
- Record remaining gaps without expanding this plan's scope: lab presets, lab flags, per-user
  vision, timeline controls, presence/permissions, durable scenario libraries, `/dev/scenario`
  migration, and final quickstart deletion.
- Mark all completed phase docs done in the implementation commit that closes this phase.

## Expected Touch Points

- `docs/design/protocol.md`
- `docs/design/client-ui.md`
- `docs/design/server-sim.md`
- `docs/context/protocol.md`
- `docs/context/client-ui.md`
- `docs/context/server-sim.md`
- `plans/lab/debug-collab/*.md`
- `tests/client_contracts.mjs`
- `tests/hud_command_card.mjs`
- `tests/protocol_parity.mjs`
- `server/src/lobby/room_task.rs` tests if final server guardrails are missing
- `scripts/check-client-architecture.mjs`

## Verification

- `cargo test --manifest-path server/Cargo.toml -p rts-server lab`
- `node tests/client_contracts.mjs`
- `node tests/hud_command_card.mjs`
- `node tests/protocol_parity.mjs`
- `node scripts/check-client-architecture.mjs`
- `node tests/select-suites.mjs --verify`
- `git diff --check`

If the implementation touched quickstart compatibility or live lobby integration, add the narrow
relevant live test and report whether a local server was required.

## Manual Test Focus

Run the two-browser collaborative lab smoke described above. Also open a normal lobby and confirm
the old visible Debug mode path is gone or clearly replaced by lab entry.

## Handoff Expectations

Summarize the final contract in player-facing language, list exact verification and manual smoke
results, name any still-preserved quickstart compatibility, and identify the next recommended plan
for lab presets or final quickstart removal.
