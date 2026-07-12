# Rendering parity and experiment ledger

This ledger records what the two world backends actually support. It is not a production-migration
checklist: Babylon is an opt-in pre-alpha experiment, and a missing row is an honest limitation to
learn from rather than a reason to delay the first playable slice.

## Reading the ledger

Use only these statuses:

- `shared external` — intentionally outside a world renderer.
- `Pixi complete` — current Pixi behavior supports the named capability.
- `Babylon complete` — Babylon supports the whole named capability.
- `prototype` — a bounded Babylon case exists for playtest, not catalog parity.
- `placeholder` — truthful generic coverage; it is not visual parity.
- `missing` — not implemented.
- `deferred` — deliberately outside the current experiment.

An evidence entry names a focused command or inspected artifact when one exists. Do not invent a
benchmark, certification suite, or future phase merely to change a status. The two required Babylon
acceptance facts are that it consumes only authorized presentation data and that it shares the
semantic camera/selection behavior with Pixi.

## Current ledger

| Capability | Pixi | Babylon | Next evidence or decision |
| --- | --- | --- | --- |
| One Match-owned rAF and visual clock | Pixi complete | missing | Phase 4 proves `scene.render()` is Match-called only. |
| Semantic camera/projection and CSS-pixel contract | Pixi complete | missing | Phase 4 consumes it with a fixed perspective adapter. |
| Perspective-safe selection, marquee, and ground commands | Pixi complete | missing | Phase 5 uses the established `SelectionSceneV1`, never mesh picking. |
| Detached fog-filtered presentation frame | Pixi complete | missing | Phase 4 backend consumes `PresentationFrameV1` only. |
| Default Pixi and explicit Babylon selector | Pixi complete | missing | Phase 4 adds explicit opt-in Babylon loading; Pixi remains default. |
| Static ground and simple generic scene | Pixi complete | missing | Phase 4 Lab kernel. |
| Current/explored fog and received visible entities | Pixi complete | missing | Phase 5 playable slice and one real two-recipient sentinel assertion. |
| Basic selection and move feedback | Pixi complete | missing | Phase 5. |
| HUD, minimap, lobby, panels, and audio | shared external | shared external | Reuse existing surfaces; no backend duplicate. |
| Remembered/reveal visuals, effects, and long-tail overlays | Pixi complete | deferred | Add only if a playtest demonstrates a need. |
| GLB pipeline, art catalog, pooling, batching, vegetation, shadows, and quality tiers | Pixi complete or n/a | deferred | Separate measured follow-up, never a prerequisite to the experiment. |
| Replays, spectators, device rollout, Babylon default, and Pixi removal | Pixi complete | deferred | Separate reviewed product decisions. |

## Completed shared-boundary evidence

### `P1-camera`

- `automated`: `node tests/client_contracts/camera_projection_contracts.mjs`.
- `assertion`: semantic camera, CSS-pixel projection, nullable ground hits, snapshots, and Pixi
  orthographic equivalence are independent of a renderer engine.

### `P1.5-navigation-minimap` and `P1.75-shared-camera`

- `automated`: `node tests/minimap_input_contracts.mjs`, camera/audio/replay/Lab contracts, and
  `node scripts/check-client-architecture.mjs`.
- `assertion`: navigation, minimap, audio, control groups, carryover, Lab, and diagnostics use
  semantic camera data rather than raw orthographic representation.

### `P2-perspective-selection`

- `automated`: `node tests/client_contracts/selection_projection_contracts.mjs` and focused input
  contracts.
- `assertion`: detached fog-filtered selection proxies and nullable ground hits preserve command
  semantics under fake perspective projection; meshes cannot choose command targets.

### `P3-presentation-frame` and `P3.5-pixi-cutover`

- `automated`: `node tests/client_contracts/presentation_frame_contracts.mjs`,
  `node tests/client_contracts/pixi_presentation_adapter_contracts.mjs`, renderer-feedback and Lab
  capture contracts, `node scripts/check-client-architecture.mjs`, and browser smoke.
- `assertion`: `Match` assembles one post-fog detached frame and calls `render(frame)`; the Pixi
  adapter samples its explicitly frozen private compatibility reads once per frame, reconciles
  decals before assembly, and keeps later frames alive after a render failure.
- `artifact`: ignored Lab captures are recorded in the relevant implementation commits; their image
  bytes are not a product gate.

## Decision after Phase 5

After the opt-in live/Lab slice is playable, capture the actual playtest limitation in a short
handoff and create at most one new targeted plan. No status in this ledger authorizes a default
switch or treats a prototype/placeholder as Pixi parity.
