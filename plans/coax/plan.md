# Tank Coaxial Machine Gun Implementation Plan

## Purpose

Implement the tank coaxial machine gun requirements in [requirements.md](requirements.md) without
bolting a second weapon onto assumptions that every entity has one attack profile. This document is
a rough architecture roadmap after adversarial review, not an executor-ready phase list. Future
planning agents should flesh the roadmap into revised phase files before implementation begins.

The core shape is:

1. Build a real weapon-profile system and move existing attacks onto it without changing gameplay.
2. Build a real target classification and priority-policy system, with user review before code.
3. Implement the Tank coax as one weapon profile using those systems, then add client feedback,
   docs, and final hardening.

Do not run `scripts/phase-runner.sh --plan coax ...` against the current phase files until a future
planning pass updates them to match this roadmap. The existing `phase-*.md` files are historical
drafts and useful scaffolding, but they still assume too much can be resolved inside the runtime
phase.

## Product Input

- [requirements.md](requirements.md) is the active product requirement source.
- Coax shots must overpenetrate. They use the same direct-fire overpenetration system as other
  direct shots, but with coax small-arms damage and coax feedback scale.
- User decisions captured during review:
  - Beta may pass through temporarily broken middle states during serial implementation, but final
    behavior must meet the requirements.
  - The coax is fixed to the main cannon/turret direction. The main cannon owns where the turret
    points; the coax only fires opportunistically at legal targets inside the current turret arc.
  - Coax targeting should have separate ranking from the Tank cannon. It may reuse direct-fire
    safety/legality checks, but not Tank cannon priority rules.
  - Riflemen, Machine Gunners, Workers, and future Panzerfaust-style infantry should be
    infantry-priority targets for machine-gun-like policies. Mortar Teams and other support weapons
    should not count as infantry-priority. Ekat and Golems are not part of the first coax decision.

## Architecture Groups

### Group A - Weapon Profile Foundation

Create a rules-owned weapon-profile abstraction that is useful beyond the Tank coax. A weapon
profile should carry stable weapon identity, damage class, base damage, range, cooldown, miss/facing
policy, overpenetration policy, event identity, and enough metadata for client feedback routing when
that identity reaches the wire. Current unit and building attacks should move onto default weapon
profiles in behavior-preserving steps.

This group should answer questions such as:

- What is the stable profile id vocabulary? For example, `rifleman_rifle`, `machine_gunner_mg`,
  `scout_car_mg`, `tank_cannon`, and later `tank_coax`.
- Which fields belong to a weapon profile versus an entity kind?
- Which current behavior is genuinely weapon-specific, such as AP damage, small-arms damage,
  overpenetration depth, armor-facing multipliers, miss policy, cooldown, and feedback?
- How does an entity expose one or more weapons without changing current one-weapon behavior?

Expected outcome: current gameplay is unchanged, but damage, cooldowns, events, and feedback can
refer to the weapon that fired instead of inferring everything from `EntityKind`.

### Group B - Target Classification And Priority Policy

Create a rules-owned target classification and priority-policy system before implementing live coax
targeting. This must not be a pile of per-weapon match statements. Target traits should live on the
target, while weapons choose from declarative priority policies that can be reused or specialized.

Think in these separate concepts:

- **Weapon profile:** what fired and how it deals damage.
- **Target classification:** what the target is, such as infantry, support weapon, light vehicle,
  armored vehicle, building, field obstacle, economy unit, or anti-armor threat.
- **Priority policy:** how a weapon ranks already-legal targets, such as ordinary small-arms,
  machine-gun-like, anti-armor, artillery/indirect, field-obstacle breach, or an idiosyncratic
  weapon-specific policy.
- **Activation constraint:** whether this weapon is allowed to consider a target at all, such as
  current turret arc, setup arc, minimum range, smoke/LOS, direct attack order, or no-chase passive
  fire.

This group is product/design gated. An implementation agent must stop as blocked and ask the user
clarifying questions before coding the priority-policy system unless a later plan already records
the answers. Do not infer the final model from this rough plan.

Questions the user should be asked include:

- Which target tags do we want now, and which are likely soon?
- Which current target behaviors are true reusable policies versus one-off exceptions?
- Should priority policies be first-matching ordered rules, scoring tuples, bucketed ranking, or
  another declarative shape?
- How should "attacking me", anti-armor threat, current target retention, distance, buildings,
  scout cars, Tank Traps, support weapons, and explicit player attack orders interact?
- Which policies should machine-gun-like weapons share, and where should the Tank coax differ only
  because it is locked to the current turret arc?

Expected outcome: existing targeting remains behavior-preserving after migration, and the coax can
use a machine-gun-like policy constrained by the Tank's current turret arc.

### Group C - Tank Coax Runtime

After Groups A and B land, implement the server-authoritative coax. The coax should be a secondary
Tank weapon profile with 6-tile range, 4 small-arms damage, 6-tick cooldown, independent cooldown
state, direct-fire legality checks, direct-fire overpenetration, and attack events carrying the
coax weapon identity.

Runtime constraints:

- The main cannon owns turret direction and normal Tank attack intent.
- The coax never rotates the turret, changes cannon target, requests chase paths, clears paths, or
  changes movement intent.
- The coax only evaluates targets inside the current authoritative turret/weapon facing arc.
- The coax should use the machine-gun-like priority policy from Group B, with fallback legal targets
  only when no infantry-priority target is legal inside the arc.
- Coax damage is small-arms damage and must not inherit Tank cannon AP behavior or Tank cannon
  armor-facing multipliers.

### Group D - Client Feedback, Docs, And Hardening

Once runtime behavior exists, teach the client to render and play coax shots from weapon identity.
Tank cannon shots keep cannon sound, large muzzle flash, tracer scale, and recoil. Tank coax shots
use machine-gun sound, small muzzle flash/tracer scale, no cannon recoil, and a small coax barrel or
muzzle anchor beside the main gun.

Finish by updating design docs, generated stats/wiki surfaces if needed, focused regression tests,
manual scenario coverage, and final patch notes.

## Draft Phase Buckets

The existing `phase-*.md` files need a follow-up planning pass to align with these buckets. A good
future split is likely:

1. Weapon profile vocabulary and default-profile parity.
2. Weapon-aware damage, overpenetration, cooldown, and event plumbing.
3. User-gated target classification and priority-policy design.
4. Target classification and priority-policy implementation, preserving current behavior.
5. Tank coax server runtime.
6. Coax client feedback and Tank rig update.
7. Docs, generated data surfaces, integration tests, and playtest hardening.

The exact boundaries should be chosen after reading the current combat code and after the user
answers the target-policy questions in Group B.

## Existing Draft Phase Summaries

### [Phase 1 - Rules Weapon Profile Skeleton](phase-1.md)

Introduce rules-owned weapon profile identity while preserving every current unit and building
attack number. Existing callers may continue asking for the default attack profile by entity kind,
but the underlying rules surface should be able to name the weapon profile and its weapon class.
This phase must not change target acquisition, damage, cooldowns, events, client feedback, or
gameplay.

### [Phase 2 - Weapon-Aware Damage Refactor](phase-2.md)

Thread weapon profile identity through direct-fire damage and overpenetration while passing each
existing attacker its default weapon. Effective damage, miss policy, facing modifiers, attribution,
under-attack notices, firing reveal, and overpenetration should produce the same results as before.
This phase removes the hard dependency on attacker kind for AP versus small-arms damage without
introducing the coax yet.

### [Phase 3 - Weapon Cooldown And Event Plumbing](phase-3.md)

Replace the single hard-coded combat cooldown path with a weapon-aware shape that still stores and
uses only the default weapon for all current entities. Extend attack events and compact snapshot
decoding with optional weapon identity so clients can distinguish future Tank cannon and Tank coax
shots, while rendering and playing existing shots exactly as they do today. This phase is a contract
and plumbing phase, not a gameplay feature.

### [Phase 4 - Tank Coax Server Runtime](phase-4.md)

Add the Tank coax weapon profile and server-authoritative firing path. The coax uses 6-tile range,
4 small-arms damage, 6-tick cooldown, independent cooldown state, the current turret/weapon facing
arc, direct-fire legality checks, and direct-fire overpenetration. It must not rotate the turret,
chase targets, steal explicit cannon intent, or change existing cannon targeting, stationary range,
cooldown, firing reveal, and overpenetration behavior.

### [Phase 5 - Coax Client Feedback And Tank Rig](phase-5.md)

Teach the client to use attack-event weapon identity for combat audio, muzzle flashes, tracers, and
recoil. Add a small coax barrel to the Tank rig and make coax shots originate from that barrel with
machine-gun-scale feedback, while Tank cannon shots continue to use the existing cannon sound,
muzzle flash, tracer, and recoil. This phase should keep legacy/default attack events safe through
fallbacks.

### [Phase 6 - Stats, Docs, And Data Surface](phase-6.md)

Update design docs, generated stats/wiki behavior where applicable, and visible data surfaces so
the new secondary weapon is documented without changing Tank command cards, cost, supply, sight, or
trainability. Align the product requirements, protocol docs, balance docs, server-sim docs, and
client docs around the final weapon-profile and coax contracts. This phase should collect factual
patch-note bullets and close any documentation drift left by the implementation phases.

### [Phase 7 - Integration And Playtest Hardening](phase-7.md)

Add final focused regression coverage and manual playtest scaffolding for the fully integrated
feature. Verify that current cannon behavior remains intact, coax behavior works against infantry
and fallback targets, fog projection stays safe, and client feedback matches the weapon actually
fired. This phase should also resolve any small follow-up issues found by CI, replay review, or
manual inspection.

## Phase Index

These files are draft scaffolding and must be rewritten or reconciled with the architecture groups
above before executor automation is used.

1. [Phase 1 - Rules Weapon Profile Skeleton](phase-1.md)
2. [Phase 2 - Weapon-Aware Damage Refactor](phase-2.md)
3. [Phase 3 - Weapon Cooldown And Event Plumbing](phase-3.md)
4. [Phase 4 - Tank Coax Server Runtime](phase-4.md)
5. [Phase 5 - Coax Client Feedback And Tank Rig](phase-5.md)
6. [Phase 6 - Stats, Docs, And Data Surface](phase-6.md)
7. [Phase 7 - Integration And Playtest Hardening](phase-7.md)

## Overall Constraints

- Keep [requirements.md](requirements.md) as the product behavior source for this plan. If an
  implementation phase discovers a product conflict, stop as blocked instead of inventing new
  gameplay.
- Preserve behavior through phases 1, 2, and 3. Those phases may change internal APIs and mirrored
  protocol shapes when explicitly scoped, but current units, damage, cooldowns, events, audio,
  visuals, targeting, overpenetration, and replay behavior must stay equivalent.
- Keep server authority. The server owns weapon profiles, target legality, target priority,
  cooldowns, damage classification, overpenetration, fog-gated events, and attribution. The client
  may render and play feedback only from projected state.
- Separate firing entity identity from weapon identity. Do not use `EntityKind::Tank` alone to
  decide whether a shot is AP, what cooldown it uses, what feedback it triggers, or what range it
  has once the refactor phases land.
- Keep `Game::tick()` panic-free. Stale ids, dead entities, non-finite positions/facings, invalid
  targets, hidden targets, blocked shots, and missing event recipients must be safe no-ops.
- Maintain fog guarantees. Weapon hints may identify the shot class needed for rendering/audio, but
  must not reveal hidden target data beyond current attack-event projection rules.
- Coax does not add a command, toggle, upgrade, research, command-card affordance, cost/supply/sight
  change, or trainability change.
- Coax does not rotate the turret, move the Tank, chase targets, or replace explicit player attack
  intent. It only fires opportunistically through the current authoritative turret/weapon facing.
- Coax shots overpenetrate with small-arms damage. They must not accidentally get Tank AP damage,
  Tank cannon facing multipliers, Tank cannon sound, Tank cannon muzzle flash, or Tank cannon recoil.
- Existing Tank cannon behavior remains the regression baseline: main cannon target selection,
  cooldown, turret rotation, stationary range ramp, direct-fire overpenetration, audio, visuals,
  and recoil should continue to work as they do today.
- Before implementing live coax targeting, complete the user-gated target classification and
  priority-policy design. An implementation agent must stop as blocked and ask the user for the
  missing policy decisions instead of inventing target tags or ranking rules.
- Avoid ad hoc per-weapon target lists. Prefer one rules-owned target classification surface plus
  reusable declarative priority policies, with weapon-specific policies only where a weapon really
  has unique behavior.
- Update mirrored contracts together. Protocol event shape changes touch Rust contract/protocol
  crates, compact metadata/encoding, JS protocol decoding, protocol parity coverage, and
  `docs/design/protocol.md`.
- Update balance and data mirrors together when a player-visible value or generated stats surface
  changes. Rust rules stay authoritative; client config/wiki surfaces mirror only the data they
  consume or display.
- Respect client architecture. Client modules should continue to receive collaborators through
  existing injection paths, and new listeners or GPU resources must have teardown.
- Collect factual patch-note bullets during gameplay phases: Tank gains passive coax, coax range,
  damage, cadence, overpenetration, infantry priority, fallback behavior, and visual/audio
  affordances.
- Each implementation phase must land on its own `zvorygin/` branch, be pushed as an owned PR with
  auto-merge armed, and wait for a definite merge with the phase head reachable from `origin/main`
  before the next phase starts.
- After implementing each phase, the implementing agent must provide a handoff message describing
  what changed, what the next agent should do, and what should be manually tested. Manual testing
  notes should name core gameplay scenarios, not an exhaustive test matrix.
- When a phase is complete, mark that phase document as done in the implementation commit for that
  phase.

## Required Verification Themes

Each phase should run the smallest relevant subset of:

- focused Rust tests for rules weapon profiles, damage classification, cooldowns, combat target
  selection, overpenetration, fog projection, and replay-stable events
- `node tests/protocol_parity.mjs` and focused protocol contract tests after event/compact/protocol
  vocabulary changes
- `node scripts/check-faction-catalog-parity.mjs` and `node scripts/check-wiki.mjs` after visible
  rules, generated stats, faction catalog, or wiki surface changes
- focused client contract tests for audio mapping, visual effect buffering, protocol decoding,
  renderer feedback, and tank rig rendering
- `node scripts/check-client-architecture.mjs` after client module or wiring changes
- `cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture` after
  sim service boundary or dependency changes
- `node scripts/check-docs-health.mjs` for docs-heavy phases
- `git diff --check`

## Suggested Execution

Do not execute the existing draft phases yet. First run a planning pass that rewrites the phase
files around the architecture groups above, especially the user-gated target classification and
priority-policy work. After the revised phase files are reviewed, implement one phase at a time from
a clean worktree, push each phase as an owned PR with auto-merge armed, and wait for the phase head
to reach `origin/main` before starting the next phase.
