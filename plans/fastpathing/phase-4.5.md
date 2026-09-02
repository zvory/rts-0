# Phase 4.5 - All-Order Terrain Routes

## Phase Status

- [ ] Policy rollout complete; interaction endpoint ranking remains incomplete.

## Objective

Make the authoritative terrain-time policy the default for every live ground-unit route. Direct
attacks, construction, repair, gathering, deconstruction, abilities, rallies, and every other
interaction should use a road, avoid slow terrain, or choose a favorable elevation route whenever
that route is actually faster. The interaction still defines where the unit must end; terrain time
defines how it gets there.

## Product Rule

- Do not prefer crossing an open field merely because it is geometrically direct. If the complete
  legal road route is faster under the authoritative metric, take the road.
- Preserve the destination semantics of each order. Attack range bands, building staging rings,
  resource slots, ability launch positions, rally destinations, and exact footprint/body legality
  remain authoritative. This phase changes route cost and route choice, not what counts as arrival.
- Apply the rule to infantry, workers, cars, pivot vehicles, and future ground movement profiles.
  Air movement and systems that do not use the ground pathing service are out of scope.
- Temporary buffs, debuffs, and moving ability fields remain outside static route selection unless a
  later design explicitly makes them path-cost inputs. Roads, slow authored tiles, elevation, and
  existing nonnegative shaping use the Phase 3/4 metric.

## Production Policy Boundary

- Convert every production `PathRequest` source to `FastestTerrainTime`: `Move`, `AttackMove`,
  `DirectAttack`, `Gather`, `Build`, `Deconstruct`, `Ability`, and `Other`.
- Cover repair or future interaction sources through the same exhaustive policy mapping rather than
  letting an unrecognized caller silently default to `LegacyShape`.
- Retain `LegacyShape` only for old serialized-state compatibility, explicit parity/oracle fixtures,
  and narrowly documented non-production diagnostics. After a new path is assigned, no live ground
  order should retain a legacy route merely because of its order type.
- Remove legacy clear-line/direct-goal bypasses from these production routes. Same-tile completion or
  an interaction already in legal range may still complete without search; that is arrival logic,
  not a terrain-blind route choice.
- Make the policy selection exhaustive at the `PathingRequestSource` boundary so adding a new order
  source requires an explicit compile-time choice and focused test.

## Interaction Route Contract

- Direct Attack keeps its target lock, closest target/footprint point, weapon min/max range band,
  pursuit phase, repath throttle, and moving-target refresh rules. Each actual pursuit path uses the
  terrain-time objective even when that means joining a road before entering range.
- Build and Deconstruct keep their legal outside-footprint staging rings and complete body checks.
  Candidate landing selection must compare end-to-end terrain route cost, not choose the nearest
  geometric landing first and only then route to it.
- Gather uses terrain time on the outbound route, resource-slot approach, return-to-depot route, and
  any blocked-slot repath. Slot ownership and mining-cycle state remain unchanged.
- Ability movement keeps the exact legal launch/staging position and cast range. Reaching that
  position uses terrain time; no shortcut may launch from an illegal point.
- Rally and production movement use the same policy after spawn. Spawn legality and exit selection
  remain body-safe; when multiple legal exits or landing points are semantically equivalent, rank
  them by complete terrain route cost with the existing deterministic tie order.
- `Other` orders must be enumerated during implementation. Classify artillery/blanket-fire landing,
  queued order resumption, recovery, and any remaining caller instead of treating `Other` as an
  exemption.
- Exact final approach helpers may retain specialized legality and candidate generation, but not a
  legacy travel-time objective. A two-stage implementation is acceptable only if both the transit
  path and the final legal approach minimize the same authoritative terrain-time metric.

## Work

- Inventory every production `PathRequest`, direct-segment bypass, same-tile special case, footprint
  candidate search, route analyzer, rally/spawn route, and mid-tick repath caller.
- Replace order-type policy exceptions with an exhaustive all-ground terrain-time policy seam.
- Extend footprint and range-ring candidate selection to use full returned-route cost while keeping
  deterministic bounds and the existing pathfinding allowance.
- Preserve Phase 3/4 one-time finalization, protected shaping anchors, full-body/hull legality, and
  authoritative anchor following for all order sources.
- Add per-source diagnostics for formerly direct-bypass requests, cache outcomes, expansions,
  deferrals, finalization, and moving-target repath frequency.
- Update source-of-truth pathing/interaction documentation and stage one factual player-facing note.

## Correctness Gates

- Every production ground route source assigns and serializes `FastestTerrainTime`; an exhaustive
  test fails if a source or caller remains unintentionally legacy.
- Raw paths equal the Dijkstra oracle under the correct infantry/car/pivot composite graph objective.
- Each interaction reaches the same legal destination set and triggers at the same range/footprint
  boundary as before; only route choice and resulting arrival time may intentionally differ.
- Offset-road fixtures prove that Direct Attack, Build, Gather, Deconstruct, Ability, rally, and each
  classified `Other` route take the road when it is faster and cross the field when it is faster.
- Dynamic target motion, building closure/reopen, exhausted resource slots, destroyed targets,
  queued orders, and stale ids remain deterministic, bounded, and panic-free.
- Repeated new-build replay/snapshot streams are byte-identical. Legacy output may differ wherever
  the written terrain metric or authored-anchor contract changes a route.

## Performance Gates

- Report eleven paired cold and warm full-return measurements for every newly converted source,
  separating former-direct-bypass requests and moving-target repaths.
- Preserve the eight-request allowance, heavy-search threshold, caps, and fallback semantics. Forty
  non-heavy requests from each source family resolve within five ticks unless an existing semantic
  wait—not search scheduling—prevents assignment.
- Finalization remains at most 10% of total path-request time and is paid once per assigned path.
- Hellhole median tick upper ratio is at most 1.03 against Phase 4 and the frozen baseline; p95 stays
  within the plan bound. The completed all-order system must also satisfy the plan's final
  no-slower-than-frozen Hellhole target or stop with per-source attribution.
- A missed performance gate does not permit reducing route fidelity, repath correctness, simulation
  cadence, request allowances, or terrain awareness. Report the dominant source and seek a separate
  optimization plan.

## Verification and Manual Focus

- Focused tests for every `PathingRequestSource`, footprint/range candidate selection, cache identity,
  checkpoint restore, dynamic blockers, capped fallback, and schedule latency.
- Direct Attack against stationary and moving targets across an offset road; worker construction and
  deconstruction on the far side of a road; repeated resource trips; ability staging; produced-unit
  rally; artillery/blanket-fire and every enumerated `Other` case.
- Infantry, Scout Car, and Tank versions where applicable, including narrow turns and reverse
  recovery after an interaction target changes.
- Eleven paired source corpus and Hellhole runs, deterministic snapshot-stream comparison, full
  simulation suite, Clippy, architecture check, docs health, and `git diff --check`.

## Handoff Expectations

Report the exhaustive caller/source inventory, interaction destination invariants, candidate-ranking
rule, every intentional route/output difference, per-source oracle and deterministic hashes,
scheduling results, paired performance, Hellhole result, and staged patch-note text. Confirm that no
new production ground route remains terrain-blind, or name the exact justified exception.

## Implementation Evidence

- The implementation commit on `zvorygin/fastpathing-phase-4.5` makes the production policy match
  exhaustive and maps all sources to `FastestTerrainTime`: `Move` (commands, rally, queued
  resumption), `AttackMove` (commands, rally, combat/artillery resumption), `DirectAttack` (combat
  pursuit and Panzerfaust), `Gather` (node approach, blocked-slot repath, and depot return), `Build`
  and `Deconstruct` (footprint approach plus fallback staging candidates), `Ability` (legal cast
  staging), and `Other` (the exhaustively enumerated Idle, Hold Position, Artillery Point Fire, and
  Artillery Blanket Fire variants). Those `Other` orders do not currently originate a live ground
  route; their explicit mapping prevents a future caller from treating the bucket as an exemption.
  The current order model has no separate repair route; any future source variant must be
  classified before the exhaustive match compiles.
- General requests, exact footprint requests, same-tile tree detours, and nonempty same-tile nudges
  persist the selected policy atomically with the path. Production requests never pass a legacy
  direct segment. Same-tile completion/current legal footprint staging remains arrival logic and
  may hold no path. Oriented vehicles use `VehicleClearance` for interactions as well as movement,
  retaining Phase 4 hull, turn, anchor, lookahead, and recovery rules.
- Direct Attack still derives the same closest target point and min/max range-band staging goal;
  Build and Deconstruct retain the same outside-footprint ring and full body checks; Gather retains
  node/slot ownership and mining state; Ability retains its exact legal launch point; rally retains
  body-safe spawn selection. The primary footprint approach follows a weighted path toward the
  footprint and trims it at the last legal outside tile; deterministic bounded fallback candidates
  each use the same weighted exact-path seam. No destination or arrival predicate changed. However,
  fallback footprint candidates are still attempted in geometric order, and Direct Attack still
  derives one geometric range-band endpoint before routing. The implementation therefore does not
  yet compare all semantically equivalent endpoints by complete route cost as required above.
- The exhaustive policy test covers all eight source variants. The offset-road test exercises those
  variants through the shared generic request seam and proves that seam retains a faster road
  anchor; it does not independently exercise every live order caller or interaction endpoint
  selector. Existing Phase 4 graph/oracle and finalizer evidence remains applicable because this
  phase changes only production source selection: reference/candidate semantic hashes
  are `9a5b92a0d69cc0e5` / `a91153a2991dae45`, cold median/upper ratios are
  `0.29156` / `0.29287`, warm ratios are `0.23206` / `0.23599`, and finalization is `3.28%`.
- Eleven warmed alternating 900-tick Hellhole pairs against Phase 4 produced a median average-tick
  ratio of `1.00180` with one-sided bootstrap 90% upper `1.00336`; median p95 ratio was `1.00221`
  and wall ratio `1.00325`. Baseline/candidate MAD was `0.16%` / `0.17%`. Against the frozen
  pre-Phase-1 median the candidate ratio is `0.71997`, with all 11 samples improving. Raw rows are
  checked in as `phase-4.5-results.json`.
- Two independently generated 900-frame streams were byte-identical at 24,899,122 bytes, SHA-256
  `396417a116a3c0b98b858d4782b56acf59f30e58fff9e5366892f400e05f2af4`. Hellhole remained
  semantically identical to Phase 4 at 26,833,202 serialized snapshot bytes, 14,899 attack events,
  92 projectile events, and 937 death events. Intentional differences are limited to route geometry
  and resulting arrival timing when a newly converted interaction encounters terrain with a faster
  non-geometric route; there is no production ground-route exception.
- Focused verification: `cargo test --manifest-path server/Cargo.toml -p rts-sim --lib
  move_coordinator::tests`; `cargo test --manifest-path server/Cargo.toml -p rts-sim --lib
  generic_request_path_uses_a_faster_offset_road_for_every_source`; two release snapshot-stream
  generations;
  eleven paired release Hellhole runs; Clippy, simulation architecture, docs health, and
  `git diff --check`.
- Patch-note text staged locally and not delivered: “All ground orders now choose routes by travel
  time, so attacks, worker interactions, abilities, and rally movement can use faster roads and
  avoid slow or unfavorable terrain without changing their legal destination.”
