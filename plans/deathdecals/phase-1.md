# Phase 1 - Client Decal Runtime

## Phase Status

- [ ] Done.

## Objective

Add the client-only death decal data flow and permanent ground texture renderer using procedural
placeholder marks. The goal is to prove the runtime shape before adding authored SVG assets: death
events create decals, decals are stamped once, and old decals do not become per-frame work.

## Scope

- Add a small client-side ground decal model/buffer.
  - Consume snapshot `death` events from `GameState.applySnapshot`.
  - Use existing event fields `id`, `x`, `y`, and `kind`.
  - Recover `owner`, `facing`, and `weaponFacing` from the previous entity map when available.
  - Resolve player color from `state.players` using the recovered owner.
  - Dedupe by dead entity id so a repeated event cannot stamp twice.
  - Queue only unpainted decals for the renderer.
- Add a renderer decal layer.
  - Insert `decals` after `terrain` and before `resources`.
  - Allocate one transparent offscreen canvas/texture for permanent decals.
  - Use a downsampled world scale, initially targeting about 4 world pixels per decal texture pixel
    unless implementation evidence points to a better constant.
  - Add one Pixi sprite to the decal layer.
  - Stamp new decals into the canvas/texture in batches.
  - Update the texture once per batch/frame, not once per decal.
- Add procedural placeholder stamp functions.
  - Infantry placeholder: a few player-color ellipses/blobs with seeded jitter.
  - Vehicle/support placeholder: a blackened rotated hull rectangle/polygon using known body
    dimensions when available, plus player-color paint fragments.
  - Unknown or no-decal kinds should stamp nothing.
- Add teardown.
  - Destroy the decal sprite/texture/base texture.
  - Clear any decal canvas, context, queue, and dedupe sets.
  - Handle repeated `destroy()` calls safely.

## Expected Touch Points

- `client/src/state.js`
- New client state helper such as `client/src/state_ground_decals.js`
- `client/src/protocol.js` imports for `EVENT`, `KIND`, and kind classification if needed
- `client/src/renderer/layers.js`
- `client/src/renderer/index.js`
- New renderer helper such as `client/src/renderer/decals.js`
- `client/src/config.js` only if a tiny render-only constant is needed
- Focused client tests under `tests/` or `tests/client_contracts/`

Avoid touching:

- Server protocol files
- Rust simulation or lobby code
- Match history and replay artifact code
- Balance values
- Existing SVG unit rig assets

## Implementation Details

- Prefer a dedicated helper for decal normalization so tests can exercise it without Pixi.
- Use deterministic seeded helpers. Do not use `Math.random()`.
- Seed from stable local data, for example `id`, `kind`, snapshot `tick`, and quantized `x/y`.
- Store no full historical decal array unless there is a bounded diagnostic reason. The long-lived
  state should be the rendered texture plus dedupe ids.
- In `GameState.applySnapshot`, process decals after `_prevById` has been assigned from the prior
  current snapshot and before callers lose access to event context.
- For owner recovery, first check the previous entity by id. If missing, check the current entity
  only as a defensive fallback. If still missing, use neutral owner/color.
- For facing recovery, prefer `facing`, then `weaponFacing`, then a deterministic fallback angle.
- Do not use current `visibleTiles` as a new gate. Receipt of the death event is the visibility
  decision for this client.
- Keep renderer errors isolated. A malformed decal should be skipped and recorded through existing
  render diagnostics rather than breaking the main render loop.

## Verification

- `node scripts/check-client-architecture.mjs`
- A focused Node/client contract test for:
  - no decal for resources/buildings/tank traps;
  - infantry and vehicle/support classification;
  - owner/facing recovery from a previous entity map;
  - deterministic variant/seed output;
  - dedupe by death id.
- A renderer-focused test or smoke assertion, where practical, proving that stamping many decals
  does not create one Pixi display object per decal.
- `git diff --check`

If browser/Pixi verification is difficult in this phase, add test-only inspection counters on the
renderer and explain the remaining visual verification in the handoff.

## Manual Testing Focus

Run a local match or dev scenario and kill at least one infantry unit and one vehicle/support unit.
Confirm placeholder marks appear below living units/buildings, remain after the dead unit disappears,
are dimmed by fog when the area leaves vision, and are cleared on rematch/teardown.

## Handoff Expectations

The handoff must name the decal buffer module, the renderer helper, the chosen decal texture
downsample, and how owner/facing fallback works. Include focused verification output, any manual
test notes, and any renderer diagnostic counters added for Phase 3 stress work.
