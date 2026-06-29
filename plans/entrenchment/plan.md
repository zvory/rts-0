# Entrenchment Implementation Plan

## Purpose

Implement the Entrenchment requirements as server-authoritative persistent battlefield state.
Eligible infantry with the researched upgrade can create neutral trenches by staying still, and
eligible infantry from any player can later use those trenches for defensive combat benefits. The
plan keeps the occupied-unit visual treatment out of scope until the user explicitly approves a
direction.

## Product Input

- [requirements.md](requirements.md) is the active product requirement source.

## Phase Summaries

### [Phase 1 - Research, Rules, And Protocol Contract](phase-1.md)

Add the Training Centre Entrenchment research item and the shared rules vocabulary for eligible
infantry, trench timing, and trench combat constants. Define the authoritative trench snapshot shape
and compact protocol slots before gameplay depends on them. This phase should make the upgrade and
contract visible to tests and docs, but it should not yet create trenches or grant combat benefits.

### [Phase 2 - Persistent Trench State And Visibility](phase-2.md)

Add the server-owned trench store, fog-safe projection, replay/lab/dev visibility behavior, and
client-side receipt of trench snapshots. A trench should be neutral battlefield terrain with stable
identity and world-pixel position, not a buildable structure or a client-only decal. This phase
should prove trenches can exist, persist, and project safely before units start making or using
them.

### [Phase 3 - Dig-In, Occupation, And Slotting](phase-3.md)

Implement automatic trench creation for eligible infantry owned by a player with Entrenchment
research after three stationary seconds on untrenched ground. Implement immediate occupation of
existing trenches, including small collision-safe slotting movement for stopped eligible infantry.
This phase should expose whether a unit is occupying a trench, but it should not yet apply the
combat bonuses.

### [Phase 4 - Entrenched Combat Benefits](phase-4.md)

Apply the defensive and offensive combat rules for eligible infantry that are stationary in an
active trench. Entrenched infantry gain one tile of weapon range, direct shots against them take a
70% miss chance, area damage against them is reduced by 70%, idle aggressive pursuit is suppressed,
and over-penetration through or into them is suppressed. This phase should cover Methamphetamines
interactions without changing the requirement that moving units are not entrenched.

### [Phase 5 - Client Rendering, UX Polish, And Hardening](phase-5.md)

Render persistent brown trench ground on the client using the authoritative trench snapshots, with
nearby trenches visually connected where practical. Add HUD/status affordances and focused
integration coverage for multiplayer trench reuse, fog, replays, and reconnects. Defer the final
occupied-unit visual treatment unless the user explicitly approves it before or during this phase.

## Phase Index

1. [Phase 1 - Research, Rules, And Protocol Contract](phase-1.md)
2. [Phase 2 - Persistent Trench State And Visibility](phase-2.md)
3. [Phase 3 - Dig-In, Occupation, And Slotting](phase-3.md)
4. [Phase 4 - Entrenched Combat Benefits](phase-4.md)
5. [Phase 5 - Client Rendering, UX Polish, And Hardening](phase-5.md)

## Overall Constraints

- Keep trench creation, occupation, and combat effects server-authoritative. The client may render
  and preview received state, but it must not decide whether a unit is entrenched.
- Do not implement trenches as ordinary buildable buildings or as client-only death decals. They
  are neutral persistent battlefield terrain/state created by eligible infantry after meeting the
  stationary requirement.
- Do not add a player command unless implementation evidence proves one is needed. The product
  requirement says trench creation is automatic after stationary time, and normal commanded
  movement cancels or prevents new trench creation.
- Existing trenches are neutral: any eligible infantry can occupy them regardless of whether that
  player's team researched Entrenchment. Research gates creating new trenches only.
- Only active trench occupation suppresses idle aggressive pursuit. Entrenchment research alone
  must not make all eligible infantry passive; untrenched eligible units keep existing idle aggro.
  An entrenched idle unit may fire at legal targets in range like hold position, but must not chase
  or leave the trench unless given an explicit command that moves it out.
- Eligible units are Riflemen, Machine Gunners, and Workers/Engineers. Mortar Teams, Ekat, Golems,
  Ekat-faction units, vehicles, buildings, other support weapons, and non-infantry entities must
  not create or benefit from trenches in this feature pass.
- Ordinary firing, weapon facing, body facing, target changes, and support-weapon setup state must
  not cancel the dig-in timer. Actual commanded movement, path movement, and non-slotting forced
  movement must prevent or cancel new trench creation.
- Slotting into an existing trench may make a small positional correction, but it must preserve
  normal collision and spacing. It must not stack units on one point, pull units through static
  blockers, or make the tick path panic-prone.
- Fog remains authoritative. Trench snapshots, occupation state, target ids, over-penetration
  events, death events, and any positional effects must not reveal hidden units or hidden positions.
- Keep `Game::tick()` panic-free. Stale ids are no-ops, client-derived coordinates are checked, and
  all tick-path indexing/arithmetic stays bounded.
- Keep protocol mirrors synchronized across `server/crates/protocol/src/lib.rs`,
  `server/src/protocol.rs`, `client/src/protocol.js`, compact snapshot metadata, and
  `docs/design/protocol.md` whenever the wire shape changes.
- Keep balance mirrors synchronized across Rust rules, client config, faction catalogs, wiki
  generation, and `docs/design/balance.md` whenever player-visible rules or catalog entries change.
- Update `docs/design/server-sim.md` whenever trench lifecycle, stationary semantics, slotting, or
  combat timing becomes a current simulation contract.
- Collect factual patch-note bullets as implementation phases land: upgrade cost/time, eligible
  units, trench creation timing, neutral reuse, range bonus, miss chance, area reduction, and visual
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

- focused Rust tests for touched simulation services, combat, projection, production/research, and
  replay behavior
- `node tests/protocol_parity.mjs` and `node tests/client_contracts/protocol_contracts.mjs` for wire
  or compact snapshot changes
- `node scripts/check-faction-catalog-parity.mjs` and `node scripts/check-wiki.mjs` for visible
  rules, upgrade, catalog, or wiki changes
- `node scripts/check-client-architecture.mjs` and focused client contract tests for client module
  or rendering changes
- `cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture` for
  cross-service or `rts-sim::game` module-boundary changes
- `git diff --check`

## Suggested Execution

Implement one phase at a time from a clean worktree. Do not start a later phase from an assumed
merge; wait for the owned PR to merge and verify reachability from `origin/main`.

```bash
scripts/phase-runner.sh --plan entrenchment 1 --pr --wait
scripts/phase-runner.sh --plan entrenchment 2 --pr --wait
scripts/phase-runner.sh --plan entrenchment 3 --pr --wait
scripts/phase-runner.sh --plan entrenchment 4 --pr --wait
scripts/phase-runner.sh --plan entrenchment 5 --pr --wait
```
