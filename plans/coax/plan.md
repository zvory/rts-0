# Tank Coaxial Machine Gun Multi-Phase Plan

## Purpose

Implement the Tank coaxial machine gun described in [requirements.md](requirements.md) through
behavior-preserving refactors before any live coax firing is added. The first refactor turns current
single-attack assumptions into rules-owned weapon profiles. The second refactor turns target facts
and priority ranking into reusable policy surfaces so the coax can use machine-gun-like targeting
without inheriting Tank cannon priorities.

This plan is ready for executor implementation. Run one phase at a time from a clean worktree and do
not start a later phase until the previous phase PR has merged to `origin/main`.

## Product Decisions Locked For Implementation

- Coax weapon id: `tank_coax`.
- Tank main cannon weapon id: `tank_cannon`.
- Attack-event weapon field: `weaponKind` on JSON/JS, represented as `weapon_kind` in Rust.
- Coax range: 6 tiles.
- Coax damage: 4 small-arms damage.
- Coax cooldown: 6 ticks, independent from the Tank cannon cooldown.
- Coax arc: 10 degrees on either side of the current authoritative Tank turret/weapon facing.
- Coax targeting policy: infantry-priority targets first, then fallback legal targets.
- Coax infantry-priority group for this implementation: Worker, Rifleman, Machine Gunner, and
  future Panzerfaust-style infantry when that unit exists.
- Mortar Teams, Artillery, Anti-Tank Guns, Ekat, Golems, vehicles, buildings, resources, and field
  obstacles are not infantry-priority targets for the coax.
- Fallback legal targets may include vehicles, buildings, support weapons, and field obstacles when
  ordinary direct-fire legality allows them. Resource nodes are never legal coax targets.
- Ties inside the same material priority use current deterministic style: distance first, then id.

If implementation discovers a conflict between these decisions and [requirements.md](requirements.md),
stop as blocked and ask for a product decision instead of inventing new behavior.

## Phase Summaries

### [Phase 1 - Weapon Profile Foundation](phase-1.md)

Add a rules-owned weapon-profile vocabulary and move current default attacks behind it without
changing gameplay. Current helpers such as `attack_profile(kind)` and `weapon_class(kind)` should
remain behavior-compatible wrappers over each entity kind's default weapon. The phase ends with
parity tests proving every current attack value and weapon class is unchanged.

### [Phase 2 - Weapon-Aware Damage And Overpenetration](phase-2.md)

Thread default weapon identity through direct-fire damage and overpenetration while every current
attacker still fires only its default weapon. Damage class, miss policy, armor-facing modifiers,
overpenetration depth, attribution, firing reveal, and under-attack notices must remain equivalent
to current mainline. This phase removes the key blocker where a future Tank coax would otherwise
inherit Tank AP damage.

### [Phase 3 - Weapon Cooldown State](phase-3.md)

Replace the single combat cooldown slot with a weapon-aware interface while preserving default
weapon behavior for all current combatants. Existing callers may keep compatibility shims, but the
Tank must be able to hold independent `tank_cannon` and later `tank_coax` cooldowns. This phase
does not add protocol fields, client feedback, target policy, or live coax firing.

### [Phase 4 - Attack Event Weapon Identity Plumbing](phase-4.md)

Add optional attack-event weapon identity across semantic Rust DTOs, compact snapshots, JS protocol
decoding, and fallback client feedback. Existing attacks should emit their default weapon ids, while
clients render and play them exactly as before when the hint is missing or default. This phase is a
contract/plumbing phase and must not add live coax behavior.

### [Phase 5 - Target Facts And Direct-Fire Legality](phase-5.md)

Create rules-owned target classification facts and extract reusable direct-fire legality helpers
without changing target acquisition results. Current hard-coded facts such as armored, support
weapon, field obstacle, resource node, vehicle body, and infantry-priority eligibility should become
explicit data that target policies can consume. The phase must preserve existing smoke, fog, LOS,
friendly-hard-blocker, route-obstruction, and resource-node behavior.

### [Phase 6 - Declarative Target Priority Policies](phase-6.md)

Move current target ranking into named priority policies that preserve mainline behavior for every
existing attacker. Add an unused or test-only machine-gun-like policy that ranks coax
infantry-priority targets ahead of fallback legal targets, with distance/id ties and no Tank cannon
threat ordering. The phase ends with current targeting parity plus pure policy tests for the future
coax policy.

### [Phase 7 - Tank Coax Server Runtime](phase-7.md)

Add the `tank_coax` weapon profile and server-authoritative secondary firing path for Tanks. The
coax uses the current turret facing, its own cooldown, the machine-gun-like priority policy, direct
fire legality, and small-arms overpenetration, without rotating the turret, chasing, changing
cannon target intent, or stealing pathing state. Existing Tank cannon behavior remains the
regression baseline.

### [Phase 8 - Coax Client Feedback And Tank Rig](phase-8.md)

Teach client feedback to route audio, muzzle flashes, tracers, overpenetration tails, and recoil by
weapon identity instead of attacker kind alone. Tank cannon shots keep cannon-scale feedback, while
`tank_coax` shots use machine-gun sound, small feedback, no cannon recoil, and a coax muzzle anchor
beside the main barrel. Legacy/default events without weapon hints must continue to degrade safely.

### [Phase 9 - Docs, Data Surfaces, And Integration Hardening](phase-9.md)

Align design docs, generated stats/wiki surfaces if needed, focused integration tests, and manual
playtest scaffolding with the finished feature. This phase audits the implementation against every
requirements bullet, closes protocol/fog/replay/client regressions, and records factual patch-note
bullets. No new balance tuning belongs here unless the user explicitly approves it.

## Overall Constraints

- Preserve behavior through Phases 1-6 except for explicitly documented internal APIs and protocol
  fields in Phase 4.
- Keep server authority. The server owns weapon profiles, target legality, priority, cooldowns,
  damage class, overpenetration, fog-gated events, and attribution.
- Keep the wire protocol mirrored across `server/crates/protocol/src/lib.rs`,
  `server/src/protocol.rs`, `client/src/protocol.js`, compact metadata, JS constants, and
  `docs/design/protocol.md` whenever Phase 4 or later protocol work changes event shape.
- Keep balance mirrored only where values are client-visible. Server-only damage policy and
  targeting policy should remain Rust-owned unless the client needs data for rendering or visible
  reference surfaces.
- Keep `Game::tick()` panic-free. Stale ids, dead targets, non-finite facings, invalid targets,
  hidden targets, blocked shots, missing cooldown entries, and missing event recipients are no-ops.
- Maintain fog guarantees. Weapon identity may identify the shot class needed for feedback, but it
  must not reveal hidden target facts, damage, arc decisions, or muzzle coordinates beyond existing
  attack-event projection.
- Separate firing entity identity from weapon identity. Once the refactors land, do not use
  `EntityKind::Tank` alone to decide whether a shot is AP, what cooldown it uses, which feedback it
  triggers, or what overpenetration factor it uses.
- Coax does not add a command, toggle, upgrade, research, command-card affordance, cost/supply/sight
  change, trainability change, or primary Tank range-display change.
- Coax does not rotate the turret, move the Tank, chase targets, clear paths, or replace explicit
  player attack intent. It only fires opportunistically through the current authoritative turret
  facing.
- Coax shots overpenetrate with small-arms damage. They must not use Tank AP damage, Tank cannon
  armor-facing multipliers, Tank cannon sound, large muzzle flash, or cannon recoil.
- Existing Tank cannon behavior is the baseline: target selection, cooldown, turret rotation,
  stationary range ramp, direct-fire overpenetration, audio, visuals, and recoil should continue to
  work as they do today.

## Executor Workflow

- Each phase must be implemented in its own clean `/tmp/rts-worktrees` worktree on a `zvorygin/`
  branch.
- Each phase must be committed, pushed, opened as an owned PR, and have auto-merge armed with
  `scripts/agent-pr.sh`.
- After opening each phase PR, the implementing agent must run `scripts/wait-pr.sh <pr>` and wait
  until GitHub reports the PR merged and the head SHA is reachable from `origin/main` before
  reporting the phase complete or starting the next phase.
- When a phase is complete, mark that phase document as done in that phase's implementation commit.
- After each phase, the implementing agent must provide a handoff message for the next phase. The
  handoff must name the completed behavior, changed files/contracts, verification commands, known
  risks, and the core manual gameplay checks the next agent should run.
- Manual test notes should cover the core feature risks for that phase, not an exhaustive matrix.
