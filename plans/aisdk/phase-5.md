# Phase 5 - Expose Rules and Known-World Queries

## Phase Status

- [ ] Ready for implementation after Phase 4 merges.

## Objective

Expose the game semantics and deterministic known-world queries AI authors currently reconstruct by
hand. Assemble a faction-bound `AiRulebook` from authoritative `rts-rules`, and construct
`WorldQueries` only from `AiFrame` plus public static map data. Preserve Jeff's exact placement and
geometry behavior behind compatibility policies and never imply knowledge of hidden dynamic state.

## Authoritative Rules Cleanup

- Move pure upgrade definition data and `required_for_unit` from `rts-sim::game::upgrade` into an
  authoritative `rts_rules::upgrade` module, using the existing `rts_rules::faction::UpgradeKind`
  and balance constants.
- Leave `rts_sim::game::upgrade` as a compatibility re-export/delegator so simulation callers,
  iteration order, replay, costs, duration, producers, prerequisites, and stable IDs do not change.
- Add a small faction-bound `AiRulebook` façade that delegates to existing `rts-rules` definitions,
  economy, faction, upgrade, combat, target, and terrain modules. Do not create another entity,
  building, upgrade, or ability table.
- Cover the high-value author questions:
  - definition and faction availability;
  - costs, supply, build time, max health, footprint, roles/capabilities, and weapon range facts;
  - builder/producer relationships and train/build prerequisites;
  - researchable upgrades, upgrade prerequisites/cost/duration, and unit unlocks;
  - supported abilities where already available from the rules catalog.
- Preserve authoritative catalog ordering in faction-filtered results and distinguish global
  definition existence from faction availability.

## Perspective-Safe World Queries

- Add deterministic indexes for owned entities, currently observed allies/enemies, separately
  labeled remembered contacts, and known resources. Returned collections have documented stable ID
  ordering; caller-ordered candidate lists remain separate when order expresses policy.
- Add checked, finite, bounds-safe world/tile coordinate helpers.
- Reuse cached public static map analysis for tile passability, clearance, component ID, and a
  result named `StaticConnectivity::{Connected, Disconnected, Invalid}`. It is terrain-only and
  must not be exposed as current reachability or pathability.
- Preserve map-analysis cache invalidation when Lab terrain/start/resource identity changes.
- Move the existing AI-known placement approximation behind the query surface with uncertainty
  types such as `Invalid`, `KnownBlocked`, and `NoKnownConflict`.
- Preserve Jeff's legacy placement/search policy exactly:
  - ring traversal, tie-breaking, `f32` comparisons, radii, and center preferences;
  - existing AI building clearance, public resource occupancy, projected-building treatment,
    footprint bounds/passability, producer spawn-tile check, and failed-site exclusions;
  - the current omission of unit-body and remembered-building occupancy.
- Never name a known-world result `Legal` or `Placeable`; ordinary simulation command validation
  remains authoritative and hidden blockers must not become query side channels.
- Defer the current defensive firing-lane approximation. It mixes enough combat-facing semantics
  that it should be reconsidered only after the smaller rulebook, placement, and static-connectivity
  API has real consumers; Jeff continues to use its compatibility helper unchanged.

## Security Invariants

- `WorldQueries` may accept only `AiFrame`, public static map analysis, and explicit controller
  exclusions such as failed known sites.
- It may never accept `Game`, `EntityStore`, full snapshots, authoritative occupancy/standability,
  fog internals, path caches, or private LOS/combat services.
- Two games that differ only in player-hidden dynamic state and produce the same frame must produce
  identical query results.
- Remembered contacts remain labeled as remembered and are not silently treated as current
  blockers or targets.

## Expected Touch Points

- `server/crates/rules/src/lib.rs` and a new `rules/src/upgrade.rs`.
- `server/crates/sim/src/game/upgrade.rs` compatibility shim.
- New `server/crates/ai/src/sdk/{rulebook.rs,world_queries.rs}` and SDK exports.
- `server/crates/ai/src/ai_shared.rs` and `selfplay/player_view.rs` placement helpers.
- `server/crates/ai/src/ai_core/map_analysis.rs`.
- Compatibility call sites in actions/defense/runtime only as required for exact delegation.
- Focused rules/query tests.
- `docs/design/ai.md`, `docs/design/server-sim.md`, and the relevant balance/rules source-of-truth
  documentation for the upgrade ownership move.

No wire protocol or client changes belong in this phase. An unexpected need for them is scope
expansion and must stop for replanning.

## Implementation Checklist

- [ ] Move pure upgrade authority into `rts-rules` with a sim compatibility seam.
- [ ] Add the faction-bound `AiRulebook` without duplicating registries.
- [ ] Add deterministic indexes, coordinate helpers, and static connectivity.
- [ ] Add uncertainty-labeled known-world placement/search.
- [ ] Delegate Jeff through exact compatibility query policies.
- [ ] Leave firing-lane behavior behind the unchanged Jeff compatibility helper.
- [ ] Add hidden-state A/B and legacy/new equivalence tests.
- [ ] Pass the unchanged Phase 1 transcript fixture.
- [ ] Update ownership and AI design documentation.
- [ ] Mark this phase done in the implementation commit.

## Verification

- Prove every façade answer matches direct authoritative rules/catalog data and preserves order.
- Prove every upgrade definition matches the pre-move simulation definition exactly.
- Test deterministic indexes, coordinate bounds, static components, Lab cache invalidation, and
  input-order independence where ordering is not a policy input.
- Compare legacy/new placement results and candidate invocation sequences across every building
  kind, representative maps, edge/resource/visible-building/spawn-blocked cases, failed exclusions,
  and randomized candidate grids.
- Add hidden-world A/B tests and run the exact Phase 1 normal/full Jeff transcript without fixture
  regeneration.
- Run focused `rts-rules` and `rts-ai` tests, crate-boundary checks, sim archcheck, docs health, and
  diff check.

## Manual Test Focus

Inspect a deterministic Jeff matchup around initial production buildings, Pump Jacks, expansion,
and the home Machine Gunner/Anti-Tank defense. In a controlled perspective-safe fixture, show that a
visible blocker may affect a query while an otherwise identical hidden blocker does not. First Tank,
Scout Car, expansion, and attack timing must remain identical.

## Non-Goals

- No A*, navigation mesh, path distance/ETA, traffic, vehicle-body, formation, influence-map, or
  tactical-search API.
- No authoritative dynamic occupancy, standability, target legality, shot interception, smoke, or
  combat LOS oracle.
- No public firing-lane query in this phase.
- No claim that a candidate will be accepted by the server.
- No remembered-enemy prediction, behavior tree, GOAP, or Jeff tuning.
- No protocol additions or external plugin ABI.

## Handoff Expectations

Report the final rulebook/query API, authoritative upgrade owner and compatibility seam, proof that
no duplicate registry exists, input-provenance table for every query, exact uncertainty semantics,
legacy quirks preserved, helper-equivalence results, full Jeff parity fixture identity, and manual
placement/defense observations. Tell Phase 6 which explicit candidates and rule answers the planner
may consume without claiming authority.
