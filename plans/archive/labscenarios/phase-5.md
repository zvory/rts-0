# Phase 5 - Hardening, Docs, and Review Flow

## Phase Status

- [x] Done.

## Objective

Make scenario selection and authoring safe enough for normal use and document the review process.

## Work

- Tighten guardrails from the earlier phases: catalog validation, duplicate slug handling, payload
  size/entity caps, path allowlists, and local export/import bounds.
- Add or update automated tests for the full scenario lifecycle: catalog load, lab launch from
  catalog, authoring validation, local export, and local import.
- Add a small reviewer checklist to local authoring documentation. It should cover scenario name,
  map, player/faction setup, entity count, intended use, and manual lab smoke.
- Document the author workflow: open lab, choose base scenario or blank, edit state, validate,
  export JSON locally, review it, and import it again before manually adding a trusted bundled asset.
- Document the operator workflow and safe-path restrictions.
- Update context capsules if source-of-truth design sections or scenario routing changed.
- Run a local manual smoke of selecting an existing scenario, validating a new scenario, and using a
  local JSON export/import path.
- Verify a manually bundled scenario appears in the catalog and can launch from `/lab`.

## Expected Touch Points

- `docs/design/client-ui.md`
- `docs/design/server-sim.md`
- `docs/design/protocol.md`
- `docs/context/client-ui.md`
- `docs/context/server-sim.md`
- `docs/context/protocol.md`
- `docs/context/deployment.md`
- Focused client/server tests from prior phases
- `plans/labscenarios/*.md`

## Verification

- `cargo test --manifest-path server/Cargo.toml -p rts-server lab`
- `cargo test --manifest-path server/Cargo.toml -p rts-sim lab_scenario`
- `node tests/client_contracts/lab_contracts.mjs`
- `node scripts/check-client-architecture.mjs`
- `node tests/protocol_parity.mjs`
- `git diff --check`

Use exact focused test names if broad filters would match too much or too little.

## Manual Test Focus

Start an existing catalog scenario, start a blank lab, author a new scenario, validate it, export
it locally, and confirm it can be imported again.

## Handoff Expectations

Summarize the completed selection/authoring workflow, exact verification, remaining non-goals,
and any follow-up plan needed for public scenario libraries or
`/dev/scenario` migration.
