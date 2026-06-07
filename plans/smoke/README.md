# Smoke Ability Plan

This plan adds a reusable ability system, then uses its targeted branch to give scout cars a Smoke
ability. Smoke is intended as an offensive tool: a player spends steel and oil to block line of
sight long enough for tanks and infantry to close on AT guns and other ranged threats.

The implementation should not hard-code the ability model around scout cars. Rifleman Charge is
already a self-activated cooldown ability and must share the same ability definition/cooldown
vocabulary where practical. Scout cars are the first world-point targeted carrier. Later phases
should be able to move Smoke to infantry, add other targeted or self-target abilities, or give
different units different ability sets without rewriting command routing, HUD targeting, or
cooldown projection.

## Confirmed Rules

- Smoke is a targeted ability with hotkey `D`.
- Scout cars can launch smoke at a target point up to 9 tiles away.
- Smoke costs 25 steel and 25 oil.
- Smoke cooldown is 20 seconds and starts when the grenade is launched.
- Resources are paid on launch.
- No resource reservation is added. If a scout car was ordered to move into launch range and the
  player no longer has enough resources at launch time, the server emits the appropriate
  `Not enough oil` or `Not enough steel` notice and the scout car idles.
- Smoke requires a completed Steelworks.
- If multiple selected, eligible scout cars are already in launch range, the server chooses the
  furthest in-range scout car from the target point.
- If no selected eligible scout car is in range, the server chooses the closest selected eligible
  scout car and moves it toward a launch point.
- Smoke supports Shift queue.
- Smoke appears instantly at the beginning of the launch. Projectile/canister visuals can be a
  later phase.
- Deployed smoke is neutral: it has no owner and is not associated with any player.
- Smoke radius is 2 tiles.
- Smoke duration is 5 seconds.
- Overlapping clouds expire independently.
- A smoke cloud is visible to a player only if that player has vision of it.
- Smoke blocks line of sight for combat and fog.
- Units inside smoke cannot be targeted.
- Friendly units inside smoke can still be seen.
- Enemy units inside smoke cannot be seen.
- Units inside smoke provide no vision outside the smoke.
- Smoke can target any map point in the first gameplay slice. Water/stone targeting restrictions
  and collision-free canister terrain validation are later work.

## Core Model

Add a generic ability layer rather than a `ScoutCarSmoke` special case.

- `AbilityKind` is the domain identity, covering existing `Charge` and new `Smoke`.
- Ability definitions own reusable data: carrier kinds, range, radius/effect parameters, cooldown,
  cost, tech requirement, target mode, and queue behavior.
- Ability definitions distinguish self-activated abilities such as Charge from world-point targeted
  abilities such as Smoke.
- Client commands for targeted abilities send ability intent: selected unit ids, ability kind,
  target point, and queued flag.
- The server validates and resolves the actual caster. The client never chooses the authoritative
  caster.
- Active/queued orders store ability intent and execution state separately, mirroring existing
  `Order` vs `OrderIntent`.
- Ability cooldown state lives on the carrier entity, keyed by ability kind, not in scout-car-only
  fields.
- Snapshot projection exposes owner-only ability cooldowns and available ability affordances without
  leaking enemy data.

## Phases

- [Phase 0 - Contracts and Test Matrix](phase-0-contracts.md)
- [Phase 1 - Reusable Targeted Ability Shell](phase-1-targeted-ability-shell.md)
- [Phase 2 - Smoke World Model and Authoritative LOS](phase-2-smoke-los.md)
- [Phase 3 - Smoke Command Execution and Queueing](phase-3-smoke-command-execution.md)
- [Phase 4 - Client UX and Rendering](phase-4-client-ux-rendering.md)
- [Phase 5 - Hardening, Docs, and Polish](phase-5-hardening-docs-polish.md)

## Non-Negotiable Invariants

1. Server authority stays intact. The client sends target intent only; the server owns tech,
   affordability, cooldown, caster choice, movement-to-range, smoke spawning, visibility, and combat
   effects.
2. Protocol mirrors stay synchronized. Any wire change updates `server/src/protocol.rs`,
   `client/src/protocol.js`, and `docs/design/protocol.md`.
3. Balance mirrors stay synchronized. Player-visible ability numbers update `server/src/config.rs`,
   `client/src/config.js`, and `docs/design/balance.md`.
4. `Game::tick()` stays panic-free. Non-finite coordinates, stale unit ids, dead casters, missing
   targets, expired smoke ids, and malicious command lists are ignored or rejected safely.
5. Fog remains authoritative. Smoke visibility, unit visibility inside smoke, event delivery, and
   combat target ids must not reveal hidden enemies.
6. Ability queues stay bounded. Shift-queued abilities must use existing queue caps or an equivalent
   hard cap.
7. Replays stay deterministic. Ability commands, caster resolution, launch timing, smoke expiration,
   and notices must replay from the command log.

## Suggested Implementation Order

Implement the phases in order. The first playable slice should prove the reusable targeted ability
path with Smoke, but avoid projectile visuals and terrain restrictions until LOS, fog, queueing, and
cooldowns are reliable.
