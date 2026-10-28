# Author a Rust AI strategy

The supported authoring seam is `rts_ai::sdk`. A strategy implements the object-safe `AiStrategy`
lifecycle and is passed to `AiController::with_strategy`; the ordinary
`CanonicalAiTickDriver` then supplies fog-filtered frames and enqueues emitted actions through the
same simulation validation and replay path as built-in AIs.

The runnable specimen is
[`server/crates/ai/examples/reference_strategy/strategy.rs`](../server/crates/ai/examples/reference_strategy/strategy.rs).
It uses only public SDK imports, including `AiRulebook` and `WorldQueries` for faction rules and
known-world selection plus `AiActions`, `AiFrame`, `AiStrategy`, `EntityKind`, and `UnitGroup`.
Run the complete canonical host example with:

```bash
cargo run --manifest-path server/Cargo.toml -p rts-ai --example reference_strategy
```

## Lifecycle and knowledge

`initialize` runs once, immediately before the first `step`; both run only on the player's
nine-tick staggered decision cadence. Keep deterministic cross-tick memory in the strategy object,
as the specimen does for its public enemy-start point, step count, and one-time scout dispatch.
Frames and their collections are owned and stable-id ordered, so preserve that ordering or add an
explicit stable tie-break when selecting candidates.

An `AiFrame` contains owned entities, currently visible allies and enemies, remembered contacts,
recipient-visible economy and production, public player starts and map data, static resource
locations, and controller-inferred submitted builds. Remembered contacts are stale, resource
amounts may be `Unknown`, and submitted builds are not acceptance receipts. Strategies receive no
full simulation snapshot, hidden occupancy, private pathing state, or command-completion status.

Use `frame.own_losses().units_lost()` for your total destroyed units, and
`frame.own_losses().buildings_lost()` for buildings. `units` and `buildings` maps break counts down
by `EntityKind`; `steel_value` and `oil_value` give their combined nominal cost. Query
`frame.observed_enemy_losses().get(&enemy_id)` for the same summary of witnessed enemy deaths.
An absent entry means none observed. Deaths in hidden areas are excluded, and an enemy leaving
sight is never assumed dead. Totals accumulate every simulation tick and survive checkpoints.

## Read rules and query the known world

Create `AiRulebook::for_frame(frame)` to bind authoritative `rts-rules` answers to the frame
player's faction. The bounded rulebook exposes faction catalog order and availability, Steel/Oil
cost, supply, production time, maximum health, building footprint, builder/producer relationships,
prerequisites, gathering capability, and train/research relationships. It does not duplicate rule
tables. Upgrade costs and timings remain omitted because their current authority is in the
simulation crate; this SDK phase deliberately does not move that ownership.

`WorldQueries::new(frame)` indexes owned entities, visible allies and enemies, remembered contacts,
and known resources by stable id. Current and remembered contacts stay separate. Nearest queries
use squared `f32` distance with stable-id tie-breaking, and known-resource iteration filters only
nodes explicitly observed as exhausted; an unknown quantity is not treated as empty.

Use its checked `tile`, `world_point`, `world_to_tile`, and `tile_center` helpers before issuing
coordinate-based requests. Known build-site answers are exactly `Invalid`, `KnownBlocked`, or
`NoKnownConflict`: the last name means only that public terrain, resources, current visible
buildings, production exits, and explicit controller exclusions reveal no conflict. It never means
legal, clear, placeable, reachable, or accepted, and hidden dynamic state is never consulted.

## Address numbered base sites

`WorldQueries::base(player_id, 3)` finds that player's third geographic base site. Rank 1 is
always their original main; later ranks use static ground pathing distance around terrain with
deterministic ties. Unreachable sites sort last and report `distance_from_main = None`.
This is independent of which sites they actually occupy. Sites include every player's main and
inferred resource clusters, and remain numbered after depletion or a Depot's destruction.

```rust
let world = WorldQueries::new(frame);
if let Some(third) = world.base(enemy_player_id, 3) {
    let (x, y) = third.site.position;
    // Use (x, y) as a scouting or attack-move objective.
    let evidence = world.known_base_depots(third.site.id);
    // Inspect owner and Owned / Visible / Remembered evidence before assuming occupation.
}
```

`frame.base_sites()` supplies the static landmarks and resource IDs; `ranked_bases(player_id)`
returns the complete order. Positions are world pixels, not legal building footprints. An empty
Depot evidence list means no known occupant, and remembered evidence is stale. Geography reveals
no hidden enemy buildings. Paths use the public terrain map, forbid diagonal corner cutting,
and ignore dynamic units/buildings and unit-specific clearance. Distances are world pixels;
cardinal/diagonal steps cost 1/1.4 tiles. Reachable distances are available on each site's
`ground_distances_from_starts` map.

## Emit actions

Call typed helpers on the `AiActions` supplied to `step`. The builder supports `paid_build`,
`resume_build`, `train`, `set_production_repeat`, `research`, `gather`, `move_group`,
`attack_move`, `attack`,
`hold_position`, and `setup_anti_tank_guns`. Tactical helpers take a `UnitGroup`, which sorts and
deduplicates IDs and rejects an empty group. Worker, resource-node, and producer candidate slices
stay in caller order so priority remains explicit.

The builder reserves same-think Steel, Oil, Supply, actors, resource nodes, and producers before
emission. Those three ID namespaces are independent. A helper returns `ActionError` with a local
`ActionBlocker` for a fact it can establish, and any such return leaves budget, reservations,
emitted actions, and trace unchanged. An `Ok` means only that the action was emitted into this
local batch; the simulation may still ignore it through ordinary validation. The builder makes no
claim of command acceptance, legality, completion, or simulation atomicity.

The specimen enables Pump Jack repeat at its Resource Depot and later attack-moves its Engineer
toward a public enemy start, tolerating missing Engineers, resources, or opponents without panicking. Actions
are retained in helper-call order up to the per-think cap and translated to ordinary commands only
after `step` returns.

The external integration test compiles the strategy without access to `rts_ai` internals, checks
the lifecycle cadence, runs the same seeded matchup twice, compares ordered command logs, observes
the commands taking effect, and replays the ordinary command log. The specimen verifies the
faction through the rulebook and discovers Oil through the known-world resource index:

```bash
cargo nextest run --config-file .config/nextest.toml \
  --manifest-path server/Cargo.toml --profile default \
  -p rts-ai --test sdk_external
```

Upgrade costs remain owned by the simulation; `research` consults that existing definition for
local affordability without publishing a duplicate rule table. Cross-tick goals, command receipts,
task status, automatic planning, and completion inference remain outside this SDK.

## Optional wounded-unit pull-back

Keep one `PullbackMemory` per caller-owned squad. Call `memory.step(frame, &rifle_ids,
PullbackParams::default(), actions)` before issuing that squad's ordinary orders. It emits only
new retreat Moves; skip normal orders for the returned `active` IDs on every decision and restore
your objective or route for `released` IDs. Defaults match Jeff's rifle micro: at most 15 HP,
three tiles, 45 ticks, triggered by a visible enemy targeting the unit while a healthier squad
member remains. The helper never chooses squads, attack objectives, or routes. Use current owned
IDs and choose membership explicitly; its SDK adapter filters dead/incomplete members and uses
only currently visible enemy units. Failed local Move emission is retried on a later decision.

`plan` is the lower-level pure planner for integrations that emit their own orders; supply live
`PullbackUnit` facts and manage emission failures there. Destinations are clamped to map bounds,
without a terrain or safety guarantee. This helper does not reserve existing active members in
later action batches, so caller-side ownership checks are required throughout a retreat.
