# Phase 2 - Structured Mirror Snapshot

Status: planned.

## Goal

Create a structured no-drift comparison that reviewers can trust before manual balance/config file
splits begin.

## Scope

- Extend the Rust-origin data used by `scripts/check-faction-catalog-parity.mjs`, or add a focused
  supplemental dump, so every Rust-owned client-visible mirror value has a structured expected value.
- Include command budget values in the structured comparison if they remain player-visible mirrors;
  either move them to an already documented Rust owner with stable re-exports or add a focused
  server-owned dump that keeps the sim command service authoritative.
- Add a client export snapshot/check that compares stable public export names and structured values
  without requiring `client/src/config.js` to stay one file.
- Decide explicitly whether source generation is useful. If generation would make the client mirror
  less reviewable or blur Rust/client ownership, keep validation-only checks and document that choice.
- Do not split runtime balance/config files in this phase except for the minimal source-of-truth
  movement needed to expose a missing structured expected value.

## Touch Points

- `server/crates/rules/src/bin/dump-faction-catalog.rs` or a small focused Rust dump binary/test
- `scripts/check-faction-catalog-parity.mjs`
- `tests/client_contracts/config_contracts.mjs`
- `docs/design/balance.md`
- `client/src/config.js`, only if a missing mirror value needs an exported stable name for checking
- `server/crates/sim/src/game/services/commands.rs` and Rust balance/config shims, only if command
  budget ownership is intentionally clarified without numeric drift

## Constraints

- Preserve all numeric values and all public imports.
- Keep client-only presentation fields on the documented exclusion list.
- Do not make a generated client file authoritative. Rust-owned values must still compare against a
  Rust-owned dump or exported Rust constants.
- If command budget ownership cannot be clarified without design input, stop and hand off that
  decision before Phase 3.

## Verification

- `node scripts/check-faction-catalog-parity.mjs`
- `node tests/client_contracts.mjs`
- Focused `cargo test --manifest-path server/Cargo.toml -p rts-rules` if Rust rules dump code is
  touched
- Focused sim command-service tests if command budget constants move
- `node scripts/check-docs-health.mjs` if docs are touched
- `git diff --check`

## Manual Testing Focus

No gameplay manual test is expected. Manually review the structured dump/check output and confirm it
would catch numeric drift in stats, resources, bodies, abilities, upgrades, faction catalogs, and
command budget values before source files split.

## Handoff

Mark this phase done only after the no-drift comparison is committed and passing. Summarize the
newly covered values, any generation decision, any values intentionally excluded as client-only
presentation, and whether Phase 3 may split `client/src/config.js`.
