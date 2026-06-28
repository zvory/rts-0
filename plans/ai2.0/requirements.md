# AI 2.0 Requirements

Status: Draft requirements. This document describes the target AI development platform and
feedback loop, not an approved implementation plan or phase scope.

## Purpose

The goal of AI 2.0 is to create a strong RTS opponent and the development harness needed to keep
making it stronger. Macro economic management is only part of that target; the hard problem is
micro, tactical control, defense, support-weapon use, and coherent army behavior under fog,
pathing, terrain, and simultaneous combat pressure.

The system should let humans and coding agents iterate on AI behavior in many directions at once.
Agents should be able to create, combine, run, compare, and promote AI variants without copying
large bespoke decision trees, relying on visual inspection alone, or requiring a human to translate
every replay observation into a narrow bug report.

## Core Requirements

### Strong Observation And Action SDK

- Provide a stable, fog-safe AI observation contract derived from the same authoritative snapshot
  surface available to players.
- Include raw visible facts and useful derived facts for tactics: entity health, position, state,
  target, range-relevant stats, current orders, production state, visible threats, remembered enemy
  buildings, resource knowledge, terrain/pathing affordances, and nearby tactical context.
- Provide an action contract that covers the full player command surface, including movement,
  attack, attack-move, stop, hold, rally, build, gather, train, research, cancel, deconstruct,
  ability use, autocast, and support-weapon setup or teardown.
- Expose legality, affordability, reservation, and rejection reasons so AI variants and debugging
  agents can understand why a candidate action was or was not valid.
- Build higher-level tactical helpers on top of ordinary commands where useful, such as focus fire,
  retreat, hold line, form concave, screen support weapons, set up anti-tank guns, and safe mortar
  fire. These helpers must still compile down to normal validated simulation commands.

### Policies As Data Where Feasible

- Represent AI profiles, module selection, utility weights, response curves, build-order choices,
  squad doctrines, tactical parameters, experiment manifests, and feature flags as data where this
  keeps behavior inspectable and composable.
- Keep complex algorithms in tested code modules. Pathing, target selection, threat-field
  generation, concave generation, tactical simulators, and command validation should not become an
  ad hoc data-language interpreter.
- Let one agent develop a reusable tactic or module and let other profiles opt into it by
  referencing that module and setting parameters, rather than forking whole AI trees.
- Support many AI profile combinations from a small library of reusable modules, overlays, curves,
  and parameter sets.
- Version, validate, hash, and record every data-driven profile used in an experiment so results
  are reproducible.

### AI Arena

- Create an arena for running AI profiles against each other at scale across seeds, sides, maps,
  tick caps, and scenario suites.
- Store machine-readable run artifacts: manifest, profile definitions, git SHA, match rows,
  summary metrics, replay links, command traces, decision traces, and failure classifications.
- Run both full-game self-play and focused tactical scenarios. Full games should measure strength;
  tactical scenarios should explain micro behavior.
- Include side swaps and holdout seeds so profile improvements are measured against bias and
  overfitting.
- Support large local sweeps on powerful developer hardware, with enough structure that coding
  agents can launch experiments, compare results, and decide what to inspect next.

### Human And Agent Legibility

- Make AI-vs-AI matches inspectable by humans through replay/debug tooling that can show what the
  AI saw, what it remembered, which managers were active, which actions were considered, how actions
  scored, what command was emitted, and what happened afterward.
- Support stepping, rewinding, and branching from earlier ticks where practical, so a developer can
  isolate why a decision went wrong and test a changed policy from the same state.
- Preserve AI decisions and observations in structured artifacts, but do not make replay
  legibility depend on handing an LLM a huge raw JSON dump; provide meaningful summaries, suspicious
  episode reports, score breakdowns, diffs, and drill-down commands so agents can retrieve detail
  only when needed.
- Link decisions to outcomes. A useful trace should say not only what the AI chose, but what it
  believed, which alternatives lost, why they lost, and whether the resulting trade or tactical
  outcome was good or bad.
- Produce compact agent-facing case files for failures and regressions, such as "attack won because
  anti-tank threat scored zero; 90 ticks later the profile lost four tanks for no kills," with links
  to exact decisions, observations, replay ticks, and source modules.

## Desired Development Loop

The AI 2.0 harness should let agents operate in a reinforcement-style loop without needing constant
human interpretation:

1. Create or modify an AI profile, data overlay, utility curve, or reusable tactic module.
2. Validate that the profile and actions are legal against the AI SDK.
3. Run arena matches and tactical scenarios against current, historical, and experimental profiles.
4. Compare results using machine-readable summaries, side-swapped seeds, and tactical metrics.
5. Inspect the most suspicious failures through compact decision case files and optional replay
   drill-downs.
6. Keep useful modules and profile overlays available for future agents to combine.
7. Promote stronger profiles only when they improve measured strength without regressing core
   tactical, macro, legality, determinism, or performance guardrails.

## Success Criteria

- AI profiles improve through repeatable arena evidence rather than one-off visual impressions.
- Agents can create and evaluate many AI variants without duplicating large bespoke decision trees.
- Humans can understand AI decisions from replay/debug artifacts without reading code first.
- Agents can understand failures from compact summaries and drill into exact decisions only when
  necessary.
- Tactical improvements become reusable modules or data overlays that other profiles can adopt.
- The live AI remains a normal fog-bound player: no private simulation authority, no hidden enemy
  knowledge, and no bypass around ordinary command validation.

## Non-Goals

- Do not start with end-to-end ML or reinforcement learning as the primary implementation path.
- Do not make the AI omniscient for normal play or normal self-play evaluation.
- Do not define success only by full-game win rate; micro, defense, support-weapon use, legality,
  determinism, and performance need separate measurement.
- Do not represent the entire RTS brain as declarative data. The data layer should compose and tune
  tested modules, not replace code with an unmaintainable hidden programming language.
- Do not rely on raw replay JSON as the main human or agent debugging interface.
