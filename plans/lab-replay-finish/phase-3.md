# Phase 3 - Replace Lab Scenario Product Surface

Status: Not started.

## Scope

Stop treating "lab scenarios" as the primary product concept. New UX and docs should distinguish
setup checkpoints from lab replays, while old `LabScenarioV1` remains only as a compatibility input.

## Expected Touch Points

- `client/src/lab_panel.js`
- `client/src/lab_scenario_authoring.js`
- `client/src/lab_scenario_submission_flow.js`
- `client/src/lab_catalog.js`
- `server/src/lab_scenarios.rs`
- `server/src/lab_scenario_submission.rs`
- `docs/design/protocol.md`
- `docs/design/server-sim.md`
- `tests/client_contracts/lab_contracts.mjs`

## Requirements

- Rename user-facing labels and docs so new exported setup artifacts are lab checkpoint setups, not
  legacy lab scenarios.
- Introduce lab replay save/open affordances separately from setup checkpoint import/export.
- Keep old `LabScenarioV1` import readable with explicit "legacy scenario" compatibility wording.
- Ensure bundled catalog entries and submission previews use checkpoint-backed setup artifacts.
- Update protocol parity/client contract tests to prevent new UI or docs from extending
  `LabScenarioV1` as the preferred path.

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
