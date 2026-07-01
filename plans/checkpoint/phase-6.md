# Phase 6 - Lab Asset Cutover

Status: Not started.

## Scope

Migrate bundled lab scenario assets and lab export/submission defaults to checkpoint-backed scenario
files. This is the first phase that may intentionally change the on-disk lab scenario format, but it
should do so only after Phase 5 proves side-by-side parity.

Old `LabScenarioV1` files should remain readable during the transition unless the implementation
adds a deliberate, documented rejection policy and updates every catalog/submission caller. Any
conversion script must be deterministic, reviewable, and keep generated noise out of unrelated
files.

Explicit non-goals:

- No replay artifact migration; that belongs to Phase 4.
- No lab timeline action capture unless required to preserve current exported scenario behavior.
- No gameplay/balance changes in scenario contents unless a scenario cannot be represented and the
  change is called out explicitly.
- No removal of compatibility readers before all bundled and persisted use cases are audited.

## Expected Touch Points

- `server/lab_scenarios/**` or the current bundled scenario asset directory: convert assets to the
  checkpoint-backed format.
- `server/src/lab_scenarios.rs`: load, validate, preview, and catalog checkpoint-backed assets.
- `server/src/lab_scenario_submission.rs`: default new submissions to checkpoint-backed scenario
  files and preserve path allowlists.
- `client/src/lab_*`: update only if visible file labels, download names, or validation messages
  need to distinguish old and new formats.
- Conversion script under `scripts/` if useful, with deterministic output and tests.
- Docs/catalog manifest updates for new scenario format.

## Verification

- Every bundled lab scenario loads from the checkpoint-backed format and produces equivalent
  snapshots to its pre-cutover version.
- Old `LabScenarioV1` compatibility fixtures still load or fail with the deliberate policy chosen in
  this phase.
- Scenario submission still rejects path traversal, duplicate ids/slugs, invalid metadata,
  unsupported maps/factions, over-cap entity counts, and malformed checkpoint files.
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
- number and location of converted bundled scenarios;
- old-format compatibility policy;
- validation and submission tests that passed;
- any client-facing copy or behavior changes;
- manual catalog/import/export focus for Phase 7.
