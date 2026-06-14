# Phase 2 - Faction-Aware Rules Catalog

Status: Done.

## Objective

Move tech-tree and kind classification decisions behind faction-aware rules APIs. The existing
faction should remain behaviorally identical, but build/train/research/ability discovery should no
longer assume one global tech tree.

## Scope

- Introduce a faction catalog layer in `rts-rules`.
- Represent the current faction's units, buildings, upgrades, buildables, trainables, and tech
  requirements through that catalog.
- Keep runtime/wire identity global for now: catalogs refer to global `EntityKind`, upgrade ids,
  ability ids, and Steel/Oil/Supply cost fields rather than introducing faction-scoped kind ids.
- Document the rule for shared global ids: reuse an `EntityKind`, upgrade id, or ability id across
  factions only when its gameplay semantics are identical across every faction that can use it.
  Divergent behavior requires a distinct global id gated by faction catalog availability.
- Establish the generated or mechanically checked JS mirror path for faction catalog data. This is
  a completion gate: the phase must add a named command or test that fails when client descriptors
  drift from the Rust-authoritative catalog.
- Update simulation command validation to ask catalog APIs for:
  - whether a building can be built by this player/faction
  - which units a building can train
  - which upgrades a building can research
  - which units/buildings satisfy requirements
  - which units can build, gather, or act as production anchors
- Reject out-of-faction build/train/research commands on the server even when the referenced global
  kind exists.
- Keep legacy helpers where needed, but make them delegate to the default faction or require an
  explicit player/faction where called from gameplay code.
- Add checker pressure against new direct current-tech-tree matches in command validation.
- Keep AI behavior current-faction-only. Do not make AI code faction-generic in this phase except
  where needed to keep current-faction AI compiling.

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
- Generated-client-catalog or JS parity tests proving the client mirror for the current faction
  matches Rust catalog data. The handoff must include the exact command.
- Focused Rust tests for build requirement, train requirement, upgrade requirement, and supply
  reservation behavior.
- Rust command tests proving out-of-faction catalog entries are rejected for a fixture faction or
  a deliberately illegal player/faction pairing.
- Architecture checker or report showing no new forbidden direct checks were introduced.
- Existing focused sim tests touched by command/build/production paths.

## Manual Testing Focus

Run a local normal match and verify the current tech tree: Worker build menu, Barracks units,
Training Centre unlocks, R&D upgrades, Vehicle Works units, and Gun Works units.

## Handoff Expectations

The handoff must describe the catalog API, the generated/mechanically checked client mirror path and
its exact verification command, remaining compatibility helpers, the shared-global-id rule, and any
hardcoded current-faction assumptions still allowed after Phase 2. It should tell Phase 3 which
catalog entry points lifecycle validation should call, and tell Phase 4 how Steel/Oil/Supply costs
attach to faction catalog entries.

## Player-Facing Outcome

No intended gameplay change. Internally, the current faction becomes one catalog entry instead of
the implicit global tech tree.
