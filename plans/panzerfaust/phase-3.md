# Phase 3 - Hidden Server Runtime And Conversion

## Phase Status

Status: done.

## Objective

Implement the server-authoritative Panzerfaust one-shot runtime for spawned Panzerfaust entities
while keeping the unit hidden from normal production. The feature should be testable through
simulation helpers or controlled lab/dev setup, but ordinary players still should not train it.

## Scope

- Add authoritative loaded Panzerfaust combat behavior:
  - Legal targets are visible enemy Tanks only.
  - Direct Attack on a non-Tank target is invalid while loaded.
  - Attack Move, Idle, and Hold Position may acquire legal Tanks.
  - Hold Position must not chase outside current range.
  - Plain Move orders must not auto-interrupt into Panzerfaust firing.
- Centralize Tank-only target legality so command validation, queued attack promotion, explicit
  ordered attacks, idle acquisition, Attack Move acquisition, and any lab/dev issue path all use the
  same rule. Existing ordered attack paths can bypass acquisition filters, so do not rely on
  acquisition-only filtering.
- Implement firing state:
  - Stop movement before firing.
  - 15-tick windup.
  - 15-tick projectile travel.
  - 15-tick post-fire recovery.
  - Methamphetamines reduces windup and recovery to the approved rounded timing while leaving
    travel unchanged.
  - Loaded Panzerfaust cannot move during windup or recovery.
- Implement shot consumption and cancellation:
  - Replacing or canceling the order during windup cancels the shot without spending it.
  - If the target leaves range or visibility during windup, do not spend the shot.
  - After launch, the shot is spent even if the order changes.
  - If the launched target dies before impact, do no damage and emit only fog-safe feedback defined
    by Phase 2.
- Prefer a small targeted projectile/recovery state for Panzerfaust rather than reusing sweeping
  ability projectile code. The runtime should launch at one legal target, apply flat damage only to
  that live target Tank at impact, then recover and convert.
- Implement damage:
  - 60 armor-piercing direct damage to Tanks.
  - No area damage, friendly fire, hull-facing multiplier, reload, or second Panzerfaust attack.
- Implement same-id conversion into a Rifleman after recovery:
  - Preserve entity id, owner, team, position, facing where sensible, current HP, selection/control
    continuity, current legal queued orders, trench occupation where possible, and normal death
    cleanup behavior.
  - After conversion, normal Rifleman stats, attacks, movement, Methamphetamines behavior, and
    command legality apply.
  - Do not refund the premium cost on conversion.
  - Clear Panzerfaust-only state, including armed shot flags, projectile/recovery timers,
    Panzerfaust target ids, loaded-only cooldowns, range modifiers, and any event state that should
    not survive as Rifleman state.
- Keep Panzerfaust out of normal production and AI training in this phase.
- Add focused simulation tests for spawned Panzerfaust entities.
- Update server-sim and protocol docs in the same phase if runtime state, order semantics, events,
  projection, or conversion behavior becomes a current contract.

## Expected Touch Points

- `server/crates/sim/src/game/entity/entity.rs`
- `server/crates/sim/src/game/entity/order.rs`
- `server/crates/sim/src/game/entity/state.rs`
- `server/crates/sim/src/game/services/combat/*.rs`
- `server/crates/sim/src/game/services/order_queue.rs`
- `server/crates/sim/src/game/services/order_execution.rs`
- `server/crates/sim/src/game/services/movement/*.rs`
- `server/crates/sim/src/game/services/death.rs`
- `server/crates/sim/src/game/services/entrenchment.rs`
- `server/crates/sim/src/game/snapshot.rs`
- `server/crates/sim/src/rules/projection.rs`
- `server/crates/rules/src/combat.rs`
- `server/crates/rules/src/balance/*.rs`
- `docs/design/server-sim.md`
- `docs/design/protocol.md`
- `docs/projection-audit-checklist.md` if projection policy changes

## Edge Cases To Cover

- Target dies during windup: shot is not spent.
- Target dies during travel: shot is spent, no damage, recovery and conversion still complete.
- Target leaves visibility or range during windup: shot is not spent and the current order resolves
  safely.
- Target leaves visibility after launch: damage resolution and feedback stay consistent with the
  Phase 2 fog policy.
- Direct Attack on Riflemen, Machine Gunners, Workers, buildings, Tank Traps, Scout Cars, and
  hidden Tanks is invalid while loaded.
- Queued direct attacks on non-Tanks are rejected or skipped by the same legality helper used for
  immediate direct attacks.
- Attack Move can fire after reaching engagement range; plain Move does not fire opportunistically.
- Queued post-shot Move or Attack Move continues after conversion when valid for Rifleman.
- Queued Tank-specific attack does not remain stuck after conversion if the Rifleman cannot attack
  it under normal rules.
- Conversion preserves same id and visible continuity for selection/control groups.
- Entrenched loaded Panzerfaust uses 4-tile range only while actively occupying a trench.
- `Game::tick()` remains panic-free with stale ids, deleted targets, interrupted orders, and bad
  timing state.

## Verification

- Focused Rust tests for Panzerfaust direct attack legality, attack-move acquisition, Hold Position
  behavior, Move non-autofire, windup cancellation, launched-shot consumption, target death during
  travel, damage, conversion, queue continuation, Methamphetamines timing, Entrenchment range, and
  death cleanup.
- Focused fog/projection tests for every new event or snapshot field emitted by the runtime.
- Existing focused combat/order queue tests affected by target legality or conversion.
- `cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture` if sim
  service boundaries or allowlists change.
- `node tests/protocol_parity.mjs` if event/protocol fields are emitted or adjusted.
- `git diff --check`.

## Manual Test Focus

Use the controlled spawn path named in the Phase 2 handoff if one exists. Inspect one spawned
Panzerfaust firing at a visible Tank, one canceled windup, one target death during travel, and one
conversion into a Rifleman; normal Barracks production should still not expose the unit.

## Handoff Expectations

Name the runtime state fields, conversion helper, event emission path, and focused test names. Tell
Phase 4 exactly which snapshot fields or events the client must render, which states are intentionally
server-only, and any runtime behavior that remains hard to inspect without client support.
