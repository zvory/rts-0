# Phase 5.5 - Architecture Course Correction Guardrails

Status: Done.

## Objective

Course-correct the faction architecture before Phase 6 ability registry work. The first five
phases created the right high-level seams, but the implementation still has a few compatibility
paths that can make future factions behave like Kriegsia if a lifecycle path forgets to validate
input. This phase should tighten those seams without adding Ekaterina gameplay, changing Kriegsia
balance, or starting the Phase 6 ability registry refactor.

## Review Findings To Address

- Gameplay rules and sim paths call `catalog_for_or_default`, so an unknown non-empty faction id
  can silently receive the Kriegsia catalog. Lifecycle validation currently prevents this in known
  lobby/replay paths, but the lower authoritative layer should also fail closed.
- Phase 5 moved standard starts into loadout records, but public `Game` constructors and tests
  still expose global `starting_steel`/`starting_oil` overrides. Those APIs keep the old
  single-loadout mental model alive and can bypass the per-player loadout story.
- Ability metadata is split across `rules::faction`, `game::ability`, command special cases,
  protocol ids, entity cooldown/use state, and client config. Phase 6 should consolidate sources of
  truth rather than adding another registry beside the old ones.
- Current guardrails are useful but too coarse: catalog parity only dumps the default catalog, and
  the faction-assumption checker mostly catches new files rather than new direct special cases
  inside already-approved large files.

## Scope

- Replace broad gameplay use of `catalog_for_or_default` with explicit fail-closed accessors.
  Missing or unknown non-empty faction ids must make faction-gated build/train/research/gather/
  supply/ability checks reject or become inert instead of falling back to Kriegsia.
- Preserve intentional defaulting only at lifecycle boundaries that own default assignment, such as
  normal lobby, quickstart, AI seat creation, and documented compatibility test helpers. Empty
  `PlayerInit.faction_id` may continue to default at the narrow compatibility boundary, but unknown
  non-empty ids must not.
- Add focused Rust tests proving unknown faction ids do not:
  - get Kriegsia starting entities or resources
  - count Kriegsia units/buildings for supply
  - build, train, research, gather, or use Kriegsia abilities
  - spend Steel/Oil or reserve Supply through rejected commands
- Narrow or rename global starting-resource constructors so production code and new tests prefer
  per-player `PlayerStartingLoadout` records. If old constructors must remain for existing tests,
  mark them as compatibility helpers and keep them out of replay/lifecycle reconstruction paths.
- Add validation for replay/loadout overrides where practical: each override should reference an
  existing player, match that player's faction id, and use that faction catalog's known loadout id.
  Invalid overrides should reject before a `Game` is built.
- Strengthen catalog parity so the Rust dump can expose all defined catalogs or an explicitly
  selected catalog, not only `CURRENT_CATALOG`. The client parity check may still assert only
  client-exposed catalogs, but it should make unsupported fixture/future catalog handling explicit.
- Tighten the faction-assumption checker so new direct current-faction special cases inside
  approved high-risk files are visible. A small count/anchor ratchet is acceptable if a full AST
  check is too much for this phase.
- Prepare Phase 6 by documenting the intended ability source of truth and the metadata that must
  move there. Do not implement the ability registry in this phase, but do add a parity note or test
  that calls out any current split metadata, especially ability costs and cooldowns that differ
  between command execution, `game::ability`, and client config.
- Update `docs/design/server-sim.md`, `docs/design/balance.md`, and
  `docs/design/faction-architecture-inventory.md` where the compatibility boundaries or guardrails
  change.

## Non-Goals

- Do not add Ekaterina catalog entries, command cards, lobby selection, art, AI behavior, or
  prediction support.
- Do not implement the Phase 6 ability registry or Phase 7 ability effect hooks.
- Do not migrate to generic resources.
- Do not preserve old replay artifacts that lack faction/loadout records.

## Expected Touch Points

- `server/crates/rules/src/faction.rs`
- `server/crates/rules/src/economy.rs`
- `server/crates/rules/src/bin/dump-faction-catalog.rs`
- `server/crates/sim/src/game/setup.rs`
- `server/crates/sim/src/game/services/commands.rs`
- `server/crates/sim/src/game/services/construction.rs`
- `server/crates/sim/src/game/services/supply.rs`
- `server/crates/sim/src/game/services/ability_orders.rs`
- `server/src/lobby/faction_validation.rs`
- `server/src/lobby/room_task.rs`
- `server/src/main.rs`
- `scripts/check-faction-assumptions.mjs`
- `scripts/check-faction-catalog-parity.mjs`
- focused Rust tests under `server/crates/rules`, `server/crates/sim`, and `server/src/lobby`
- `docs/design/faction-architecture-inventory.md`
- `docs/design/server-sim.md`
- `docs/design/balance.md`

## Verification

- `cargo test --manifest-path server/Cargo.toml -p rts-rules faction`
- Focused `rts-sim` tests for unknown-faction loadout, command, supply, gather, and ability
  rejection.
- Focused `rts-server` tests for replay/loadout validation if server lifecycle validation changes.
- `node scripts/check-faction-assumptions.mjs`
- `node scripts/check-faction-catalog-parity.mjs`
- `cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture`
- `node tests/protocol_parity.mjs` if replay/loadout or catalog dump contracts change.
- `git diff --check`

## Manual Testing Focus

No gameplay manual testing should be required beyond a normal Kriegsia sanity check if the executor
touches live start assembly. Kriegsia should still start with the same City Centre, Workers,
Steel/Oil/Supply values, command card, gathering, training, and replay startup behavior.

## Handoff Expectations

The handoff must name the new fail-closed catalog access pattern, list any remaining intentional
defaulting or compatibility constructors, describe how loadout override validation works, and tell
Phase 6 exactly which ability metadata source should become authoritative. It must also list any
remaining current-faction special cases that are still allowed and why they are not blockers for
Phase 6.

## Player-Facing Outcome

No intended gameplay change. This phase makes future faction mistakes fail closed instead of
quietly behaving like Kriegsia, and it makes the next ability-registry phase safer to execute.
