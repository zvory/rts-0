# AI 1.0 Opponent Requirements

## Purpose

Build a fair, maintainable 1.0 lobby AI that teaches the game by playing a strong, readable RTS
style. The AI should be hard for a new RTS player to beat for many games, should pressure an
experienced RTS player for at least a few games while they learn the tech tree, and should remain
easy to tune while the game's economy, units, and balance keep changing.

The target is not hidden information, machine learning, or opponent modeling. The target is a
strong RTS opponent with transparent decisions, fast tests, good self-play artifacts, and a
consistent stream of increasingly sophisticated attacks.

## Product Requirements

- The AI must be strictly fair: ordinary commands, normal economy, normal fog, normal validation,
  and no hidden bonuses.
- The launch target is 1v1 main-lobby play. 2v2 should keep working well enough for tests and
  rough cooperative play, but 2v2 behavior may be simple.
- Keep the current first attack timing in the same broad window as the existing saturation AI.
- Do not use workers offensively for 1.0.
- The 1.0 unit arc is Riflemen first, Scout Cars for flanking steel-line harassment second, then
  Tanks. Machine Gunners, Anti-Tank Guns, Artillery, and Command Cars are optional/future unless needed
  as simple defensive support once tank production is running.
- The AI must expand and tech. It should generally expand earlier than the current saturation AI
  because oil becomes a serious constraint on tech.
- Required attack styles for 1.0 are frontal staged waves and Scout Car harassment routed toward
  the back of the enemy steel line.
- Do not require split attacks, retreat/regroup, mortar dodging, offensive worker use, building
  ignore logic, or focused unit targeting for the first launch version.
- Scout Cars should eventually use smoke against enemy combat units, especially Machine Gunners or
  other stationary support weapons, rather than smoking workers.

## Planning Requirements

This file is a product brief, not an implementation plan. Before writing implementation phases, the
next planning agent must inspect the existing AI system and decide how to evolve it.

- First inspect `server/crates/ai`, current profiles, the decision loop, live AI adapter, self-play
  tooling, matchup tooling, and `docs/design/ai.md`.
- Then propose a plan that improves AI difficulty and performance while evolving the AI code toward
  a more maintainable structure.
- The proposed architecture may be an evolution of the existing profile/decision system, explicit
  managers, HTN/GOAP, behavior trees, utility scoring, or a hybrid. The plan should justify that
  choice against the current code, expected tuning needs, testability, and implementation risk.
- Do not assume that manager extraction is required. Also do not assume the current profile system
  is sufficient without evaluating how it handles economy, tech, attacks, harassment, defense,
  blockers, and debug traces.

The current `rifle_flood_full_saturation` behavior should stay available as a named baseline and
rollback option. The new AI should be built alongside it first, then promoted to the live-lobby
default only after matchup and scenario tests show that it is not worse.

The existing AI action layer is part of this model, not something to replace. Managers should
choose typed goals, blockers, and high-level action intents, then route executable work through
`AiActionContext` and the helpers in `server/crates/ai/src/ai_core/actions.rs`. Extend that shared
action layer when a new action family is needed so budget reservation, worker/building reservation,
stable ordering, and final `SimCommand` emission stay centralized.

## Testing Strategy

Testing is a first-class requirement, not a final hardening task. Most AI bugs should be caught by
fast focused tests and short scenario tests before any long self-play run is needed.

- Pure decision tests should run in milliseconds and validate target selection, reservations,
  priority order, command generation, and blockers.
- Scenario tests should start from compact authored states, including mid-game and late-game states,
  so tank, expansion, and harassment behavior can be tested without simulating 10,000 setup ticks.
- Matchup tests against the existing saturation baseline should run bounded simulations and compare
  scorecard metrics such as army value, worker count, tech milestones, attacks launched, buildings
  killed, and damage dealt.
- A normal AI regression target should run in under one minute at first. Individual fast tests
  should stay much faster so they remain useful during development.
- Long 15,000 to 20,000 tick tests are acceptable only as optional full-AI checks, not as the
  primary development loop.

## Maintainability Goals

- The AI should expose enough structure to explain its current goal, active constraints, blockers,
  and emitted commands.
- AI behavior should be tuneable through clear strategic data or localized policies, not scattered
  magic thresholds.
- Economy, build order, production, attack timing, harassment, defense, and tech transitions should
  be testable from authored states.
- Keep final `SimCommand` emission centralized in `AiActionContext` / `ai_core::actions`; higher
  level AI structure should choose goals, blockers, and intents rather than hand-rolling parallel
  command generation, resource budgeting, or reservation policy.
- The implementation should avoid parallel systems that duplicate existing AI concepts unless the
  plan explains why replacement is safer than evolution.
- The implementation should avoid over-general frameworks unless the extra abstraction clearly
  reduces tuning complexity or future behavior risk.

## Overall Constraints

- Keep AI commands on the ordinary simulation command path.
- Keep final `SimCommand` emission centralized in `AiActionContext` / `ai_core::actions`; managers
  choose actions/intents and should not hand-roll parallel command generation, resource budgeting,
  or reservation policy.
- Do not mutate `Game` state directly from AI code.
- Keep the old saturation AI around as `rifle_flood_full_saturation` and preserve tests that prove
  it still runs.
- Make the new AI selectable by profile id before making it the live default.
- Prefer explicit goals, policies, and tests over scattered supply/time/resource thresholds.
- Use time and supply as fallback progress signals, not as the only source of truth.
- Keep decisions deterministic: stable sorted inputs, no hash iteration order, no nondeterministic
  profile choices in tests.
- Use authored scenario states for late-game behavior tests instead of requiring long setup sims.
- Do not run broad test bundles during implementation. Use focused Rust tests, focused matchup
  tests, and bounded scenario runs matching the touched behavior.
- Update `docs/design/ai.md` whenever live AI behavior, profile selection, debug surfaces, or
  self-play contracts change.

## Promotion Bar

The new AI should not replace the current saturation AI until it satisfies all of these:

- It reaches Rifleman, Scout Car, expansion, and Tank milestones from normal starts.
- It launches staged frontal attacks and at least one Scout Car harassment pattern.
- It beats or out-scores the current saturation AI in bounded matchup tests by army/economy/kill
  metrics when a full elimination is too slow.
- Its fast scenario tests cover opening, expansion, harassment, tank tech, and blocked-goal cases.
- Its debug trace explains the current phase, active targets, blockers, and emitted commands well
  enough to inspect self-play failures.

---

# AI 1.0 Phased Implementation Plan

## Chosen Architecture

Evolve the current AI into a light goal/manager layer over the existing observation, facts, profile,
decision, and action systems. Managers should choose explicit strategic goals, blockers, and
high-level action intents for economy, tech, production, defense, attacks, harassment, and
expansion. Final command emission must remain centralized in `AiActionContext` and the helpers in
`server/crates/ai/src/ai_core/actions.rs`.

This plan intentionally avoids a full HTN, GOAP, or behavior-tree replacement for AI 1.0. The
current AI already has deterministic profile data, a shared action layer, self-play adapters, and
matchup tooling, so replacing it would add risk without solving the immediate tuning and
explainability problems. The new manager layer should make decisions easier to test from authored
states while preserving the existing fair-command path.

## Phase Summaries

Phase 1 establishes the measurement baseline before behavior changes. It expands focused decision,
scenario, and matchup reporting around the existing profiles so later phases can prove they are not
regressions. The result is a fast AI development loop with clear milestone and scorecard evidence.

Phase 2 adds explicit strategic state and debug traces without changing live behavior. It introduces
named goals, blockers, and manager output records that mirror the current decision order. The result
is an inspectable skeleton that later phases can fill in safely.

Phase 3 moves economy and expansion choices into the new manager layer. It keeps workers, supply,
resource assignment, and City Centre expansion on ordinary action helpers while making expansion
timing and blocked-goal reasons testable. The result is an AI that can reliably grow beyond its
main base without hiding economy rules in scattered thresholds.

Phase 4 moves tech and production choices into the manager layer for the 1.0 unit arc. It builds a
new selectable profile that opens with Riflemen, adds Scout Cars for harassment, and converts into
Tanks when economy and tech milestones are met. The result is a profile that reaches the required
milestones from normal starts before it is asked to beat the baseline.

Phase 5 adds the frontal staged-wave attack manager. It should preserve the current broad first
attack timing while making wave readiness, staging, reissue cadence, and visible-target reactions
explicit. The result is a readable pressure plan that can be tuned independently from economy and
tech.

Phase 6 adds Scout Car steel-line harassment. It routes small harassment groups toward the back of
the enemy steel line using public start/resource information and fog-respecting visible targets.
The result is the required second attack style without requiring split-attack mastery, retreat
logic, or hidden information.

Phase 7 promotes the new profile only after matchup, scenario, and debug-trace gates pass. It keeps
`rifle_flood_full_saturation` selectable as the named rollback baseline while making the new profile
available to live lobby AI. The result is an evidence-backed AI 1.0 default with clear release notes
and follow-up work.

## Phase Index

1. [Phase 1 - Baseline Metrics and Scenario Gates](phase-1.md)
2. [Phase 2 - Strategic Goal Skeleton and Debug Trace](phase-2.md)
3. [Phase 3 - Economy and Expansion Managers](phase-3.md)
4. [Phase 4 - Tech and Production Managers](phase-4.md)
5. [Phase 5 - Frontal Wave Attack Manager](phase-5.md)
6. [Phase 6 - Scout Car Harassment Manager](phase-6.md)
7. [Phase 7 - Promotion, Live Selection, and Rollback](phase-7.md)

## Overall Constraints

- Keep AI fair: ordinary commands, normal economy, normal fog, normal validation, and no hidden
  bonuses.
- Keep `Game` AI-free. AI code may use public, fog-respecting `Game`/snapshot surfaces only.
- Keep final `SimCommand` emission centralized in `AiActionContext` and
  `ai_core::actions`; managers choose goals and intents, not ad hoc command paths.
- Keep `rifle_flood_full_saturation` available as a named baseline and rollback option throughout
  the effort.
- Make the new AI selectable by profile id before making it the live default.
- Preserve deterministic decisions with sorted inputs, stable tie-breakers, and no nondeterministic
  test profile choices.
- Prefer authored scenario states and focused decision tests over long setup simulations.
- Use `RTS_FULL_AI_TESTS=1 cargo test` or the full AI gate only when a phase touches strategy
  behavior, profile-backed self-play, replay determinism, or long-match balance behavior.
- Update `docs/design/ai.md` whenever live AI behavior, profile selection, debug surfaces, or
  self-play contracts change.
- Collect factual patch-note bullets during behavior phases so the final summary can explain
  player-facing AI changes.

## Implementation and Handoff Rules

Implement one phase at a time. Each phase must be committed, merged to `main`, and pushed before
the next phase begins. When a phase is complete, mark that phase document as done in the same
implementation commit.

After implementing each phase, the implementing agent must provide a handoff message describing
what the next agent should do and what should be manually tested. Manual testing notes should name
the core AI behaviors to inspect, not a comprehensive test matrix. If a phase leaves a known test,
debug, or balance gap, call it out directly in the handoff.
