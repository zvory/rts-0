# AI 1.0 Opponent Plan

## Purpose

Build a fair, maintainable 1.0 lobby AI that teaches the game by playing a strong, readable RTS
style. The AI should be hard for a new RTS player to beat for many games, should pressure an
experienced RTS player for at least a few games while they learn the tech tree, and should remain
easy to tune while the game's economy, units, and balance keep changing.

The target is not hidden information, machine learning, or opponent modeling. The target is a
scripted RTS opponent with a transparent phase plan, simple managers, fast tests, good self-play
artifacts, and a consistent stream of increasingly sophisticated attacks.

## Product Requirements

- The AI must be strictly fair: ordinary commands, normal economy, normal fog, normal validation,
  and no hidden bonuses.
- The launch target is 1v1 main-lobby play. 2v2 should keep working well enough for tests and
  rough cooperative play, but 2v2 behavior may be simple.
- Keep the current first attack timing in the same broad window as the existing saturation AI.
- Do not use workers offensively for 1.0.
- The 1.0 unit arc is Riflemen first, Scout Cars for flanking steel-line harassment second, then
  Tanks. Machine Gunners, AT Teams, Artillery, and Command Cars are optional/future unless needed
  as simple defensive support once tank production is running.
- The AI must expand and tech. It should generally expand earlier than the current saturation AI
  because oil becomes a serious constraint on tech.
- Required attack styles for 1.0 are frontal staged waves and Scout Car harassment routed toward
  the back of the enemy steel line.
- Do not require split attacks, retreat/regroup, mortar dodging, offensive worker use, building
  ignore logic, or focused unit targeting for the first launch version.
- Scout Cars should eventually use smoke against enemy combat units, especially Machine Gunners,
  rather than smoking workers.

## Core Model

Use a classic RTS bot structure: a small strategic director plus deterministic managers. Avoid a
generic GOAP, HTN, behavior-tree, or utility-AI framework for this pass; those are more machinery
than the launch opponent needs.

- Strategic director: chooses the current scripted phase and exposes phase targets.
- Economy manager: workers, resource assignment, supply, and expansion targets.
- Build manager: buildings, tech path, and placement requests.
- Production manager: units and research queues.
- Army manager: staging, attack waves, Scout Car harassment, and simple local defense.
- Debug/telemetry layer: reports phase, active goals, blockers, and emitted commands without
  spamming normal server logs.

The current `rifle_flood_full_saturation` behavior should stay available as a named baseline and
rollback option. The new AI should be built alongside it first, then promoted to the live-lobby
default only after matchup and scenario tests show that it is not worse.

## Testing Strategy

Testing is a first-class requirement, not a final hardening task. Most AI bugs should be caught by
fast manager tests and short scenario tests before any long self-play run is needed.

- Pure manager tests should run in milliseconds and validate target selection, reservations,
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

## Phase Summaries

Phase 1 captures the AI product contract and creates fast AI scenario fixtures. It adds authored
game-state builders for opening, mid-game, expansion, harassment, and tank-tech situations without
changing live AI behavior. The outcome is a fast testing foundation that can verify managers from
specific positions instead of always replaying a whole match.

Phase 2 introduces the new scripted AI architecture behind a non-default profile id. It adds the
strategic director, phase model, manager interfaces, decision trace structure, and baseline parity
tests while keeping the current saturation AI unchanged. The outcome is a debuggable AI skeleton
that can explain what phase it is in, what goal it is pursuing, and why a goal is blocked.

Phase 3 implements the economy, build, and production managers for the 1.0 plan. It makes the new
AI saturate steel, expand earlier for oil, build the required tech path, produce Riflemen, then
Scout Cars, then Tanks, and avoid supply stalls. The outcome is a macro-capable fair AI that can
reach the required tech arc from both opening and seeded mid-game states.

Phase 4 implements the army manager for staged attacks and Scout Car harassment. It keeps frontal
Rifleman waves in staged groups, routes Scout Cars toward the rear of the enemy steel line for
harassment, and launches tank waves once tank production is online. The outcome is a launch-grade
attack cadence with more novelty than the current saturation AI.

Phase 5 adds matchup gates, replay/debug visibility, and live-lobby rollout controls. It compares
the new AI against the existing saturation baseline under bounded simulations, exposes high-value
AI decision traces for self-play/watch/debug mode, and keeps a clean fallback to the old profile.
The outcome is a measurable promotion path to make the new AI the default only when it is stronger
or at least clearly more useful for players.

Phase 6 hardens launch behavior and adds optional tactical polish. It tunes phase timings, economy
targets, harassment paths, smoke usage, and defensive support production based on tests and
playtest notes. The outcome is the 1.0 opponent: fair, economically powerful, capable of teching to
tanks, and able to apply recurring staged pressure.

## Phase Index

1. [Phase 1 - Scenario Harness and Product Contract](phase-1.md)
2. [Phase 2 - Scripted AI Skeleton](phase-2.md)
3. [Phase 3 - Macro Managers](phase-3.md)
4. [Phase 4 - Attack and Harassment Managers](phase-4.md)
5. [Phase 5 - Matchup Gates and Debug Visibility](phase-5.md)
6. [Phase 6 - Launch Tuning and Tactical Polish](phase-6.md)

## Overall Constraints

- Keep AI commands on the ordinary simulation command path.
- Do not mutate `Game` state directly from AI code.
- Keep the old saturation AI around as `rifle_flood_full_saturation` and preserve tests that prove
  it still runs.
- Make the new AI selectable by profile id before making it the live default.
- Prefer phase targets and manager tests over scattered supply/time/resource thresholds.
- Use time and supply as fallback progress signals, not as the only source of truth.
- Keep decisions deterministic: stable sorted inputs, no hash iteration order, no nondeterministic
  profile choices in tests.
- Use authored scenario states for late-game behavior tests instead of requiring long setup sims.
- Do not run broad test bundles during implementation. Use focused Rust tests, focused matchup
  tests, and bounded scenario runs matching the touched phase.
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

## Implementation and Handoff Rules

Implement one phase at a time. Each phase should be committed, merged to `main`, and pushed before
the next phase begins. When a phase is complete, mark that phase document as done in the same
implementation commit.

After each phase, the implementing agent must provide a handoff message describing what the next
agent should do and what should be manually tested. Manual testing notes should cover the core
features for that phase, not an exhaustive test matrix.
