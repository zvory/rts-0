# Phase 6 - Lab Asset Cutover

Status: Done.

## Scope

Migrate bundled lab scenario assets and lab export/submission defaults to checkpoint-backed scenario
containers. This is the first phase that may intentionally change the lab scenario JSON shape, but
it should do so only after Phase 5 proves side-by-side parity.

Old `LabScenarioV1` files should remain readable during the transition unless the implementation
adds a deliberate, documented rejection policy and updates every catalog/submission caller. Any
conversion script must be deterministic, reviewable, and keep generated noise out of unrelated
files.

Treat checkpoint-backed lab scenario JSON as a public, untrusted import/export contract. The
existing lab scenario import field lets users paste JSON that the client sends to the server; after
this cutover, that path may carry an embedded `GameCheckpointV1`. That is still not a generic
live-match checkpoint upload endpoint, but the server must validate the scenario container, embedded
payload, map binding, entity/player counts, byte limits, path allowlists, and authoring metadata
before constructing a live `Game`.

Explicit non-goals:

- No replay artifact migration; that belongs to Phase 4.
- No lab timeline action capture unless required to preserve current exported scenario behavior.
- No gameplay/balance changes in scenario contents unless a scenario cannot be represented and the
  change is called out explicitly.
- No removal of compatibility readers before all bundled and persisted use cases are audited.
- No map-as-checkpoint container. Scenario assets may include normal map data or a stable map
  binding beside the checkpoint payload, but bundled map assets remain map assets.
- No generic restore-any-game checkpoint upload. Checkpoint-backed scenario import/export remains
  constrained to the lab scenario protocol, lab room permissions, and scenario validation policy.

## Expected Touch Points

- `server/lab_scenarios/**` or the current bundled scenario asset directory: convert assets to the
  checkpoint-backed format.
- `server/src/lab_scenarios.rs`: load, validate, preview, and catalog checkpoint-backed assets.
- `server/src/lab_scenario_submission.rs`: default new submissions to checkpoint-backed scenario
  files and preserve path allowlists.
- `server/src/lobby/room_task/lab.rs` and lab import/export handlers: update scenario import,
  export, validation, and result mapping only through the existing lab scenario surface; preserve
  operator permissions, id-remap behavior, room dirty/timeline semantics, and user-facing validation
  errors.
- `client/src/lab_*`: update only if visible file labels, download names, or validation messages
  need to distinguish old and new formats.
- `server/crates/protocol/src/lib.rs`, `server/src/protocol.rs`, `client/src/protocol.js`, and
  `docs/design/protocol.md`: update if the scenario import/export message shape or DTO changes.
- Conversion script under `scripts/` if useful, with deterministic output and tests.
- Docs/catalog manifest updates for new scenario format.

## Verification

- Every bundled lab scenario loads from the checkpoint-backed format and produces equivalent
  snapshots to its pre-cutover version.
- Old `LabScenarioV1` compatibility fixtures still load or fail with the deliberate policy chosen in
  this phase.
- Scenario submission still rejects path traversal, duplicate ids/slugs, invalid metadata,
  unsupported maps/factions, over-cap entity counts, and malformed checkpoint payloads.
- Scenario import/export still validates map identity/hash before restore, rejects oversized
  scenario containers and embedded payloads, preserves entity id-remap responses expected by current
  import callers, and prevents scenarios from smuggling unrelated game state through map assets or
  unchecked checkpoint fields.
- If the scenario DTO or protocol-visible JSON changes, run `node tests/protocol_parity.mjs`.
- Suggested focused commands:

```bash
cargo fmt --manifest-path server/Cargo.toml
cargo test --manifest-path server/Cargo.toml -p rts-sim lab
cargo test --manifest-path server/Cargo.toml -p rts-server lab_scenario
node scripts/check-docs-health.mjs
node scripts/check-crate-boundaries.mjs
git diff --check -- server client scripts docs plans/checkpoint
```

If asset conversion touches many JSON files, also run the narrowest catalog loader or validation
script that checks every bundled scenario.

## Manual Testing Focus

Open the lab scenario catalog, launch several migrated scenarios, export one, import it again, and
verify the same visible lab state and controls. Also verify a malformed or old-format scenario
shows the intended compatibility or validation message.

## Handoff

The handoff must name:

- asset format chosen and conversion method;
- how map data/binding and embedded `GameCheckpointV1` are represented without turning maps into
  checkpoint containers;
- number and location of converted bundled scenarios;
- old-format compatibility policy;
- public lab scenario import/export hardening added for checkpoint-backed JSON, including byte/count
  caps, map binding checks, path allowlists, id-remap compatibility, and malformed-payload failures;
- validation and submission tests that passed;
- any client-facing copy or behavior changes;
- manual catalog/import/export focus for Phase 7.
