# Phase 8 - Second Faction Brief and Rules Spec

Status: Designed, not implemented.

## Objective

Create the approved second-faction brief and rules/balance spec before implementing real faction
content. This is the gate where major design decisions about how the faction works must be run by
the user.

## Scope

- Write or reference the faction brief, including theme, intended strategic identity, economy,
  resource names, production model, strengths, weaknesses, and expected match pacing.
- Define the starting loadout, resource model, supply/capacity model, and whether universal map
  resources are ignored or used.
- Define the minimum playable implementation slices:
  - start, economy, and first production path
  - baseline combat unit
  - signature ability-heavy unit
  - later roster/progression expansions
- Define the initial unit/building/upgrade/ability roster with factual stat targets or placeholder
  ranges that can be tested.
- Define required client art/readability expectations for each initial unit/building.
- Decide whether prediction remains disabled for the new faction, which should be the default.
- Confirm AI remains blocked for the new faction unless the user explicitly approves AI work.
- Identify every mechanic that needs a new ability hook, resource behavior, fog event, or protocol
  field before Phase 9 starts.
- Do not implement Rust, JS, protocol, balance, art, tests, or other implementation files for the
  real faction in this phase.

## Expected Touch Points

- `plans/faction/`
- `docs/design/balance.md`
- `docs/design/server-sim.md`
- `docs/design/client-ui.md`
- `docs/design/protocol.md` if the spec introduces new contract requirements
- optional faction brief/spec docs under `docs/` or `plans/faction/`

## Verification

- Documentation review only.
- Confirm the brief/spec answers economy, loadout, production, first combat, signature ability, AI,
  prediction, and art-readability questions.
- No broad test run is required for a docs-only phase.

## Manual Testing Focus

No gameplay manual testing. Human review should focus on whether the faction design is approved and
small enough for Phase 9 and Phase 10 to implement independently.

## Handoff Expectations

The handoff must name the approved brief/spec files, list explicit user-approved decisions, identify
any open questions, and state exactly what Phase 9 may implement. If any major faction mechanic is
not approved, Phase 9 must not implement it.

## Player-Facing Outcome

No gameplay change. The second faction is designed and scoped before implementation starts.
