# Phase 6 - Share Command Unit-List Limits

Status: Incomplete.

## Objective

Give the ordinary and Lab-bypass command unit-list caps one dependency-safe owner. Preserve the
current values, deduplication behavior, whole-command rejection semantics, and runtime/Lab boundary
behavior.

## Work

- Move the ordinary `256` and Lab-bypass `4,096` unit-list caps to one existing dependency-safe
  contract location.
- Consume those constants from both simulation runtime command admission and Lab replay artifact
  validation.
- Remove the duplicate local numeric definitions and avoid introducing aliases that can drift
  independently.
- Add focused tests at the accepted and rejected boundaries for ordinary and Lab-bypass commands.
- Add a small ownership check or source-level assertion only if needed to prevent the two production
  paths from reintroducing local copies.
- Update the hardening design source of truth to name the shared owner.

## Non-goals

- Do not change command budgets, per-tick scheduling, command payloads, artifact formats, or the cap
  values themselves.
- Do not change ability/upgrade ownership completed in Phase 5.
- Do not change replay reconstruction; Phase 7 owns that behavior.
- Do not create a general limits registry.

## Likely Touch Points

- `server/crates/contract/src/lib.rs` or another existing dependency-safe contract module
- `server/crates/sim/src/game/services/commands.rs`
- `server/crates/protocol/src/lab_replay.rs`
- focused Rust tests near runtime admission and replay validation
- `docs/design/hardening.md`

## Verification

- Focused Rust tests for ordinary and Lab accepted/rejected boundaries.
- Focused evidence that both production consumers import the same constants.
- `cargo nextest run --config-file .config/nextest.toml --manifest-path server/Cargo.toml --profile default -p rts-contract -p rts-protocol -p rts-sim`
- `node tests/protocol_parity.mjs`
- `node scripts/check-crate-boundaries.mjs`
- `git diff --check`

## Manual Test Focus

Spot-check an ordinary command and a Lab issue-as command at their documented boundaries. Confirm an
oversized command is still rejected as a whole rather than partially applied.

## Handoff

Mark this phase done in its implementation commit. Report the final constants and owner, both
production consumers, and boundary-test results. Tell the Phase 7 agent that it must preserve these
validation semantics while rebuilding replay or Lab state.
