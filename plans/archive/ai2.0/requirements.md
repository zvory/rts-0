# AI 2.0 Requirements

Status: Draft design requirements. This document describes the desired AI development system and
feedback loop, not an approved implementation plan or phase scope.

## Purpose

AI 2.0 is an agent-first AI development system whose product goal is a stronger RTS opponent. The
system should make it easier for humans and coding agents to create AI variants, run fair
comparisons, inspect what happened, and promote better behavior with evidence.

The first design priority is not a single clever tactic. The first priority is a repeatable loop:
change an AI profile or module, run it against useful opponents and scenarios, summarize the game in
plain text, inspect the relevant replay and decision evidence, then keep or reject the change.

The live AI must remain a normal fog-bound player. It should see only what a real player could see,
issue normal commands, and win by better decision-making rather than private simulation authority.

## Design Position

- Build the AI development platform first, with a stronger live opponent as the proof that the
  platform works.
- Treat the LLM as an investigator and designer, not as a magic oracle. The system should give the
  LLM a useful textual description of the game and indexed evidence it can search, not expect it to
  infer everything from raw replay JSON.
- Use data-driven AI definitions where they make behavior easier to inspect, combine, and tune.
  Keep complicated tactical algorithms in tested code modules with data parameters.
- Defer replay branching. Rewinding and branching from an exact tick is useful, but it is not
  required for the first acceptable AI 2.0 loop.
- Do not try to automatically detect "the match-losing mistake" in the first version. That problem
  is close to building the strong AI itself. The first version should surface likely moments,
  summaries, metrics, and decision traces so a human or LLM can investigate.

## Core Requirements

### Observation And Action SDK

- Provide a stable, fog-safe AI observation contract derived from the same authoritative snapshot
  surface available to players.
- Include visible facts and useful derived facts for AI decisions, such as health, position, state,
  target, production state, visible threats, remembered enemy buildings, resource knowledge,
  terrain/pathing hints, and nearby tactical context.
- Provide an action contract that covers the normal player command surface the AI is expected to
  use: movement, attack, attack-move, stop, hold, rally, build, gather, train, research, cancel,
  deconstruct, ability use, autocast, and support-weapon setup or teardown.
- Expose enough legality, affordability, reservation, and rejection information for profiles and
  agents to understand why a candidate action was not issued or did not work.
- Build higher-level helpers on top of ordinary commands where useful, such as focus fire, retreat,
  hold line, screen support weapons, set up anti-tank guns, and safe mortar fire. These helpers must
  still compile down to normal validated simulation commands.

### Modular Policies

- Represent profile composition, module selection, utility weights, response curves, build-order
  choices, tactical parameters, experiment manifests, and feature flags as data where this makes AI
  behavior easier to inspect and combine.
- Keep complex algorithms in code. Pathing, target selection, threat evaluation, formation
  generation, tactical simulation, and command validation should not become an ad hoc data-language
  interpreter.
- Let agents create reusable modules and allow profiles to opt into those modules with parameters
  instead of forking whole decision trees.
- Version, validate, hash, and record every profile or data overlay used in an experiment so results
  are reproducible.

### AI Arena

- Run AI profiles against current, historical, and experimental opponents across seeds, sides, maps,
  tick caps, and focused scenario suites.
- Store machine-readable run artifacts: manifest, profile definitions or hashes, git SHA, match
  rows, summary metrics, replay links, command traces, decision traces, and notable event indexes.
- Run both full-game self-play and focused tactical scenarios. Full games measure overall strength;
  tactical scenarios make specific behavior easier to inspect.
- Include side swaps and holdout seeds so improvements are not just spawn, seed, or map bias.
- Support large local sweeps on developer hardware while keeping outputs structured enough for
  coding agents to compare results and decide what to inspect next.

### Textual Replay Investigation

- Produce a compact textual game brief for every important run. The brief should describe the
  matchup, map, seed, major timing milestones, economy shape, army composition, major fights,
  winner or tick-cap state, and unusual spikes or stalls.
- Index replay and decision evidence so an agent can search by tick, event type, player, unit kind,
  command type, tactical module, or decision label.
- Preserve AI observations and decisions in structured artifacts, but do not make the main workflow
  depend on reading huge raw JSON dumps.
- Link summaries to evidence. When the brief says an attack failed, economy stalled, or support
  weapons were idle, it should point to replay ticks, nearby commands, and relevant AI trace labels.
- Prefer "investigation starting points" over final blame. The system should say, for example,
  "large tank trade at tick 8120; AI had three idle anti-tank guns nearby; inspect defense/support
  traces around 7900-8250" rather than claim it knows the single match-losing mistake.

## Desired Development Loop

The AI 2.0 harness should let agents operate with less constant human interpretation:

1. Create or modify an AI profile, data overlay, utility curve, or reusable tactic module.
2. Validate that the profile is well-formed and uses only legal AI modules/actions.
3. Run arena matches and focused scenarios against current, historical, and experimental profiles.
4. Compare results using machine-readable summaries, side-swapped seeds, holdout seeds, and tactical
   metrics.
5. Read the textual game brief and use it to search replay and decision evidence around relevant
   ticks.
6. Keep useful modules and profile overlays available for future agents to combine.
7. Promote stronger profiles only when they improve measured strength without regressing core
   tactical, macro, legality, determinism, fog-safety, or performance guardrails.

## First Acceptable Slice

The first acceptable AI 2.0 slice should prove the loop, not the whole vision:

- A small set of reusable AI modules or profile overlays can be combined without copying a full
  decision tree.
- Arena runs can compare variants against the current default across a fixed seed/map/side-swap set.
- Each run saves a replay, manifest, profile identity, summary metrics, and a bounded decision trace.
- Each run produces a plain-text game brief that gives an LLM enough context to decide where to
  inspect next.
- The evidence is searchable by important ticks, commands, events, and AI decision labels.
- Promotion requires repeatable improvement plus no regression in legality, determinism,
  fog-safety, basic economy progress, and performance.

## Success Criteria

- AI profiles improve through repeatable arena evidence rather than one-off visual impressions.
- Agents can create and evaluate variants without duplicating large bespoke decision trees.
- Humans and agents can understand the shape of a match from a compact textual brief before opening
  the replay.
- Agents can use the brief to search exact replay ticks and decision traces instead of reading raw
  replay JSON from top to bottom.
- Tactical improvements become reusable modules or data overlays that other profiles can adopt.
- The live AI remains a normal fog-bound player: no private simulation authority, no hidden enemy
  knowledge, and no bypass around ordinary command validation.

## Non-Goals

- Do not start with end-to-end ML or reinforcement learning as the primary implementation path.
- Do not make the AI omniscient for normal play or normal self-play evaluation.
- Do not define success only by full-game win rate; micro, defense, support-weapon use, legality,
  determinism, fog-safety, and performance need separate measurement.
- Do not represent the entire RTS brain as declarative data. The data layer should compose and tune
  tested modules, not replace code with an unmaintainable hidden programming language.
- Do not rely on raw replay JSON as the main human or agent debugging interface.
- Do not make replay branching a first-version requirement.
- Do not require the system to automatically identify the single match-losing mistake.
