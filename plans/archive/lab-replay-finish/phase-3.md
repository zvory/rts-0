# Phase 3 - Replace Lab Scenario Product Surface

Status: Done.

## Scope

Stop treating "lab scenarios" as the primary product concept. New UX and docs should distinguish
setup checkpoints from lab replays while legacy setup compatibility waits for Phase 4 cleanup.

## Expected Touch Points

- `client/src/lab_panel.js`
- `client/src/lab_scenario_authoring.js`
- `client/src/lab_scenario_authoring_flow.js`
- `client/src/lab_catalog.js`
- `server/src/lab_scenarios.rs`
- `docs/design/protocol.md`
- `docs/design/server-sim.md`
- `tests/client_contracts/lab_contracts.mjs`

## Requirements

- Rename user-facing labels and docs so new exported setup artifacts are lab checkpoint setups, not
  legacy lab scenarios.
- Introduce lab replay save/open affordances separately from setup checkpoint import/export.
- Audit the current `exportScenario`/`importScenario` wire and client helper names. Keep them only as
  compatibility/setup internals with clear comments, or add new lab replay-specific helpers so new
  save/open behavior does not extend the legacy scenario vocabulary.
- Keep old legacy setup imports readable during this phase with explicit compatibility wording.
- Ensure bundled catalog entries and authoring previews use checkpoint-backed setup artifacts.
- Update protocol parity/client contract tests to prevent new UI or docs from extending legacy
  setup DTOs as the preferred path.
- Add client contract coverage for the lab replay save/open labels and for the setup-vs-replay
  distinction in import/export controls.

## Out Of Scope

- Do not remove the legacy reader in this phase.
- Do not remove replay schema 2 in this phase.
- Do not rename low-level Rust DTOs if doing so would create churn without improving the product
  surface. Prefer compatibility aliases where needed.

## Verification

- `node tests/client_contracts/lab_contracts.mjs`
- `node tests/protocol_parity.mjs`
- `cargo test --manifest-path server/Cargo.toml -p rts-server lab_scenario`
- Focused live lab import/export test if UI behavior changes
- `git diff --check`

## Manual Testing Focus

Open the lab UI and verify the new wording makes it clear when the user is working with a setup
checkpoint versus a replayed lab session.

## Handoff Notes

List any remaining "scenario" names that are intentionally kept as compatibility internals and any
that should be removed in Phase 4.
