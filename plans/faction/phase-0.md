# Phase 0 - Architecture Inventory and Harness

Status: Designed, not implemented.

## Objective

Create the safety rails before changing faction behavior. This phase should inventory the current
single-faction assumptions and turn the highest-risk assumptions into focused tests, reports, or
architecture checks. It should leave gameplay unchanged.

## Scope

- Add a faction-architecture inventory document or generated report that lists current hardcoded
  assumptions around:
  - pre-alpha compatibility boundaries that may be intentionally broken
  - `EntityKind` identity and protocol kind codes
  - steel/oil/supply resource fields
  - compact snapshot resource encoding and replay artifact resource schema
  - Worker-only build/gather paths
  - City Centre plus Worker starting loadout
  - current building tech tree and upgrade tree
  - ability ids, carriers, cooldown projection, and special-case execution
  - client command-card build/train/research/ability descriptors
  - AI assumptions around steel/oil saturation, workers, barracks, factories, tanks, and expansions
  - prediction/WASM assumptions around start payloads, snapshots, commands, and current resources
  - replay, branch, dev scenario, spectator, quickstart, match-history, and post-match replay flows
- Add or extend focused tests that lock current behavior before refactors begin.
- Add an initial faction lifecycle matrix that lists every match creation, playback, replay branch,
  spectator, dev scenario, self-play, quickstart/debug, AI, prediction, and match-history path that
  later phases must keep updated.
- Add architecture-check coverage or a script/report that can flag new direct special cases outside
  approved modules.
- Document the approved places for temporary current-faction compatibility shims while the
  migration is in progress.
- Document that old replay/protocol compatibility is intentionally not preserved during this
  pre-alpha refactor.

## Expected Touch Points

- `docs/design/architecture.md`
- `docs/design/server-sim.md`
- `docs/design/protocol.md`
- `docs/design/balance.md`
- `scripts/` for an optional faction-assumption checker
- `tests/` for JS contract/parity coverage
- focused Rust tests under `server/crates/rules`, `server/crates/sim`, and `server/crates/protocol`

## Verification

- Run the new architecture/inventory checker, if added.
- Run focused Rust tests for rules/catalog and current start loadout behavior.
- Run `node tests/protocol_parity.mjs` if protocol constants are inventoried or asserted.
- Run command-card descriptor tests if the inventory touches client command-card assumptions.
- Run or document the existing prediction/WASM smoke coverage if the inventory touches prediction
  assumptions.
- Do not run broad bundles unless this phase adds a shared checker used by broad suite selection.

## Manual Testing Focus

No gameplay manual testing should be required. If a human reviews anything, it should be the
generated inventory/report and whether it identifies the right files for future refactors.

## Handoff Expectations

The handoff must list the inventory artifacts, lifecycle matrix, new tests/checkers, and the most
important current gameplay assumptions that Phase 1 must avoid breaking. It should also name any
special cases that were found but not yet covered by tests, and any old replay/protocol surfaces
that are safe to break under the approved no-backcompat policy.

## Player-Facing Outcome

No gameplay change. This phase makes current behavior explicit and regression-testable before
faction identity is introduced.
