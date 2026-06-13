# Phase 2 - Faction-Aware Rules Catalog

Status: Designed, not implemented.

## Objective

Move tech-tree and kind classification decisions behind faction-aware rules APIs. The existing
faction should remain behaviorally identical, but build/train/research/ability discovery should no
longer assume one global tech tree.

## Scope

- Introduce a faction catalog layer in `rts-rules`.
- Represent the current faction's units, buildings, upgrades, buildables, trainables, and tech
  requirements through that catalog.
- Update simulation command validation to ask catalog APIs for:
  - whether a building can be built by this player/faction
  - which units a building can train
  - which upgrades a building can research
  - which units/buildings satisfy requirements
  - which units can build, gather, or act as production anchors
- Keep legacy helpers where needed, but make them delegate to the default faction or require an
  explicit player/faction where called from gameplay code.
- Add checker pressure against new direct current-tech-tree matches in command validation.

## Expected Touch Points

- `server/crates/rules/src/defs.rs`
- `server/crates/rules/src/economy.rs`
- `server/crates/rules/src/kind.rs`
- `server/crates/sim/src/game/services/commands.rs`
- `server/crates/sim/src/game/services/construction.rs`
- `server/crates/sim/src/game/services/production.rs`
- `server/crates/sim/src/game/services/supply.rs`
- `server/crates/sim/src/game/services/world_query.rs`
- `server/crates/archcheck/` or `scripts/` if a new ratchet is added
- `docs/design/balance.md`
- `docs/design/server-sim.md`

## Verification

- Catalog contract tests proving the current faction catalog exactly matches today's train/build/
  research tables.
- Focused Rust tests for build requirement, train requirement, upgrade requirement, and supply
  reservation behavior.
- Architecture checker or report showing no new forbidden direct checks were introduced.
- Existing focused sim tests touched by command/build/production paths.

## Manual Testing Focus

Run a local normal match and verify the current tech tree: Worker build menu, Barracks units,
Training Centre unlocks, R&D upgrades, Vehicle Works units, and Gun Works units.

## Handoff Expectations

The handoff must describe the catalog API, remaining compatibility helpers, and any hardcoded
current-faction assumptions still allowed after Phase 2. It should tell Phase 3 how to attach
faction-specific economy and start-loadout definitions.

## Player-Facing Outcome

No intended gameplay change. Internally, the current faction becomes one catalog entry instead of
the implicit global tech tree.

