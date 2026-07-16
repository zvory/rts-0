# Phase 3 - Consolidate Server Contract Ownership

Status: Incomplete.

## Objective

Consolidate two small server contracts that currently have competing owners: typed ability/upgrade
identities and command unit-list limits. Keep rules as the typed gameplay authority, protocol as an
intentional wire-vocabulary mirror, and one dependency-safe contract module as the owner of the
ordinary and Lab-bypass caps.

## Work

### Typed rule identities

- Make `rts-rules` own `AbilityKind`, `UpgradeKind`, their stable string ids, and ability target
  mode adjacent to the faction catalog rows they identify.
- Make lookup from a valid typed kind to its catalog definition total. Typed callers must not
  receive `Option`/`Result` or depend on `unwrap`, `expect`, or `unreachable` for catalog coverage.
- Keep conversion from untrusted raw strings fallible. Unknown ability or upgrade ids from live
  commands, replay/Lab artifacts, or checkpoints must remain normal validation errors/no-ops at the
  appropriate boundary.
- Remove the competing exhaustive typed identity and raw-string registries from `rts-sim`, while
  retaining thin compatibility re-exports where they materially reduce caller churn. Keep
  simulation-only planner codes, effect hooks, order execution, and dispatch in `rts-sim`.
- Retain the ability and upgrade constants/code tables in `rts-protocol` as an intentional wire
  mirror: crate direction does not permit protocol to depend on rules. Add explicit parity evidence
  that every rules-owned stable id agrees with the protocol vocabulary and compact-code coverage.
- Add focused tests proving stable-id uniqueness and round trips, fallible unknown-string parsing,
  one total catalog row for every typed kind, and total simulation handling where required.
- Update `docs/design/server-sim.md` and `docs/design/protocol.md` to describe the typed owner, the
  protocol mirror, and their parity boundary.

### Command unit-list limits

- Move the ordinary `256` and Lab-bypass `4,096` raw submitted-id caps to `rts-contract`, which is
  already dependency-safe for both `rts-protocol` and `rts-sim`. Consume those constants from live
  simulation admission, Lab replay/artifact validation, and checkpoint validation; remove local
  numeric definitions and aliases that can drift independently.
- Intentionally change live admission to reject the entire command when the raw submitted unit list
  is cap plus one or larger, before deduplication. Accept a raw list exactly at the selected cap,
  then preserve first-seen deduplication for accepted lists.
- Preserve Lab artifact validation's whole-command rejection and make live/Lab behavior agree at
  the same raw boundary. Checkpoint restore must enforce the shared `4,096` persisted-command bound
  rather than a third local value.
- Preserve both numeric caps, the ordinary-versus-`ignoreCommandLimits` selection, the existing
  human command-supply budget, AI and Lab budget exemptions, per-tick scheduling, payloads, and
  command-log shapes.
- Add focused ordinary and Lab-bypass tests at exactly cap and raw cap plus one. Prove that cap is
  accepted, cap plus one is rejected as a whole rather than truncated or partially applied, and
  duplicate ids within an accepted list still dedupe without changing budget behavior. Add focused
  checkpoint boundary coverage for the shared persisted-command cap.
- Update `docs/design/hardening.md` to name the shared cap owner and describe raw-list rejection
  before deduplication.

## Non-goals

- Do not change balance values, ability effects, research availability, catalog membership, wire
  strings/codes, replay or checkpoint formats, fog, or client mirrors.
- Do not change the `256`/`4,096` values, command-supply budget scalars, queue limits, scheduling, or
  Lab authority semantics.
- Do not consolidate AI profile registries, create a generalized registry/limits platform, or add a
  broad compatibility layer.
- Do not change replay or Lab reconstruction; Phase 4 owns failure-atomic reconstruction.

## Likely Touch Points

- `server/crates/rules/src/faction.rs` and a small adjacent rules module if useful
- `server/crates/sim/src/game/ability.rs`
- `server/crates/sim/src/game/upgrade.rs`
- `server/crates/protocol/src/contract_metadata.rs`
- `server/crates/contract/src/lib.rs`
- `server/crates/sim/src/game/services/commands/`
- `server/crates/protocol/src/lab_replay.rs`
- `server/crates/sim/src/game/checkpoint/`
- focused `rts-rules`, `rts-protocol`, and `rts-sim` tests
- `docs/design/server-sim.md`
- `docs/design/protocol.md`
- `docs/design/hardening.md`

## Verification

- Focused Rust tests for typed stable-id uniqueness/round trips, unknown-string parsing, total
  catalog lookup, and total simulation handling.
- Focused Rust tests for live ordinary and Lab-bypass admission at cap/cap+1, Lab artifact
  validation at cap/cap+1, checkpoint validation at cap/cap+1, whole-command rejection, accepted
  deduplication, and unchanged command-budget behavior.
- `cargo nextest run --config-file .config/nextest.toml --manifest-path server/Cargo.toml --profile default -p rts-contract -p rts-rules -p rts-protocol -p rts-sim`
- `node tests/protocol_parity.mjs`
- `node scripts/check-wiki.mjs`
- `node scripts/check-faction-catalog-parity.mjs`
- `node scripts/check-crate-boundaries.mjs`
- `cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture`
- `git diff --check`

## Manual Test Focus

In one local session, confirm representative research plus one self-targeted and one world-targeted
ability behave as before. Spot-check an ordinary command and a Lab `issueCommandAs` command at their
documented limits, confirming oversized raw lists are rejected as a whole rather than trimmed.

## Handoff

Mark this phase done in its implementation commit. Report the final typed and cap owners, retained
protocol/compatibility mirrors, parity evidence, removed panic-on-drift path, and cap/cap+1 results
for all three consumers. Tell the Phase 4 agent that command validation and typed rule identities
are stable and reconstruction must preserve those semantics.
