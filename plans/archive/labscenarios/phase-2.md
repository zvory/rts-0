# Phase 2 - Authoring Metadata and Validation

## Phase Status

- [x] Done.

## Objective

Make the lab authoring panel produce scenario exports that are valid, reviewable, and catalog
ready without any server-side write path.

## Work

- Add authoring fields for scenario title/name, stable slug, short description, optional tags, and
  intended review notes. Keep field limits explicit and consistent with the server catalog.
- Add a validation/dry-run path that exports from the authoritative lab `Game`, applies the authoring
  metadata, pretty-formats the JSON, and validates it against the same restore/catalog rules used by
  bundled scenarios.
- Show blocking validation errors in the lab panel. Examples: invalid slug, duplicate id, unsupported
  map metadata, too many entities, empty scenario name, malformed tags, or scenario JSON that cannot
  restore.
- Preserve existing browser JSON export/import and download behavior as a local authoring fallback.
- Decide where authoring state lives in the client. It should stay in app-owned lab UI or a small
  injected helper, not in `GameState`.
- Add deterministic JSON formatting so locally exported files are readable.
- If useful, add a generated preview of the files that would be added or changed locally.

## Expected Touch Points

- `client/src/lab_panel.js`
- `client/src/lab_client.js`
- Optional new client helper such as `client/src/lab_scenario_authoring.js`
- `server/src/lobby/room_task/lab.rs`
- `server/src/main.rs` if validation is exposed over HTTP instead of lab request/result
- `server/assets/lab-scenarios/` catalog manifest rules from Phase 1
- `docs/design/client-ui.md`
- `docs/design/protocol.md` if adding a lab op or HTTP DTO
- `tests/client_contracts/lab_contracts.mjs`

## Verification

- Focused client contracts for metadata validation, slug generation, error display, and preserving
  existing export/import behavior.
- Focused server tests for dry-run validation of accepted and rejected scenario metadata.
- `node tests/client_contracts/lab_contracts.mjs`
- `cargo test --manifest-path server/Cargo.toml -p rts-server lab`
- `cargo test --manifest-path server/Cargo.toml -p rts-sim lab_scenario`
- `node tests/protocol_parity.mjs` if protocol changes.
- `git diff --check`

## Manual Test Focus

Create a lab setup, enter valid metadata, run validation, and confirm the panel reports it as ready.
Then try an invalid slug and a duplicate existing scenario id and confirm export remains blocked.

## Handoff Expectations

Summarize the authoring fields, validation rules, exact error behavior, and the catalog-ready local
export shape.
