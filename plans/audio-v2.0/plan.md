# Audio v2.0 First-Pass Plan

## Purpose

Deliver the first high-value audio cleanup without redesigning combat cadence, adding gameplay
notifications, or building a general-purpose sound framework. The selected work is limited to
making existing spoken notices own the mix briefly, preventing combat from consuming the whole
voice pool, reducing the reach of ordinary combat audio, and giving existing server notices one
small match-owned presentation policy. The result should remain busy and warlike at the camera while
leaving enough perceptual room for current information-bearing voice lines.

## Overall Constraints

- Keep the plan client-only. Do not change simulation events, server notice emission, the wire
  protocol, fog rules, balance, or authoritative gameplay.
- Preserve the current attack-event cadence and sample-start behavior. In particular, do not add
  rapid-fire cooldowns, controlled MG loops, retrigger suppression, per-emitter cadence changes, or
  early voice stops based on assumed silent tails.
- Use deliberately permissive combat voice ceilings because decoded buffer lifetime includes quiet
  or silent asset tails. The ceilings are guardrails against pathological stacking, not a target for
  making ordinary fights sparse.
- Route only existing `Notice` events through the new match-owned notice presenter. Do not create
  resource-exhaustion, idle-economy, advisory, production, supply-warning, or other new notices.
- Preserve current replay/spectator notice-audio suppression, under-attack viewport suppression,
  toast text, minimap behavior, and category sliders unless a phase explicitly narrows repeated
  under-attack presentation.
- Preserve local HUD command-feedback triggers, selected clips, cadence/cooldowns, and direct local
  routing; those existing alert-category clips may inherit only the approved stronger/slower mix
  duck from phase 1.
- Prefer a few explicit constants and one small collaborator over registries, adaptive mixers,
  duration analysis, telemetry systems, or per-weapon configuration surfaces.
- Update `docs/design/client-ui.md` alongside implementation behavior in each phase. No protocol
  design document should change because this plan does not alter the wire contract.
- Treat automated checks as contract coverage, not proof that the mix sounds good. Each handoff must
  name its focused manual listening check, and phase 3 ends by preparing an integrated player
  checkpoint rather than claiming subjective mix quality automatically.
- Each implementation phase must land on its own `zvorygin/` branch, be pushed as an owned PR with
  auto-merge armed, and wait for a definite merge with the phase head reachable from `origin/main`
  before the next phase begins.
- When a phase is complete, mark its phase document done in that phase's implementation commit and
  provide a handoff describing what changed, what the next agent should do, and the core manual test
  focus.

## Phase Summaries

### [Phase 1 - Existing Notice Policy and Mix Ducking](phase-1.md)

Extract the current server-notice fanout from `Match` into one small match-owned presenter that
continues to drive the existing toast, minimap, and audio surfaces. Make every existing spoken
notice request a fast, deeper mix duck, then restore combat gradually after the last ducking voice
ends. Move the existing under-attack incident policy into that presenter so one match-scoped
admission decision prevents repeated hits from spamming toast, minimap, or voice together.

### [Phase 2 - Permissive Combat Voice Guardrails](phase-2.md)

Keep the global 48-voice pool while adding a deliberately high combined combat ceiling and three
coarse combat-family ceilings. Tag existing combat sound specifications explicitly and reuse the
current priority, distance, ownership, and age score when a constrained combat voice must be
replaced. Do not change how often attack events request sounds, where clips start, how long they
play, or how machine-gunner keys are stopped.

### [Phase 3 - Combat Audibility Envelope and Listening Checkpoint](phase-3.md)

Give combat categories a tighter radial profile based on the existing camera listener reference
distance while leaving non-combat spatial behavior unchanged. Keep nearby fighting present, make
edge and offscreen combat recede strongly, and retain current panning, low-pass filtering, and
smooth updates when the camera moves. Finish with an integrated dense-battle listening checkpoint
covering combat texture, existing notices, camera movement, and the permissive voice limits.

## Phase Index

1. [Phase 1 - Existing Notice Policy and Mix Ducking](phase-1.md)
2. [Phase 2 - Permissive Combat Voice Guardrails](phase-2.md)
3. [Phase 3 - Combat Audibility Envelope and Listening Checkpoint](phase-3.md)

## Non-Goals

- No new gameplay notices, warning tiers, advisory messages, or economy-state detection.
- No notification feed, queue, history, acknowledgement flow, persistent event rail, camera-jump
  action, or Spacebar notification navigation.
- No attack-event thinning, automatic-weapon loop, per-emitter cooldown, retrigger policy, or
  combat-event aggregation.
- No asset trimming, silence detection, re-encoding, replacement, normalization, or license work.
- No compressor, limiter, sidechain processor, adaptive mix, loudness telemetry, or new user-facing
  audio setting.
- No generalized notification registry, application-wide event bus, or migration of countdown,
  victory/defeat, unit barks, lobby sounds, or unrelated UI to the notice presenter.
- No exact viewport geometry in the audio engine and no renderer/camera import added to `audio.js`.
- No deployment or beta rollout in this plan; the final deliverable is a locally validated first
  pass ready for an ordinary later rollout decision.

## First-Pass Success Criteria

- An existing spoken notice remains understandable over a dense local fight, combat falls quickly
  while it speaks, and the normal mix returns smoothly rather than snapping back.
- Repeated under-attack events in one match-scoped incident do not continually overwrite the toast
  and restamp minimap pings, while a distinct location still presents normally.
- A dense mixed fight remains audibly busy but cannot consume more than the documented permissive
  combat total/family guardrails or block higher-priority alert/UI voices.
- Combat near the listener remains satisfying, combat near and beyond the visible edge recedes, and
  far routine combat is dropped without audible popping during camera motion.
- The result does not alter attack cadence, simulation outcomes, notice vocabulary, protocol data,
  or available user settings.

## Required Verification Themes

Each phase should run the smallest applicable subset of:

- `node tests/client_contracts.mjs`
- `node scripts/check-client-architecture.mjs`
- `node tests/select-suites.mjs --verify`
- `git diff --check`

Run the focused files named in each phase while iterating. The final phase must leave a runnable
local match and focused checklist for the user/manual tester; unit tests and an executor cannot
establish perceptual quality by themselves.

## Implementation Process

Implement and merge one phase at a time. Do not begin a later phase from an assumed PR state; wait
for the prior PR to merge and verify its head is reachable from `origin/main`. For unattended
executor passes after this plan is approved, use:

```bash
scripts/phase-runner.sh --plan audio-v2.0 phase-1 --pr --wait
scripts/phase-runner.sh --plan audio-v2.0 phase-2 --pr --wait
scripts/phase-runner.sh --plan audio-v2.0 phase-3 --pr --wait
```

After every phase, the implementing agent must provide a handoff message describing the landed
behavior, relevant constants and tests, what the next agent should do, and the core features the
user should manually listen to. Keep the manual notes focused on the phase's central behavior rather
than producing an exhaustive device or browser matrix.

## Deferred Backlog

Only revisit asset-tail trimming, loudness normalization, limiting/compression, per-weapon spatial
profiles, or combat-event aggregation if the phase 3 listening checkpoint produces evidence that
the first-pass policy cannot solve the remaining problem. These are not approved executable phases
of this plan.
