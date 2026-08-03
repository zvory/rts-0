# Phase 5 - Expose the Rules and Queries Authors Actually Need

## Phase Status

- [ ] Ready for implementation after Phase 4 merges and reports concrete gaps.

## Objective

Expose a small faction-aware rulebook and fog-safe known-world queries for questions the Phase 4
reference strategy or current AI code actually asks. Delegate to existing authoritative rules and
existing AI map/placement logic; do not move ownership between crates or build a comprehensive game
ontology in the name of the SDK.

## Public Surface

- Add a faction-bound `AiRulebook` that delegates to current authoritative `rts-rules` definitions.
  Start with the demonstrated high-value questions: costs, supply, build/train time, health,
  footprint, builder/producer relationships, faction availability, prerequisites, and existing
  capabilities needed by a real consumer.
- Add deterministic `WorldQueries` indexes for owned entities, currently visible allies/enemies,
  remembered contacts, and known resources. Keep current and remembered knowledge separate.
- Add checked finite world/tile coordinate helpers.
- Expose the existing AI-known build-site approximation behind uncertainty-honest results such as
  `Invalid`, `KnownBlocked`, and `NoKnownConflict`. Never call the result legal, placeable, clear,
  or accepted.
- Preserve the existing candidate order, ring traversal, tie-breaking, floating-point operations,
  footprint checks, public-resource treatment, projected-building treatment, and failed-site
  exclusions for compatibility consumers.
- Add static terrain connectivity only if the Phase 4 consumer needs it. Name it explicitly as
  static connectivity, never current reachability or pathability.

## Scope Guardrails

- Do not move upgrade definitions from `rts-sim` to `rts-rules` in this phase. That ownership cleanup
  is independently plausible but not required to improve authoring and would enlarge the contract
  and verification surface.
- Do not duplicate entity, building, upgrade, weapon, or ability tables. If a requested answer lacks
  an authoritative source, omit it and record the gap.
- `WorldQueries` may depend only on `AiFrame`, public static map data/analysis, and explicit
  controller-owned exclusions. It may not accept `Game`, full snapshots, entity stores, hidden
  occupancy/standability, fog internals, or simulation path caches.
- Hidden dynamic state must not affect a query when the input frame is unchanged.
- Do not expose the defensive firing-lane approximation, A*, ETA, tactical search, target legality,
  line of fire, or authoritative dynamic placement.

## Existing-AI Adoption

- Route at least one real Jeff/AI-2.1 rule lookup and one placement/query call through the shared
  implementation, using narrow compatibility policy where truthful public knowledge differs from
  historical inputs.
- Treat that adoption as the proof that the surface is not an unused façade. Preserve the Phase 1
  transcript exactly; do not tune any policy.
- Remove a duplicate only when the adopted call site proves it is one-for-one obsolete.

## Expected Touch Points

- New `server/crates/ai/src/sdk/{rulebook.rs,world_queries.rs}` and SDK exports.
- Existing `rts-rules` APIs only where a small missing public accessor is required; no authority move.
- Existing AI placement/map-analysis helpers and their compatibility call sites.
- The Phase 4 reference consumer and focused public integration tests.
- `docs/design/ai.md` and the author guide.

No wire protocol, client, balance, lobby, or simulation command-processing changes belong here.

## Implementation Checklist

- [ ] Derive the initial API from named Phase 4/current-AI call sites.
- [ ] Add the bounded faction rulebook without a second registry.
- [ ] Add deterministic known-world indexes and checked coordinate helpers.
- [ ] Expose only the existing known-placement approximation with honest uncertainty names.
- [ ] Add static connectivity only for a concrete consumer.
- [ ] Adopt the surface in at least one current rule lookup and one current query call.
- [ ] Add hidden-state A/B and old/new equivalence tests.
- [ ] Pass the unchanged Phase 1 transcript and mark the phase done.

## Verification

- Prove every rulebook answer matches its authoritative source and preserves catalog ordering.
- Compare old and new placement results and candidate invocation order across representative maps,
  edge/resource/visible-building/spawn cases, failed exclusions, and randomized candidate grids.
- Prove identical frames produce identical query answers when hidden game state differs.
- Compile the Phase 4 consumer using public imports only.
- Run focused `rts-rules`/`rts-ai` tests, clippy, crate/sim architecture checks, docs health, diff
  checks, and the exact Phase 1 normal/full transcript without fixture regeneration.

## Non-Goals

- No exhaustive semantic catalog or unrelated authoritative-owner cleanup.
- No guarantee that a locally selected action will be accepted by the simulation.
- No hidden-state oracle, current reachability, path ETA, traffic, formation, firing-lane, influence
  map, or tactical solver.
- No task status, scheduler, behavior tree, GOAP, plugin ABI, or non-Rust contract.

## Handoff Expectations

Report each public rule/query with its concrete consumer and authoritative input, uncertainty and
ordering semantics, existing-AI call sites migrated, duplicates actually removed, hidden-state and
parity evidence, and specific action friction left for Phase 6.
