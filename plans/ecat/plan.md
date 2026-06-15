# Ekat Ability Systems Plan 0.1

## Purpose

Build a first-class complex ability foundation, using Ekat's fun-test kit as the product driver.
The target abilities are a dash that leaves a visible return marker and can later return to that
spot, an out-and-back line projectile whose damage can scale with travel time or distance, and a
Magic Anchor that persists on the ground, can be destroyed, and causes line projectiles to launch
from both Ekat and the anchor. Balance quality is not the goal of this version; the goal is a solid
server-authoritative system that can support more complex hero and world interactions without
turning every new ability into a bespoke tick-path branch.

## Product Requirements

- Ekat has a location-targeted dash. The dash leaves a return marker at the original position, the
  marker is visible only to players who can see that spot, and the return cannot be activated on the
  same tick or same instant as the dash.
- Ekat can activate the return while the return marker is active. Returning moves her to the marked
  original position only if the destination is still valid under the server's standability rules.
- Ekat has an out-and-back line projectile. It damages on the outbound pass and again while returning
  toward Ekat's current position, so the return path may curve if she moves or dashes after firing.
  The runtime keeps enough metadata for a later phase to scale damage by time out, distance traveled,
  or leg.
- Ekat can place a Magic Anchor on the ground. The anchor lasts 10 seconds, is projected through
  authoritative fog, and can be destroyed by enemies.
- If Ekat has an active anchor, the line projectile launches from both Ekat and the anchor toward
  the cursor. If the anchor is destroyed before expiry, the anchor placement ability is locked out
  for 60 seconds.
- This is a fun-test rollout. Phases should prefer clear mechanics, diagnostics, and architecture
  over polish, perfect tuning, or broad hero-design completeness.

## Phase Summaries

Phase 0 inventories the existing ability hooks, command paths, snapshot projection, client
targeting, and tests before changing behavior. It records the exact current one-off Ekat teleport
and line-shot seams, the smoke/shell world-state patterns that can be reused, and the protocol or
client architecture constraints that later phases must respect. The outcome is a narrow contract
for the shared runtime names, object kinds, and command semantics used by the remaining phases.

Phase 1 adds the server-side active ability runtime skeleton without exposing new gameplay. It
creates a deterministic store for ability instances and world objects, wires it into `Game` and the
tick pipeline, and proves expiry, ownership, ids, cloning, and panic-free stale-caster behavior with
focused tests. The outcome is a safe place for complex ability state that is not an entity, smoke
cloud, mortar shell, or artillery shell.

Phase 2 adds fog-filtered protocol projection for ability world objects. It defines a compact
snapshot shape for visible ability objects, mirrors it in Rust and JavaScript, and updates protocol
documentation without yet requiring the client to render polished art. The outcome is that return
markers, anchors, and later projectile visuals have one authoritative projection path instead of
ad hoc transient events.

Phase 3 adds the client-side ability object surface and preview foundation. It stores projected
ability objects in `GameState`, renders simple marker/anchor/debug visuals through the existing
renderer layer order, and allows ability-specific previews to draw multiple origins and path
shapes. The outcome is a client that can visualize server-projected ability state and richer
targeting previews without embedding gameplay authority.

Phase 4 defines reusable recast and per-caster ability-state semantics. It decides how a live
client asks for a second activation, validates active ability state server-side, and projects
owner-only affordances such as return availability, remaining lifetime, and lockout cooldowns. The
outcome is a general enough contract for dash-return and future two-stage abilities without
overloading missing `x`/`y` fields ambiguously.

Phase 5 implements the dash and delayed return marker using the new runtime. It scrubs the current
immediate teleport behavior and replaces it with a dash that creates a return marker, blocks instant
snapback, and returns only while the marker is active and the destination is standable. The outcome
is the first product ability proving ability objects, recast semantics, cooldowns, and client
rendering work together.

Phase 6 adds a generic moving projectile runtime for ability-owned hit volumes. It supports
outbound and returning line projectiles, per-leg hit dedupe, owner/team filtering, travel metadata,
and fog-safe visual events or object projection. The outcome is a reusable projectile system that
can support Ekat's line projectile without hard-coding damage into command acceptance.

Phase 7 implements Ekat's out-and-back line projectile on top of the projectile runtime. It launches
from Ekat toward the target point, turns around at its endpoint, then returns toward Ekat's current
position so the return path can curve as she moves. The projectile damages valid enemies on both legs
and records enough metadata for distance or time-out damage scaling; the old immediate line-damage
hook is scrubbed rather than kept as a product path.

Phase 8 implements Magic Anchor placement as a persistent, destructible ability world object. It
lasts 10 seconds, projects through fog, can take enemy damage or destruction events according to
the phase's targetability contract, and applies a 60-second placement lockout only when destroyed
rather than naturally expired. The outcome is a second product object proving lifetime,
destructibility, owner state, and lockout rules.

Phase 9 composes Magic Anchor with the line projectile. It makes line projectile activation query
Ekat's active anchor, launch a second projectile from the anchor toward the same cursor point, and
return both projectiles toward Ekat's current position. It renders client previews for both origins
before the command is sent, proving the systems can combine independent world objects and projectiles.

Phase 10 consolidates docs, tests, diagnostics, and old special-case cleanup. It updates the
server-sim, protocol, client-ui, and balance design docs to describe the new ability runtime,
verifies the obsolete one-off Ekat teleport and line-damage paths are gone, and adds regression
coverage for the highest risk replay, fog, and client-command cases. The outcome is a shippable 0.1
foundation with clear follow-up hooks for tuning, art, sound, AI awareness, and future hero abilities.

## Phase Index

1. [Phase 0 - Inventory and Runtime Contract](phase-0.md)
2. [Phase 1 - Server Ability Runtime Skeleton](phase-1.md)
3. [Phase 2 - Ability Object Projection Protocol](phase-2.md)
4. [Phase 3 - Client Ability Object Surface](phase-3.md)
5. [Phase 4 - Recast and Per-Caster State Contract](phase-4.md)
6. [Phase 5 - Dash Return Ability](phase-5.md)
7. [Phase 6 - Moving Projectile Runtime](phase-6.md)
8. [Phase 7 - Ekat Out-and-Back Line Projectile](phase-7.md)
9. [Phase 8 - Magic Anchor Lifecycle](phase-8.md)
10. [Phase 9 - Anchor and Projectile Composition](phase-9.md)
11. [Phase 10 - Cleanup, Docs, and Regression Coverage](phase-10.md)

## Overall Constraints

- Keep the server authoritative. The client may preview and animate, but command success,
  projectile hits, marker lifetime, anchor destruction, cooldowns, and lockouts are resolved by the
  simulation.
- Preserve fog privacy. Return markers, anchors, projectiles, hit events, destruction events, and
  positioned notices must be projected only to recipients allowed to see the relevant position or
  owner-only state.
- Keep `Game::tick()` panic-free. Stale caster ids, expired ability objects, missing anchors,
  destroyed targets, invalid landing points, and malformed commands must be no-ops or notices, not
  panics.
- Do not turn `AbilityEffectHook` into a generic script engine. Add typed runtime data and narrow
  execution helpers with explicit tests rather than stringly ability scripts.
- Keep `rules::faction` the source of ability metadata: ids, labels, command-card visibility,
  carriers, ranges, cooldowns, charges, costs, queueability, and compact codes.
- Keep wire protocol mirrors aligned. Any snapshot, event, command, or compact transport change must
  update `server/crates/contract`, `server/crates/protocol`, `server/src/protocol.rs` if needed,
  `client/src/protocol.js`, and `docs/design/protocol.md`.
- Keep client architecture boundaries. Prefer dependency injection through `Match` and focused
  renderer/input collaborators; do not add broad cross-area imports to make the ability UI quick.
- Keep ability world objects distinct from normal entities unless a phase explicitly decides an
  object needs full entity behavior. Anchors may be targetable/destructible, but they should not
  silently inherit supply, production, selection, pathing, or scoring semantics.
- Keep phases small. If a phase discovers it needs a protocol redesign, generated config system, or
  broad combat retargeting rewrite, stop and update the plan instead of smuggling that work into the
  phase.
- Preserve existing Kriegsia gameplay unless a phase explicitly touches shared systems. Ekat changes
  should be gated by the Ekat faction catalog and should not grant new ability affordances to other
  factions.
- Replays and dev-watch flows matter. Ability runtime state must clone deterministically for replay
  keyframes, and new snapshot fields should behave sensibly for normal player, spectator, full-world,
  and replay snapshots.
- Local-player prediction is out of scope for this 0.1 plan unless a phase explicitly calls it out.
  Do not attempt to predict dash/projectile/anchor state in WASM as part of this rollout.
- AI strategic use is out of scope. The systems should not prevent later AI support, but live AI
  does not need to use Ekat abilities during this plan.
- Art, sound, and balance polish are out of scope beyond simple readable placeholders and factual
  patch notes.

## Implementation and Handoff Rules

Implement one phase at a time. Each phase should be committed, merged to `main`, and pushed before
the next phase begins. When a phase is complete, mark that phase document as done in the same
implementation commit.

After each phase, the implementing agent must provide a handoff message describing what the next
agent should do and what should be manually tested. Manual testing notes should cover the core
features for that phase, not an exhaustive test matrix.
