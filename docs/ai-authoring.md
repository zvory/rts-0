# Author a Rust AI strategy

The supported authoring seam is `rts_ai::sdk`. A strategy implements the object-safe `AiStrategy`
lifecycle and is passed to `AiController::with_strategy`; the ordinary
`CanonicalAiTickDriver` then supplies fog-filtered frames and enqueues emitted actions through the
same simulation validation and replay path as built-in AIs.

The runnable specimen is
[`server/crates/ai/examples/reference_strategy/strategy.rs`](../server/crates/ai/examples/reference_strategy/strategy.rs).
It uses only these public SDK imports: `AiActionRequest`, `AiActions`, `AiFrame`,
`AiResourceAmount`, `AiStrategy`, and `EntityKind`. Run the complete canonical host example with:

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

## Emit actions

Create requests in `step` by calling `AiActions::submit`. Requests are retained in call order up to
the documented per-step bound, translated after `step` returns, and then processed as ordinary
commands; a `true` return means only that the request was retained, not that the simulation
accepted or completed it. The specimen gathers steel with one worker and later attack-moves a
different worker toward a public enemy start, tolerating missing workers, resources, or opponents
without panicking.

The external integration test compiles the strategy without access to `rts_ai` internals, checks
the lifecycle cadence, runs the same seeded matchup twice, compares ordered command logs, observes
the commands taking effect, and replays the ordinary command log. Its intentionally hard-coded
steel target is concrete evidence for the faction rulebook gap below:

```bash
cargo nextest run --config-file .config/nextest.toml \
  --manifest-path server/Cargo.toml --profile default \
  -p rts-ai --test sdk_external
```

## Concrete next API needs

The specimen intentionally keeps first-use gaps visible for the next SDK phases.

Rules and fog-safe queries, in priority order:

1. Faction-scoped costs, supply, prerequisites, builders, producers, and research catalogs, so an
   author need not hard-code rules before choosing a production action.
2. A known-world resource query that filters exhausted nodes and reports known mining conflicts
   without implying hidden occupancy or authoritative placement legality.
3. Uncertainty-honest building-site and known-target queries with deterministic nearest/stable-id
   selection; no hidden path or line-of-fire oracle.

Typed per-think actions, in priority order:

1. `gather` and `attack_move` helpers that accept typed candidates, preserve caller order, and
   prevent one unit from receiving conflicting same-think assignments.
2. `train`, `build`, and `research` helpers with same-think resource/supply budgets and explicit
   reservations shared across helper calls.
3. Local blocker results such as missing producer, known prerequisite, known resource, or action
   capacity; emission success must remain distinct from simulation acceptance or completion.
