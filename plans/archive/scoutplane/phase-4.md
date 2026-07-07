# Phase 4 - Upkeep, Dismissal, Fog, And Projection

## Phase Status

Status: done.

## Objective

Complete the hidden authoritative server behavior that makes Scout Plane strategically meaningful
and fog-safe. Add oil upkeep, fuel reserve handling, manual/automatic dismissal, aerial fog stamping,
and projection rules before any normal player can train the plane.

## Scope

- Read [docs/context/server-sim.md](../../../docs/context/server-sim.md) and
  [docs/context/protocol.md](../../../docs/context/protocol.md) before changing fog, projection,
  snapshots, events, checkpoint, replay, or commands.
- Implement oil upkeep:
  - Upkeep starts immediately when the plane launches.
  - Spend 1 Oil every 20 ticks while active.
  - If the owner has enough Oil when upkeep is due, spend Oil and keep the fuel tank full.
  - If the owner has zero Oil when upkeep is due, drain the fuel reserve for unpaid upkeep time.
  - If Oil becomes available before the reserve reaches zero, spend available Oil rapidly enough to
    refill the reserve and continue flying.
  - If the fuel reserve reaches zero, dismiss the plane automatically.
  - The first implementation does not need a warning before fuel dismissal.
- Implement dismissal:
  - Manual dismiss removes the plane and stops oil upkeep.
  - Automatic fuel dismissal removes the plane and stops oil upkeep.
  - Dismissal is safe for stale ids, already removed planes, dead owners, and replay/checkpoint
    restore paths.
- Enforce one active hidden plane per player at the server runtime level. If Phase 6 adds production
  admission enforcement, this phase should still prevent runtime duplicates from breaking upkeep or
  projection.
- Implement authoritative aerial vision:
  - Scout Plane grants owner/team fog vision from its current position.
  - Scout Plane sight uses the approved 12-tile radius.
  - Scout Plane sight ignores terrain and building line-of-sight blockers.
  - Scout Plane sight still respects smoke where practical.
  - Sight updates as the plane moves and orbits.
- Implement fog-safe projection:
  - The owner sees the active plane while active.
  - Enemy players see the plane only when it is inside their current visibility.
  - Enemy projection does not expose owner resources, queued private commands, hidden target data,
    fuel reserve internals unless intentionally public, or hidden positions.
  - Spectator, replay, lab, and observer analysis projection follow the same visibility principles
    as other authoritative world objects.
- Update docs/design contracts in the same phase if fog, projection, protocol, checkpoint, or replay
  contracts change.
- Do not expose normal City Centre production, command-card UI, final rendering, or audio in this
  phase.

## Expected Touch Points

- `server/crates/sim/src/game/fog.rs`
- `server/crates/sim/src/rules/projection.rs`
- `server/crates/sim/src/game/snapshot.rs`
- `server/crates/sim/src/game/systems.rs`
- `server/crates/sim/src/game/services/*.rs`
- `server/crates/sim/src/game/state.rs`
- `server/crates/sim/src/game/checkpoint*.rs`
- `server/crates/sim/src/game/replay*.rs`
- `server/crates/protocol/src/lib.rs`
- `server/crates/protocol/src/compact_snapshot.rs`
- `server/src/protocol.rs`
- `client/src/protocol.js`
- `client/src/protocol_snapshot.js`
- `docs/design/server-sim.md`
- `docs/design/protocol.md`
- `docs/design/balance.md` if upkeep constants become a rules-owned visible surface
- `docs/projection-audit-checklist.md` only if the checklist itself needs updating

## Edge Cases To Cover

- Upkeep begins on launch, not while queued in production.
- A player with enough Oil pays integer upkeep and keeps a full reserve.
- A player at zero Oil drains fuel and auto-dismisses only after the 5-second reserve expires.
- Oil income during fuel drain refills the plane and prevents dismissal.
- Manual dismiss stops future upkeep immediately.
- Automatic dismissal cleans up the active-limit slot and projection state.
- A disconnected or defeated owner does not make upkeep or dismissal panic.
- A hidden duplicate plane, if created by a test or lab path, cannot corrupt player resources or fog.
- Plane vision reveals through terrain/building blockers but remains blocked by smoke per the chosen
  smoke policy.
- Enemy snapshots include the plane only when visible and hide private plane state.
- Replay, checkpoint restore, lab vision modes, spectators, and observer analysis remain fog-safe.

## Verification

- Focused Rust tests for oil spending cadence, reserve drain, reserve refill, auto-dismiss, manual
  dismiss, active-limit cleanup, and resource accounting.
- Focused Rust fog tests for aerial sight through terrain/building blockers and smoke blocking.
- Focused projection tests for owner visibility, enemy visible/invisible cases, hidden private state,
  replay/checkpoint restore, lab, spectator, and observer analysis behavior.
- Protocol tests if new snapshot fields or command/event vocabulary are added.
- `node tests/protocol_parity.mjs` if protocol vocabulary changes.
- `cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture` if sim
  service boundaries or state ownership change.
- `git diff --check`.

## Manual Test Focus

If a hidden dev or lab scenario exists, inspect one plane revealing across a wall of blockers,
through open terrain, and near smoke. Drain the owner's Oil, watch the plane auto-dismiss after the
reserve expires, then repeat with a Pump Jack producing Oil and confirm it stays active.

## Handoff Expectations

Name the upkeep state owner, dismissal command/path, fuel accounting rules, fog stamping helper,
smoke interpretation, and projection policy. Call out whether Phase 5 needs to display any public
fuel/upkeep state or whether the first client pass should only support move and dismiss controls.
