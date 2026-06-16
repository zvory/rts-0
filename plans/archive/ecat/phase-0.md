# Phase 0 - Inventory and Runtime Contract

Status: Done.

## Goal

Inventory the current ability, world-effect, projection, client preview, and test surfaces before
changing behavior. Define the shared naming and contracts that later phases will implement.

## Scope

- Inventory current ability metadata and execution:
  - `server/crates/rules/src/faction.rs`
  - `server/crates/sim/src/game/ability.rs`
  - `server/crates/sim/src/game/services/ability_orders.rs`
  - `server/crates/sim/src/game/hero_abilities.rs`
  - `server/crates/sim/src/game/services/order_queue.rs`
- Inventory existing non-entity world state patterns:
  - `server/crates/sim/src/game/smoke.rs`
  - `server/crates/sim/src/game/mortar.rs`
  - `server/crates/sim/src/game/artillery.rs`
  - `server/crates/sim/src/game/snapshot.rs`
  - `server/crates/sim/src/rules/projection.rs`
- Inventory wire and client mirrors:
  - `server/crates/contract/src/lib.rs`
  - `server/crates/protocol/src/lib.rs`
  - `server/src/protocol.rs`
  - `client/src/protocol.js`
  - `client/src/config.js`
- Inventory client ability UI:
  - `client/src/state.js`
  - `client/src/input/commands.js`
  - `client/src/minimap.js`
  - `client/src/hud_command_card.js`
  - `client/src/renderer/feedback.js`
  - `client/src/renderer/index.js`
- Inventory tests and checks that later phases should extend:
  - focused Rust sim tests around command validation, snapshots, fog privacy, and ability orders
  - `tests/client_contracts.mjs`
  - `tests/protocol_parity.mjs`
  - `scripts/check-client-architecture.mjs`
  - `cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture`
- Decide and record:
  - the server type names for active ability instances and projected ability world objects
  - the initial object kinds needed by this plan, such as return marker, anchor, and projectile
  - whether projectile visuals are projected as active objects, transient events, or both
  - the recast command shape to be implemented in Phase 4
  - whether the existing `ekatTeleport` and `ekatLineShot` ids should be repurposed for the new
    dash and projectile behavior or replaced with new ids; choose the convenient path, but do not
    preserve the old immediate teleport or immediate line-damage semantics as product behavior
  - the projectile return-target contract, including Ekat projectiles returning toward Ekat's current
    position rather than a fixed launch origin
  - how anchors are damaged or targeted in Phase 8 without becoming ordinary production entities

## Expected Deliverables

- This phase document updated with inventory notes and explicit decisions.
- A short implementation map naming the files each later phase should touch first.
- No gameplay behavior changes.
- No protocol or client behavior changes unless the phase only updates planning/design notes.

## Out of Scope

- Adding the runtime store.
- Adding new protocol fields.
- Implementing dash, return, projectile, or anchor behavior.
- Retuning Ekat stats or cooldowns.

## Verification

- Use `rg`/`fd` inventories and cite concrete files in the phase notes.
- No automated suite is required if this phase only updates planning notes.

## Manual Testing Focus

None. This is a contract and inventory phase with no intended player-facing change.

## Handoff Expectations

The handoff must name the chosen runtime/object names, the recast command shape, the old-id
repurpose/removal decision, the projectile return-target contract, the anchor targetability decision,
the projection strategy for projectile visuals, and the exact files Phase 1 should edit first.

## Inventory Notes

### Ability Metadata and Execution

- `server/crates/rules/src/faction.rs` is the ability metadata source. Ekat currently exposes
  `EKAT_TELEPORT_ABILITY` as `ekatTeleport` with compact ability code `6` and order-stage code `12`,
  and `EKAT_LINE_SHOT_ABILITY` as `ekatLineShot` with compact ability code `7` and order-stage code
  `13`. Both are Ekat-carried, world-point command-card abilities in the `ekat` faction catalog.
- `server/crates/sim/src/game/ability.rs` mirrors those ids as `AbilityKind::EkatTeleport` and
  `AbilityKind::EkatLineShot`. Their current `AbilityEffectHook` values are `Teleport` and
  `LineDamage`, which are one-off hooks rather than reusable runtime categories.
- `server/crates/sim/src/game/services/ability_orders.rs` owns validation and launch. All
  world-point abilities pass through map clamping, faction/carrier validation, tech checks,
  cooldown/use/cost checks, range checks, and optional order staging before the hook-specific match.
  Ekat teleport immediately computes a destination, spends cost, starts cooldown, moves Ekat, and
  emits a positioned "Teleport" notice. Ekat line shot immediately applies line damage, starts
  cooldown, clears the active order unless preserving it, and emits a positioned "Line Shot" notice.
- `server/crates/sim/src/game/hero_abilities.rs` contains the current bespoke Ekat behavior:
  `ekat_teleport_destination`, `move_ekat_to`, and `apply_ekat_line_shot`. Teleport uses
  `standability::unit_static_standable` for Ekat at the clamped target. Line shot clamps to range,
  gathers enemy hits along a segment, dedupes implicitly by scanning entity ids once, and applies
  immediate damage with attack attribution.
- `server/crates/sim/src/game/services/order_queue.rs` promotes queued ability orders through
  `launch_world_ability` after movement/facing readiness. It has no separate recast path; queued
  ability intents still require a world point.

### Non-Entity World State Patterns

- `server/crates/sim/src/game/smoke.rs` is the closest persistent world-object pattern. It has a
  cloneable store, deterministic ids, pending spawn support, active retention by tick, map-point
  validation, visibility checks, and snapshot projection via `SmokeCloudView`. This is the best
  model for return markers and anchors that persist across ticks and need fog-filtered projection.
- `server/crates/sim/src/game/mortar.rs` and `server/crates/sim/src/game/artillery.rs` are delayed
  projectile/event patterns. Their stores are cloneable, resolve due work in tick order, use
  `std::mem::take` to avoid mutation hazards while resolving, and gate launch/impact events through
  team/fog helpers. They are useful references for transient launch or impact events but are too
  event-only for selectable/debuggable ability objects.
- `server/crates/sim/src/game/snapshot.rs` projects entities, resource deltas, remembered
  buildings, smokes, visible tiles, upgrades, and optional full-world player resources. New ability
  world objects should be added alongside `smokes` with deterministic sorting by id.
- `server/crates/sim/src/rules/projection.rs` owns fog-safe projection details and owner-only
  affordances. It already projects entity ability cooldowns through `AbilityCooldownView` and
  owner-only order-plan markers, so later owner-only recast state should be projected here rather
  than by leaking it through public object fields.

### Wire and Client Mirrors

- `server/crates/contract/src/lib.rs` defines the semantic snapshot shape, including `Snapshot`,
  `SmokeCloudView`, `AbilityCooldownView`, entity views, and `Event`. This is the first contract
  file to extend for ability world objects.
- `server/crates/protocol/src/lib.rs`, `server/src/protocol.rs`, and `client/src/protocol.js`
  already reserve Ekat entity and ability ids. Compact protocol parity is guarded by
  `tests/protocol_parity.mjs`. Any new snapshot array, object-kind code, event code, command shape,
  or ability-state field must be mirrored across the Rust protocol crate, JS decoder, and
  `docs/design/protocol.md` in the phase that introduces it.
- `client/src/config.js` mirrors current Ekat ability ranges, cooldowns, radius/damage display data,
  and the Ekat faction command-card ability list. Later phases should keep server rules
  authoritative and update this file only for UI/render/fog-facing metadata.

### Client Ability UI

- `client/src/state.js` stores snapshots, transient events, command targeting state, projected
  smokes, and local ability target previews. It has no ability-object collection yet.
- `client/src/input/commands.js` and `client/src/minimap.js` issue `cmd.useAbility(ability, units,
  x, y, queued)` for world-point ability targeting. Self-target abilities can omit coordinates
  through command-card handling, but there is no explicit recast command shape yet.
- `client/src/hud_command_card.js` builds ability buttons from projected entity `abilities` plus
  `client/src/config.js` definitions. It computes enabled state from cooldowns, remaining uses,
  setup blockers, and affordability. Owner-only return availability or anchor lockout should enter
  as projected affordance state rather than client guesses.
- `client/src/renderer/feedback.js` draws generic range/cursor previews in
  `_drawAbilityTargetPreview`; `client/src/renderer/index.js` owns layer wiring and currently draws
  smokes, smoke canisters, mortar/artillery event visuals, selected ranges, and the ability preview.
  Ability objects should get their own renderer layer or focused feedback helper instead of being
  folded into smoke or mortar visuals.

### Tests and Checks to Extend Later

- Server sim coverage should start near `server/crates/sim/src/game/ability.rs`,
  `server/crates/sim/src/game/services/ability_orders.rs`, `server/crates/sim/src/game/snapshot.rs`,
  and `server/crates/sim/src/rules/projection.rs` for command validation, stale caster behavior,
  tick expiry, snapshot sorting, owner-only state, and fog privacy.
- Client contract coverage should extend `tests/client_contracts.mjs` for snapshot decoding, command
  builders, command-card affordances, state storage, and renderer preview behavior.
- Protocol parity belongs in `tests/protocol_parity.mjs` whenever compact codes or mirror constants
  change.
- Client architecture boundaries are checked by `scripts/check-client-architecture.mjs`; new client
  modules should follow the existing dependency-injection shape and only import allowed shared
  modules such as `protocol.js` and `config.js`.
- Server architecture checks remain
  `cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture` when new
  simulation modules or `Game` seams are introduced.

## Runtime Contract Decisions

- Server runtime store names:
  - Use `AbilityRuntime` as the owner stored on `Game`.
  - Use `ActiveAbilityInstance` for per-caster/per-activation state such as dash return and anchor
    lockout bookkeeping.
  - Use `AbilityWorldObjectStore` and `AbilityWorldObject` for persistent projected objects.
  - Use `AbilityProjectileStore` and `AbilityProjectile` for moving hit volumes introduced in the
    projectile phase.
- Projected object kind names:
  - `returnMarker` for the dash return point.
  - `magicAnchor` for the placed anchor.
  - `lineProjectile` for the moving Ekat projectile if represented as an active object.
- Projectile visual strategy:
  - Represent moving line projectiles as active ability objects while they exist so fog, replay, and
    late-joining snapshots have authoritative state.
  - Add transient launch, hit, expire, or destruction events only when a phase needs short-lived
    visual/audio feedback. Events are supplemental and must not be the only source of projectile
    position while a projectile is active.
- Recast command shape for Phase 4:
  - Add an explicit ability recast command rather than overloading missing `x`/`y` on
    `useAbility`.
  - Planned semantic shape: `recastAbility { ability, units, targetObjectId?, queued }`.
  - Planned client builder shape: `cmd.recastAbility(ability, units, targetObjectId = null, queued =
    false)`.
  - The initial dash return recast should validate the caster's active return marker server-side and
    ignore the command if the marker is missing, expired, same-tick blocked, or destination-invalid.
- Old-id decision:
  - Repurpose `ekatTeleport` for the new dash/return ability and `ekatLineShot` for the new
    out-and-back projectile behavior. Keeping these ids avoids command-card/protocol churn for the
    existing fun-test Ekat surface.
  - Do not preserve the old immediate teleport or immediate line-damage behavior as product
    semantics. Later phases should scrub the `Teleport` and `LineDamage` hooks once the replacement
    runtime paths exist.
- Projectile return-target contract:
  - Ekat line projectiles return toward Ekat's current server position each movement tick, not toward
    a fixed launch origin. If Ekat moves or dashes, the return leg may curve as it continuously
    retargets.
  - Projectile metadata should retain origin object id, caster id, launch tick, outbound endpoint,
    current leg, distance traveled, ticks out, and hit sets per leg so later tuning can scale damage
    by time, distance, or leg.
- Anchor targetability contract:
  - `magicAnchor` is an ability world object with owner, position, hp, radius, expiry tick, and
    destroyed-vs-expired reason. It should be targetable/damageable by enemy attacks through a
    narrow combat query path, but it must not become an ordinary production entity.
  - Anchors do not consume supply, do not enter production queues, do not appear in normal selection
    groups, do not path, do not score as units/buildings, and do not unlock entity commands.
  - Destroyed anchors trigger the 60-second placement lockout; naturally expired or replaced anchors
    do not.

## Implementation Map for Later Phases

- Phase 1 should edit first:
  - `server/crates/sim/src/game/mod.rs`
  - `server/crates/sim/src/game/systems.rs`
  - a new `server/crates/sim/src/game/ability_runtime.rs`
  - `server/crates/sim/src/game/ability.rs`
  - focused tests near the new runtime module and `Game` clone/tick behavior.
- Phase 2 should edit first:
  - `server/crates/contract/src/lib.rs`
  - `server/crates/protocol/src/lib.rs`
  - `server/src/protocol.rs` only if semantic conversion is needed
  - `client/src/protocol.js`
  - `docs/design/protocol.md`
  - `tests/protocol_parity.mjs` and compact snapshot tests in `tests/client_contracts.mjs`.
- Phase 3 should edit first:
  - `client/src/state.js`
  - `client/src/renderer/index.js`
  - `client/src/renderer/feedback.js` or a new focused renderer helper
  - `client/src/input/commands.js`
  - `tests/client_contracts.mjs`
  - `scripts/check-client-architecture.mjs` if a new client module needs classification.
- Phase 4 should edit first:
  - `server/crates/contract/src/lib.rs`
  - `server/crates/protocol/src/lib.rs`
  - `client/src/protocol.js`
  - `server/crates/sim/src/game/command.rs`
  - `server/crates/sim/src/game/services/commands.rs`
  - `client/src/hud_command_card.js`
  - `client/src/input/commands.js`.
- Phase 5 should edit first:
  - `server/crates/sim/src/game/hero_abilities.rs`
  - `server/crates/sim/src/game/services/ability_orders.rs`
  - the new ability runtime module
  - `server/crates/sim/src/game/services/standability.rs`
  - dash/return focused sim tests.
- Phase 6 should edit first:
  - the new projectile runtime module
  - `server/crates/sim/src/game/systems.rs`
  - `server/crates/sim/src/game/services/ability_orders.rs`
  - `server/crates/sim/src/game/services/combat/`
  - projectile movement/hit-dedupe tests.
- Phase 7 should edit first:
  - `server/crates/sim/src/game/hero_abilities.rs`
  - the new projectile runtime module
  - Ekat line-shot tests around outbound hits, return hits, moving Ekat retargeting, and old
    immediate damage removal.
- Phase 8 should edit first:
  - the new ability world object store
  - combat target acquisition/damage seams in `server/crates/sim/src/game/services/combat/`
  - projection/fog tests for anchor visibility and destruction lockout.
- Phase 9 should edit first:
  - Ekat line projectile launch code
  - anchor lookup in the ability runtime
  - client preview origins in `client/src/input/commands.js` and renderer preview code.
- Phase 10 should edit first:
  - `docs/design/server-sim.md`
  - `docs/design/protocol.md`
  - `docs/design/client-ui.md`
  - `docs/design/balance.md` if exposed ability metadata changes
  - regression tests covering removed immediate teleport/line-damage paths.
