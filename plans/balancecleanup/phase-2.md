# Phase 2 - Command Budget And Export Snapshot

Status: done.

## Goal

Create the remaining structured no-drift comparisons that reviewers need before manual
balance/config file splits begin.

## Scope

- Treat the existing Rust rules/catalog `clientConfig` dump as the baseline for stats, bodies,
  resources, upgrades, ability descriptors/effects, faction catalogs, and client-visible scalar
  constants. Do not rebuild that coverage from scratch.
- Add a structured comparison for `BASE_COMMAND_SUPPLY_CAP` and
  `COMMAND_CAR_SUPPLY_CAP_BONUS`, which are currently mirrored between `client/src/config.js` and
  `server/crates/sim/src/game/services/commands.rs` outside the Rust rules dump.
- Prefer a focused sim-owned command budget export or dump over moving command budget constants into
  `rts_rules::balance`. Move them into rules only if `docs/design/balance.md`,
  `docs/design/server-sim.md`, and the implementation all intentionally establish that owner.
- Add a client config export-name snapshot/check that compares stable public export names without
  requiring `client/src/config.js` to remain one physical file.
- Decide explicitly whether source generation is useful. If generation would make the client mirror
  less reviewable or blur Rust/client ownership, keep validation-only checks and document that
  choice.
- Do not split runtime balance/config files in this phase except for the minimal source-of-truth
  movement needed to expose command budget expected values.

## Touch Points

- `server/crates/sim/src/game/services/commands.rs` or a focused sim-owned command budget helper
- a small focused Rust dump/test for command budget values, or `scripts/check-faction-catalog-parity.mjs`
  if the project deliberately folds command budget into an existing parity command
- `tests/client_contracts/config_contracts.mjs`
- `docs/design/balance.md`
- `docs/design/server-sim.md` or `docs/design/hardening.md`, only if command budget ownership
  wording changes
- `client/src/config.js`, only if a missing mirror value needs an exported stable name for checking
- Rust balance/config shims, only if command budget ownership is intentionally clarified without
  numeric drift

## Constraints

- Preserve all numeric values and all public imports.
- Keep client-only presentation fields on the documented exclusion list.
- Do not make a generated client file authoritative. Rust-owned values must still compare against a
  Rust-owned dump or exported Rust constants.
- If command budget ownership cannot be clarified without design input, stop and hand off that
  decision before Phase 3.
- Do not duplicate every config value in a new snapshot. Use the existing structured parity payload
  for values it already covers.

## Verification

- `node scripts/check-faction-catalog-parity.mjs`
- `node tests/client_contracts.mjs`
- Focused `cargo test --manifest-path server/Cargo.toml -p rts-rules` if Rust rules dump code is
  touched
- Focused `cargo test --manifest-path server/Cargo.toml -p rts-sim command_budget` if command
  budget constants or sim command-service exports move
- `node scripts/check-docs-health.mjs` if docs are touched
- `git diff --check`

## Manual Testing Focus

No gameplay manual test is expected. Manually review the structured dump/check output and confirm it
would catch command budget drift and public config export drift before source files split. Existing
parity should continue to catch stats, resources, bodies, abilities, upgrades, faction catalogs, and
client-visible scalar drift.

## Handoff

Mark this phase done only after the no-drift comparison is committed and passing. Summarize the
newly covered command budget/export values, any generation decision, any values intentionally
excluded as client-only presentation, and whether Phase 3 may split `client/src/config.js`.
