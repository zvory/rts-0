# Phase 9.5 - Memory, Reveal, and Secrecy Gate

## Phase Status

- [ ] Not started.

## Depends On

- Phase 9 merged with current/explored fog, semantic layers, and view-generation core.

## Objective

Complete remembered, below-fog intel, and explicit above-fog reveal presentation, then prove the
highest-risk no-leak invariant end to end. Use a mandatory real server-projected two-recipient
fixture plus controlled fog-edge evidence. Keep normal routes blocked until interaction/overlays
merge in Phase 10.5.

## Work

- Render remembered buildings only from explicit received memory. They are visually distinct and
  contain no current hidden HP, queue, target, animation, effect, or movement data.
- Preserve explicit policies: `visionOnly`/legacy intel stays in `belowFogIntel`; shot/event reveals
  appear in `aboveFogReveal` only when the received frame includes the normalized reveal and only
  for its lifetime. Never resolve a hidden source id.
- Extend Phase 9 view-generation clearing to remembered data, events/history, decals, reveal state,
  SelectionScene, fog/resource state, and recipient-derived diagnostics before the next render.
- Gate every surface, not just mesh visibility: geometry, selection candidates, diagnostics, capture
  metadata, labels, particles, lights, future shadows/fit bounds, and resource keys.
- Add a mandatory real server-projected two-recipient fixture with never-authorized sentinel ids and
  positions. Assert absence from PresentationFrame, SelectionScene, Babylon objects, registry names,
  diagnostics, capture metadata, particles, lights, and future caster admission; fake frames alone
  are insufficient.
- Add checked-in Lab scenario `render3d-fog-edge` with fixed seed/team vision and visible, explored,
  unseen, remembered, below-fog intel, explicit reveal, and sentinel cases. Open separate authorized
  sessions/recipients as needed; do not add a client-only vision override.
- Cover replay seek/perspective, spectator union, Lab reset, resize, detached capture, freeze,
  rematch, and resource teardown. Keep the controlled-Lab gate closed.
- Capture `render3d-fog-edge` deterministically with `lab-interact` and inspect one PNG once.
- Update durable rendering/parity docs with no-leak evidence and remaining interaction work.

## Expected Touch Points

- remembered/reveal presentation and resource modules
- Phase 9 view-generation/reset hooks
- `tests/client_contracts/babylon_visibility_contracts.mjs` (create it in this phase)
- real two-recipient server projection coverage
- `tests/rendering_visibility_integration.mjs` (create it in this phase; owns a private server)
- `tests/browser_babylon_fog.mjs` secrecy cases wired into the authoritative runner
- durable rendering docs/parity ledger
- `plans/render3d/phase-9.5.md` status update in the implementation commit

## Security Requirements

- Invisible objects cannot cast, pick, label, light, emit, diagnose, or retain hidden positions.
- Remembered presentation is historical received data, never a live hidden entity.
- Above-fog presentation requires explicit normalized reveal semantics.
- Real recipient sentinel absence and full generation clearing are automated blocking gates.

## Explicit Exclusions

- No generic all-kind entities, selection/HP, placement/order overlays, real effect, batching,
  shadows, vegetation, representative GLB, or normal live/replay/spectator route enablement.

## Implementation Checklist

- [ ] Add remembered and below/above-fog reveal presentation.
- [ ] Complete full view-generation reset set.
- [ ] Add real two-recipient sentinel absence gate.
- [ ] Cover replay/spectator/Lab/capture/rematch resources.
- [ ] Capture/inspect `render3d-fog-edge`, update durable docs/ledger, and mark done.

## Verification

    node tests/client_contracts/babylon_visibility_contracts.mjs
    node tests/client_contracts/babylon_fog_contracts.mjs
    node tests/client_contracts/presentation_frame_contracts.mjs
    node tests/rendering_visibility_integration.mjs
    node tests/browser_babylon_fog.mjs --scenario secrecy
    node scripts/check-client-architecture.mjs
    tests/run-all.sh --only-browser-scenarios=smoke
    git diff --check

## Manual Test Focus

Inspect a remembered building, below-fog intel, explicit reveal, replay/spectator perspective, Lab
reset, capture, freeze, and rematch. Look for hidden geometry, labels, hits, resource keys,
diagnostics, metadata, stale memory, or generation mixing.

## Handoff Expectations

Report memory/reveal policy, real-recipient sentinel results, full generation clears, resource
baselines, preview command/URL, and inspected PNG. Name Phase 10 as next and identify catalog-derived
fallbacks, perspective targeting, selection/HP, minimap/audio/control groups, and continued route gate.
