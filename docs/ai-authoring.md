# Author a Rust AI strategy

The supported authoring seam is `rts_ai::sdk`. A strategy implements the object-safe `AiStrategy`
lifecycle and is passed to `AiController::with_strategy`; the ordinary
`CanonicalAiTickDriver` then supplies fog-filtered frames and enqueues emitted actions through the
same simulation validation and replay path as built-in AIs.

The runnable specimen is
[`server/crates/ai/examples/reference_strategy/strategy.rs`](../server/crates/ai/examples/reference_strategy/strategy.rs).
It uses only public SDK imports, including `AiRulebook` and `WorldQueries` for faction rules and
known-world selection plus `AiActionRequest`, `AiActions`, `AiFrame`, `AiStrategy`, and
`EntityKind`. Run the complete canonical host example with:

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

## Emit actions

Create requests in `step` by calling `AiActions::submit`. Requests are retained in call order up to
the documented per-step bound, translated after `step` returns, and then processed as ordinary
commands; a `true` return means only that the request was retained, not that the simulation
accepted or completed it. The specimen gathers steel with one worker and later attack-moves a
different worker toward a public enemy start, tolerating missing workers, resources, or opponents
without panicking.

The external integration test compiles the strategy without access to `rts_ai` internals, checks
the lifecycle cadence, runs the same seeded matchup twice, compares ordered command logs, observes
the commands taking effect, and replays the ordinary command log. The specimen derives its
expansion-saving threshold from the rulebook and chooses steel through the known-world resource
index:

```bash
cargo nextest run --config-file .config/nextest.toml \
  --manifest-path server/Cargo.toml --profile default \
  -p rts-ai --test sdk_external
```

## Concrete next API needs

The specimen keeps the remaining action-construction friction visible for the next SDK phase.

Typed per-think actions, in priority order:

1. `gather` and `attack_move` helpers that accept typed candidates, preserve caller order, and
   prevent one unit from receiving conflicting same-think assignments.
2. `train`, `build`, and `research` helpers with same-think resource/supply budgets and explicit
   reservations shared across helper calls.
3. Local blocker results such as missing producer, known prerequisite, known resource, or action
   capacity; emission success must remain distinct from simulation acceptance or completion.
