# Tank Coaxial Machine Gun Implementation Plan

## Purpose

Implement the tank coaxial machine gun requirements in [requirements.md](requirements.md) without
bolting a second weapon onto assumptions that every entity has one attack profile. The early phases
separate weapon identity, damage classification, cooldown state, and feedback hints in
behavior-preserving steps before any Tank gets a live coax shot. Later phases add the server
runtime, client audio/visual treatment, generated data surfaces, documentation, and final
regression hardening.

## Product Input

- [requirements.md](requirements.md) is the active product requirement source.
- Coax shots must overpenetrate. They use the same direct-fire overpenetration system as other
  direct shots, but with coax small-arms damage and coax feedback scale.

## Phase Summaries

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
- Before implementing Phase 4, resolve the infantry-priority vocabulary if requirements still leave
  it ambiguous. The likely implementation should be a rules-owned helper rather than ad hoc matches
  inside combat code.
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

Implement one phase at a time from a clean worktree. Do not start a later phase from an assumed
merge; wait for the owned PR to merge and verify reachability from `origin/main`.

```bash
scripts/phase-runner.sh --plan coax 1 --pr --wait
scripts/phase-runner.sh --plan coax 2 --pr --wait
scripts/phase-runner.sh --plan coax 3 --pr --wait
scripts/phase-runner.sh --plan coax 4 --pr --wait
scripts/phase-runner.sh --plan coax 5 --pr --wait
scripts/phase-runner.sh --plan coax 6 --pr --wait
scripts/phase-runner.sh --plan coax 7 --pr --wait
```
