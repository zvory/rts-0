# Phase 9 - Second Faction Brief and Rules Spec

Status: Reset. Ekaterina is reserved only; no rules/balance spec is approved.

## Objective

Create a new approved Ekaterina brief and rules/balance spec before implementing any real faction
content. The prior RTS-style Ekaterina concept was purged; this phase must restart from the
hero-centric direction.

## Scope

- Write or reference the faction brief, including theme, intended strategic identity, how the
  mostly-one-hero control model works, strengths, weaknesses, and expected match pacing.
- Define whether Ekaterina uses Kriegsia's Steel/Oil/Supply model, a stripped-down variant, hero
  progression, cooldowns, objectives, or some other economy. Do not assume workers, bases, or
  production buildings.
- Define the minimum playable implementation slices for a MOBA-like hero faction.
- Define the initial hero, controls, progression, abilities, respawn/death behavior, objective
  interaction, and any supporting non-hero entities with factual stat targets or placeholder ranges
  that can be tested.
- Define required client art/readability expectations for the hero, ability targeting, and any
  approved supporting entities.
- Decide whether prediction remains disabled for the new faction, which should be the default.
- Confirm AI remains blocked for the new faction unless the user explicitly approves AI work.
- Identify the exact existing assignment path Phase 10 may use to start as Ekaterina before normal
  lobby selection is exposed, if any.
- Define command-id namespace expectations for the faction's build/train/research/ability actions
  so hotkey profiles remain stable.
- Identify every mechanic that needs a new ability hook, resource/progression behavior, fog event,
  or protocol field before Phase 10 starts.
- Explicitly record which resource/progression ideas are approved and which are out of scope.
- Explicitly reject the purged RTS-style content unless the user re-approves it by name.
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
- Confirm the brief/spec answers hero control, progression/economy, starting state, first combat,
  signature abilities, AI, prediction, and art-readability questions.
- No broad test run is required for a docs-only phase.

## Manual Testing Focus

No gameplay manual testing. Human review should focus on whether the faction design is approved and
small enough for Phase 10 and Phase 11 to implement independently.

## Handoff Expectations

The handoff must name the approved brief/spec files, list explicit user-approved decisions, identify
any open questions, name the exact assignment path Phase 10 should use, and state exactly what
Phase 10 may implement. If any major faction mechanic is not approved, Phase 10 must not implement
it. A generic "start/economy/production" slice is not approved for Ekaterina.

## Player-Facing Outcome

No gameplay change. Ekaterina remains reserved until the new hero-centric design is approved.
