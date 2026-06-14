# Phase 3 - Assignment, Lifecycle, and Command Identity Guardrails

Status: Split into Phase 3A-3D, not implemented.

## Objective

Make every faction assignment path explicit before faction-specific resources, starts, or client UI
are expanded. This phase is intentionally split because the original single executor scope mixed
server lifecycle policy, AI restrictions, prediction compatibility, command identities, hotkey
profile behavior, and replay/dev path validation.

The existing faction is **Kriegsia** with canonical id `kriegsia`. The planned second faction is
**Ekaterina** with reserved id `ekaterina`. Earlier Phase 1/2 work introduced `steel_vanguard` as a
temporary current-faction id; Phase 3A must correct that before later phases build durable
contracts.

## Scope

Phase 3 is now an umbrella with four executor-sized subphases:

- [Phase 3A - Canonical Faction Validation and Lifecycle Matrix](phase-3a.md): rename the
  temporary current-faction id to `kriegsia`, reserve `ekaterina`, add server validation policy, and
  update the lifecycle matrix.
- [Phase 3B - AI and Prediction Fail-Closed Policy](phase-3b.md): keep AI Kriegsia-only and disable
  prediction only when the local player is on an unsupported faction.
- [Phase 3C - Command Identity and Per-Faction Hotkeys](phase-3c.md): namespace faction-specific
  build/train/research/ability command ids and store custom/direct hotkeys per faction.
- [Phase 3D - Replay, Branch, and Dev Lifecycle Tests](phase-3d.md): make replay, branch,
  self-play, match-history, and dev paths load recorded faction ids or reject unsupported inputs
  cleanly.

Do not implement Ekaterina gameplay content in any Phase 3 subphase.

## Expected Touch Points

See the individual subphase documents for expected touch points. Each subphase should update
`plans/faction/lifecycle-matrix.md` only for the rows it owns.

## Verification

Run the focused verification named by each subphase. Avoid broad test bundles during development;
the final implementation commit for each subphase should rely on the normal hook unless the
subphase is docs-only.

## Manual Testing Focus

Start a normal match and confirm it still uses Kriegsia and current gameplay. Later Phase 3
subphases add targeted manual checks for AI rejection, prediction diagnostics, hotkey behavior, and
replay/branch metadata.

## Handoff Expectations

Each subphase handoff must name the exact contract it introduced and what the next Phase 3 subphase
should do. Do not mark this umbrella phase complete until all four subphases are done.

## Player-Facing Outcome

No intended gameplay change. Faction choice remains hidden for normal play, but internal/dev/test
paths become explicit and the current faction is correctly named Kriegsia.
