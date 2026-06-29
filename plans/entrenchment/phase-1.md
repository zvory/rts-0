# Phase 1 - Research, Rules, And Protocol Contract

## Phase Status

Status: not started.

## Objective

Create the shared vocabulary for Entrenchment before gameplay depends on it. This phase should add
the Training Centre research item, authoritative constants, eligibility helpers, and the protocol
contract for trench state, while leaving trench creation and combat bonuses inactive.

## Scope

- Add an `Entrenchment` upgrade researched at the Training Centre.
- Set the upgrade cost to 100 steel and 0 oil, and the research duration to 10 seconds.
- Add rules-owned constants for the 3-second dig-in duration, 1-tile range bonus, 70% direct miss
  chance, and 70% area-damage reduction.
- Add a rules or sim helper that identifies eligible entrenchment infantry: Rifleman,
  MachineGunner, and Worker only for the default faction feature pass.
- Ensure MortarTeam, Ekat, Golem, vehicles, buildings, non-infantry entities, other support weapons,
  and Ekat-faction units are excluded.
- Define the trench snapshot contract for later phases: stable id, world-pixel center, radius or
  footprint/shape data needed by slotting and rendering, and no owner field unless later evidence
  proves it is needed.
- Add compact snapshot metadata and client decode placeholders for the trench list if the chosen
  contract is a new snapshot field.
- Update protocol and balance docs to describe the new upgrade, constants, and planned trench
  snapshot boundary.
- Do not implement automatic trench creation, occupation, slotting, rendering, or combat bonuses in
  this phase.

## Expected Touch Points

- `server/crates/rules/src/balance/upgrades.rs`
- `server/crates/rules/src/faction.rs`
- `server/crates/rules/src/bin/dump-faction-catalog.rs`
- `server/crates/sim/src/game/upgrade.rs`
- `server/crates/contract/src/lib.rs`
- `server/crates/protocol/src/contract_metadata.rs`
- `server/crates/protocol/src/compact_snapshot.rs`
- `server/crates/protocol/src/lib.rs`
- `server/src/protocol.rs`
- `client/src/protocol_constants.js`
- `client/src/protocol_snapshot.js`
- `client/src/config/rules_mirror.js`
- `client/src/config/factions.js`
- `client/src/protocol.js`
- `docs/design/protocol.md`
- `docs/design/balance.md`

## Verification

- Focused Rust tests for upgrade parsing, researchability, cost, duration, and faction catalog
  membership.
- Protocol representative snapshot and compact snapshot tests if a trench snapshot field is added.
- `node tests/protocol_parity.mjs`.
- `node tests/client_contracts/protocol_contracts.mjs`.
- `node scripts/check-faction-catalog-parity.mjs`.
- `node scripts/check-wiki.mjs`.
- `git diff --check`.

## Manual Test Focus

Research Entrenchment from a Training Centre and confirm it appears with the correct cost and time.
Also confirm Ekat or other excluded faction/unit surfaces do not advertise trench creation or
benefits.

## Handoff Expectations

Name the final upgrade id, trench snapshot field name, compact snapshot version bump if any, and
the eligibility helper added for later phases. Call out any protocol or catalog compatibility notes
the Phase 2 agent must preserve.
