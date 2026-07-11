# Rendering parity and evidence ledger

This is the active evidence ledger for the renderer-neutral/Babylon foundations. Update it in every
phase that changes a fact. Runtime tests and policy read durable docs/code, never an archived plan.

## Status and evidence contract

The only status values are `shared external`, `Pixi complete`, `Babylon complete`,
`representative`, `placeholder`, `missing`, and `deferred`.

- `shared external` means the surface is intentionally outside both world backends.
- `complete` means the named backend implements the whole capability named by the row. Any phase
  transition to `complete` requires focused automated evidence and, when visible, one inspected
  artifact.
- `representative` proves the bounded foundation case, not catalog-wide parity.
- `placeholder` is truthful generic coverage and never counts as parity.
- `missing` has no implementation. `deferred` is intentionally outside this foundations plan.

Every evidence update has the exact fields `phase`, `commit`, `automated`, `assertion`, `artifact`,
`inspected`, and `notes`. `automated` is a repository command; `assertion` names the blocking fact;
`artifact` is a reproducible path/manifest or `none`; `inspected` is `yes`, `no`, or `n/a`.
Evidence cannot cite manual review for secrecy, authority, lifecycle counts, targeting, capture
clocks, pool/counter reset, budgets, or teardown.

Gate values are `required`, `not required`, or `deferred`. The content-expansion gate permits broad
asset/content waves; the default-cutover gate permits a reviewed proposal to make Babylon default.
A row passes a gate only when its required backend/status and automated evidence are present.

## Active ledger

`P0-docs` means Phase 0 documentation/inventory only; it is not an evidence update and cannot make
a row pass either gate. A backend column is `complete` only when that backend already implements
the whole capability named by the row; an existing visual or partial legacy path is `placeholder`
when the new shared contract is not yet implemented.

| Capability | Pixi | Babylon | Content gate | Default gate | Evidence / current fact |
| --- | --- | --- | --- | --- | --- |
| One Match-owned rAF/visual clock | Pixi complete | missing | required | required | P0-docs; current `frame_recovery.js` owns scheduling; Babylon assertion Phase 6.5 |
| Exact selector and Babylon-free default loading | placeholder | missing | required | required | Current default is Babylon-free but the exact selector/preflight does not exist; Phase 6 owns evidence |
| Semantic camera/projection and CSS-pixel contract | Pixi complete | missing | required | required | `P1-camera`; orthographic adapter implements the semantic API and immutable projection snapshot; perspective backend remains Phase 6.5 |
| Navigation, resize, minimap footprint/recenter | Pixi complete | missing | required | required | `P1.5-navigation-minimap`; gestures use semantic CSS-pixel pan/dolly and minimap uses ground polygon/focus; remaining resize/app-shell consumers are Phase 1.75 |
| Audio, alerts, control groups, carryover, Lab/diagnostics camera consumers | placeholder | missing | required | required | Current behavior still reads raw orthographic state; migration and ratchet Phase 1.75 |
| Perspective-safe picking/selection/marquee | placeholder | missing | required | required | Current Pixi is orthographic-only; Phase 2 owns proxies/SelectionScene |
| Detached least-privilege presentation frame | missing | missing | required | required | Current renderer reads mutable state/intent/fog; Phases 3/3.5 |
| Immutable terrain/fog grids and revision pinning | missing | missing | required | required | Contract frozen; implementation Phase 3 |
| Static terrain | Pixi complete | missing | required | required | Current cached Pixi terrain; Babylon Phase 7 |
| Permanent ground decals/trenches | Pixi complete | missing | not required | required | Long-tail Babylon presentation after foundations |
| Generic visible entities/buildings/resources | Pixi complete | missing | required | required | Babylon truthful placeholders Phases 7/10 |
| Representative tracked vehicle/articulation/team slot/anchors | Pixi complete | missing | required | required | Repository-generated representative fixture Phase 13 |
| Faction-wide final art catalog | Pixi complete | deferred | not required | required | Explicitly outside this plan |
| Remembered buildings | Pixi complete | missing | required | required | Presentation category Phase 9.5 |
| Fog secrecy and current/explored mask | Pixi complete | missing | required | required | Server-filtered inputs; Babylon implementation/assertions Phase 9 |
| Below-fog intel and above-fog reveal separation | Pixi complete | missing | required | required | Category/order/secrecy Phases 9/9.5 |
| Semantic layer order and occlusion | Pixi complete | missing | required | required | Descriptor frame Phase 3; Babylon Phases 9/9.5 |
| Selection/range/HP tactical readability | Pixi complete | missing | required | required | Representative Babylon coverage Phase 10.5; long-tail default review remains |
| Placement, command, entity marker, Lab area, marquee | Pixi complete | missing | required | required | Exact representative set Phase 10.5 |
| Normalized attack/muzzle finite event | placeholder | missing | required | required | Current Pixi has attack feedback but not the shared normalized identity/history contract; Phases 4/5/10.5/13 |
| Long-tail abilities/smoke/projectiles/impacts/toasts | Pixi complete | deferred | not required | required | Not content-expansion blocking; default parity work |
| Observer/Lab world diagnostics | Pixi complete | missing | not required | required | Fog-safe representative overlays before route unlock |
| HUD, minimap, lobby, panels, scoreboard, audio | shared external | shared external | required | required | Deliberately shared DOM/canvas/audio surfaces |
| Deterministic fixed/event capture | placeholder | missing | required | required | Current Pixi has fixed capture, not detached retained-event capture; Phase 5 owns Pixi completion |
| Capture readiness/soft error/fallback diagnostics | Pixi complete | missing | required | required | Backend-neutral readiness Phase 5; Babylon failure paths Phases 6.5-8 |
| Transactional create/reset/resize/destroy and late loads | placeholder | missing | required | required | Current Pixi has resize/destroy paths but Match construction is not transactional; shared lifecycle evidence starts in Phases 3.5/5 and Babylon Phase 6.5 |
| Shared resource registry and child-safe disposal | placeholder | missing | required | required | Current Pixi has local pools/caches; explicit registry Phase 8 |
| Repeated same-page rematch/context cleanup | Pixi complete | missing | required | required | Three-cycle benchmark Phase 11; ten-cycle foundation gate Phase 13.5 |
| Counter reset and benchmark metadata/schema | missing | missing | required | required | Exact scenarios frozen; harness and current-frame semantics Phase 11 |
| Batching, instances, bounded effect pools/budgets | missing | missing | required | required | Phase 11.5; budgets use measured formula |
| Instanced vegetation and quality densities | missing | missing | not required | required | Representative environment scale path Phase 12 |
| Shadows/caster secrecy/quality degradation | missing | missing | required | required | Representative structural path Phase 12.5 |
| Browser/device rollout matrix and accessibility/perf sign-off | Pixi complete | deferred | not required | required | Separate default-cutover plan; no universal one-machine gate |
| Babylon as default / Pixi removal | Pixi complete | deferred | not required | required | Explicit reviewed decision after full default gate; Pixi remains default |

## Phase evidence

### `P1-camera`

- `phase`: Phase 1.
- `commit`: Phase 1 implementation commit containing this evidence.
- `automated`: `node tests/client_contracts/camera_projection_contracts.mjs`.
- `assertion`: Pixi orthographic equivalence; finite rejection; CSS-pixel projection and extent;
  depth/clip priority; nullable fake-perspective ground hits; bounded deduplicated clockwise ground
  polygons and empty views; anchored dolly, pan, clamp, fit/focus, resize/map bounds; versioned and
  legacy restore; immutable projection snapshots; semantic listener and audio data.
- `artifact`: none.
- `inspected`: n/a.
- `notes`: This completes the Pixi semantic core only. Navigation/minimap consumers migrate in
  Phase 1.5, remaining shared consumers in Phase 1.75, and Babylon perspective evidence later.

### `P1.5-navigation-minimap`

- `phase`: Phase 1.5.
- `commit`: Phase 1.5 implementation commit containing this evidence.
- `automated`: `node tests/minimap_input_contracts.mjs && node tests/client_contracts/match_replay_contracts.mjs && node scripts/check-client-architecture.mjs`.
- `assertion`: Live/replay wheel and pinch use anchored semantic dolly; mouse, touch, middle/Space,
  pointer-lock, and fallback drags use semantic CSS-pixel pan deltas; CSS-scaled/non-1-DPR minimap
  input uses semantic focus; orthographic, empty, partial, and malformed semantic ground footprints
  draw without raw camera reads or invented bounds; the remaining raw-read allowlist is exact and
  stale-entry checked.
- `artifact`: none.
- `inspected`: n/a.
- `notes`: Pixi behavior remains equivalent. Phase 1.75 owns audio, alerts/control groups,
  app/replay carryover, profiles, Lab, observer/diagnostics, resize, and final shared-consumer ratchet
  closure.

## Gate interpretation

Broad content waves remain blocked until every `content gate = required` row has the required
Babylon automated evidence. In particular: fog/event secrecy, semantic camera/input/selection,
least-privilege frames, capture, asset validation/fallback, ownership/lifecycle, scale benchmarks,
structural budgets, representative overlays/effect/asset, and shadow caster secrecy must pass.

Making Babylon the default is not an outcome of this foundations plan. It additionally requires
long-tail visual/effect parity, browser/device rollout, accessibility/readability review, and an
explicit go/revise/stop plan. `representative` or `placeholder` never satisfies a default-cutover
row that requires complete catalog behavior.

## Benchmark and budget ledger

Scenario definitions, metadata, counters, warmup/sample/repeat policy, tier factors, and the budget
formula are frozen in [client-rendering.md](client-rendering.md#10-reproducible-benchmark-contracts).
Phase 11 adds schema v1 and unoptimized report hashes/summaries; Phase 11.5 adds comparable optimized
reports and formula ceilings. Generated reports remain ignored. Record durable entries here as:

```text
scenario | tier | viewport/DPR | definition hash | environment id |
report SHA-256 | structural maxima/ceilings | timing median/p95/warning | phase/commit
```

There are no Phase 0 numeric baselines or budgets. Historical PoC observations are non-binding
leads and must not be used as ledger evidence.
