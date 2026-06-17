# SVG-Authored Unit Rig Plan

## Purpose

Move unit visuals from hardcoded PixiJS draw branches to SVG-authored, named-part rigs compiled
into Pixi containers at runtime. SVG is the authoring and interchange format, while the match
renderer remains Pixi-owned so camera, fog, layers, pooling, replay, and game-state-driven
animation stay in one rendering system. The migration must be automatically verifiable: while both
renderers exist, a temporary equivalence harness compares legacy procedural units against rigged
units across static poses and animation samples, then the harness is removed after the migration is
complete.

## Why Phase 5.x Exists

The first Phase 5 Worker migration proved the SVG runtime path could be wired into live gameplay,
but it did not prove visual equivalence strongly enough. Manual review caught issues the automated
checks missed: Workers used to be static outlined pentagons whose facing line rotated, while the
new rig made the body rotate, changed the body silhouette, and dropped the dark outline. That is a
process failure in the migration gate, not a request for more human visual review.

Phase 5.x exists to make unit conversion mechanical and practical for an LLM executor. The
executor should receive small, objective failures such as `worker.body missing outline`,
`worker.body rotated across facing samples`, or `worker.facingTick pixel mismatch`, then iterate on
SVG parts, bindings, and draw order until deterministic pixel gates pass. Human or multimodal
review can help diagnose a failed artifact, but acceptance must come from Pixi-vs-Pixi transparent
buffer comparisons, named part gates, and explicit manifests.

## Target Architecture

- Source unit art lives as SVG files with stable part ids and rig metadata conventions.
- A small compiler/importer turns SVG documents into normalized rig definitions with explicit
  parts, transforms, pivots, anchors, tint slots, bounds, and animation bindings.
- The live renderer consumes only the normalized rig API, not raw SVG DOM nodes.
- Runtime unit art is a `PIXI.Container` per unit with one child per compiled rig part, using
  `PIXI.Graphics`, cached textures, or sprites behind a narrow part-renderer boundary.
- Live renderer routing is distinct from the temporary equivalence/comparison seam. Production
  routing owns kind-to-renderer selection, live rig pools, shot-reveal instances, and fallback
  gating; it must not depend on `_rigComparisonEnabled`, `_rigComparisonPool`, or a test-only
  comparison layer.
- JS animation remains game-state-driven from entity snapshots: `facing`, `weaponFacing`,
  `setupState`, recoil progress, movement deltas, team color, visibility, and ability state.
- Legacy procedural draw functions stay available only until their migrated unit kinds pass the
  temporary equivalence suite.
- Rigs own unit body and shadow parts only. Selection rings, HP bars, fog, feedback overlays,
  placement, command markers, and hit-testing remain outside rig definitions. If another renderer
  area needs an anchor such as `muzzle`, expose a narrow anchor lookup rather than importing rig
  internals.

## Phase Summaries

Phase 0 locks the rendering contract before implementation. It inventories the current procedural
unit anatomy, animation inputs, draw layers, pooling behavior, and unit-lab assets, then writes the
approved rig API and equivalence-spec boundaries into the phase notes. The outcome is a concrete
contract for what the SVG compiler, Pixi rig runtime, and temporary migration harness must prove.

Phase 1 creates the temporary visual equivalence harness while legacy rendering is still the only
runtime path. It renders legacy units in deterministic headless fixtures across representative
facings, weapon facings, recoil values, setup states, movement deltas, team colors, and visibility
states, then stores expected measurement baselines rather than fragile screenshots. The outcome is
a testable oracle that later phases can compare against when rigged units are introduced.

Phase 2 adds the normalized rig schema, validator, and client architecture guardrails without
rendering any live unit from a rig. It defines stable APIs for rig definitions, parts, pivots,
anchors, tint slots, animation bindings, and semantic bounds, plus focused tests for invalid SVG or
metadata failing closed. The outcome is a narrow, documented data contract that prevents unit art
from becoming ad hoc JSON or direct cross-module renderer state.

Phase 3 implements SVG import and fixture tooling for authored rigs. It parses approved SVG
conventions into normalized rig definitions, extracts named groups and metadata, verifies required
anchors such as muzzle and selection bounds, and emits deterministic local fixtures for the lab and
tests. The outcome is an SVG-authored source path that can be inspected by humans and validated by
automation before touching the live renderer.

Phase 4 builds the Pixi rig runtime behind a dormant renderer seam. It compiles normalized parts
into pooled Pixi containers, applies team tint and per-frame transforms, samples animation bindings
from game-state inputs, and exposes a side-by-side test-only path that draws legacy and rigged
versions under the same fake entity state. The outcome is an inactive runtime capable of matching
legacy output in tests without changing normal gameplay visuals.

Phase 5 migrates one simple unit kind end to end, preferably the Engineer/Worker. It authors the
SVG rig, compiles it through the new path, enables it behind a per-kind feature gate, and proves
static pose, facing, selection bounds, shadow, health bar placement, busy indicator, and movement
samples against the equivalence harness. The outcome is the first live rigged unit with rollback to
the legacy draw path still available.

Phase 5.1 adds the transparent pixel-buffer harness that should have existed before the first live
unit gate. It renders legacy and rig output through Pixi into fixed transparent buffers, compares
RGBA data mechanically, and writes failure artifacts only when a sample fails. The outcome is a
reproducible visual gate that lets an LLM iterate against numbers and artifacts without asking a
human whether the unit is close enough.

Phase 5.2 adds named part capture and part-level comparison. It gives legacy procedural drawing
stable part names, renders matching rig parts in isolation, and compares each part before checking
the whole composed unit. The outcome is smaller, mechanical failures such as missing outline,
rotated body, wrong primitive, wrong alpha, or wrong draw order, which are much easier for an LLM
to fix than whole-unit screenshot drift.

Phase 5.3 re-migrates Worker through the new gates. It fixes the known Worker mismatch so the body
remains the legacy static outlined pentagon while only the facing tick rotates, then requires both
part-level and full-composition pixel gates to pass. The outcome is a live Worker rig whose
acceptance is mechanical rather than manual, proving the process before Phase 6 starts.

Phase 5.4 turns the new gates into future migration guardrails. It adds per-unit migration
manifests, live-routing checks, and a small legacy part metadata dump tool so Phase 6 and Phase 7
executors can migrate units by fixing named mechanical failures. The outcome is an enforceable
workflow where no new unit kind becomes live-rigged without passing part and composition gates.

Phase 6 migrates infantry and support-weapon units. It converts Rifleman, Machine Gunner,
Anti-Tank Gun, Mortar Team, Artillery, and Ekat as applicable, including weapon-facing separation,
setup/deploy animation, recoil, muzzle anchors, and special owner-only visual affordances. The
outcome is broad coverage for strict top-down humanoid and crew-served rigs while equivalence tests
continue to guard animation behavior.

Phase 7 migrates vehicle-body units. It converts Scout Car, Command Car, and Tank with hull vs
weapon-facing separation, track/wheel movement phases, recoil, fuel cues, breakthrough ring
attachment, shadows, and selection/hp bounds. The outcome is full unit-kind coverage through the
rig renderer while the legacy procedural implementation remains only for equivalence comparison.

Phase 8 flips enforcement and removes the discarded migration scaffolding. It deletes legacy unit
draw branches and the temporary pixel/measurement equivalence harness, keeps permanent schema,
anchor, architecture, and smoke coverage, and documents the SVG authoring workflow for future unit
art. The outcome is a clean, enforced SVG-authored rig pipeline with no long-term duplicate
renderer burden.

## Phase Index

1. [Phase 0 - Contract and Current Anatomy Inventory](phase-0.md)
2. [Phase 1 - Temporary Legacy Visual Oracle](phase-1.md)
3. [Phase 2 - Normalized Rig Schema and Guardrails](phase-2.md)
4. [Phase 3 - SVG Importer and Authoring Fixtures](phase-3.md)
5. [Phase 4 - Dormant Pixi Rig Runtime](phase-4.md)
6. [Phase 5 - First Live Rigged Unit](phase-5.md)
7. [Phase 5.1 - Transparent Pixel Buffer Harness](phase-5.1.md)
8. [Phase 5.2 - Named Part Capture and Part-Level Gates](phase-5.2.md)
9. [Phase 5.3 - Worker Re-Migration Through Pixel Gates](phase-5.3.md)
10. [Phase 5.4 - Future Unit Migration Guardrails](phase-5.4.md)
11. [Phase 6 - Infantry and Support Weapon Migration](phase-6.md)
12. [Phase 7 - Vehicle Migration](phase-7.md)
13. [Phase 8 - Enforcement and Harness Removal](phase-8.md)

## Overall Constraints

- Keep the match renderer Pixi-owned. Do not mount live SVG DOM into the game world or introduce a
  second camera/fog/layer stack.
- SVG is an authoring source, not the unchecked runtime API. The runtime consumes validated,
  normalized rig definitions through a narrow renderer seam.
- Keep the no-build-step ES module client unless a phase explicitly updates the approved plan.
  SVG import must work through browser-native parsing, checked-in generated fixtures, or a small
  repo-local script that does not require a JS bundler.
- Preserve server authority and wire protocol. Unit rigs must depend only on existing client entity
  state and renderer-local animation state unless a later approved gameplay feature changes the
  protocol.
- Preserve fog authority. Rig anchors, muzzle flashes, shot reveals, selection rings, and health
  bars must not leak hidden entity positions or target positions beyond existing renderer behavior.
- Preserve strict top-down readability. Rigs may use parts, pivots, and rotations, but not
  perspective tricks that make collision, facing, weapon facing, or selection bounds misleading.
- Keep boundaries testable. SVG parsing, rig validation, Pixi compilation, animation sampling, and
  live renderer selection must each have focused tests rather than one broad screenshot-only test.
- Make invalid art fail closed. Missing required parts, duplicate ids, invalid colors, unsupported
  SVG features, non-finite transforms, or missing anchors should reject the rig and report a clear
  error instead of silently falling back in shipped paths.
- Equivalence testing is temporary. It exists only while legacy and rigged renderers coexist, and
  Phase 8 must delete it after every unit kind has migrated and permanent contract tests exist.
- Equivalence tests should compare a mix of semantic measurements and bounded pixel diffs. Prefer
  stable checks for bounds, anchors, transforms, draw-layer membership, and animation sample
  positions, with pixel tolerances reserved for visual drift.
- Pixel equivalence gates must compare Pixi-rendered transparent buffers for legacy and rig paths.
  Do not use raw browser SVG rendering as the oracle for runtime visuals.
- Part-level visual gates must pass before full-unit composition gates for migrated unit kinds.
  Full-unit tolerances must not be loosened to hide named part failures.
- Human or LLM review may diagnose failures, but pass/fail for migrated unit visuals must come from
  deterministic mechanical gates or an explicitly recorded intentional art change.
- Do not require exact artistic identity forever. During migration, each rig must match the legacy
  renderer within the approved spec; after Phase 8, future art changes should be reviewed as normal
  art changes through schema, smoke, and manual visual checks.
- Keep unit-lab compatibility in mind. The lab can become the SVG rig preview/edit loop, but this
  plan should not depend on OpenAI generation or ignored local attempt files for core tests.
- Avoid broad test bundles during development. Each phase should run targeted JS architecture,
  renderer, SVG parser, or browser smoke checks that match touched files; rely on the commit hook
  only for merge-ready commits.

## Required APIs

- `validateRigDefinition(definition)` returns normalized data or structured errors and is pure.
- `compileSvgRig(svgText, metadata)` returns a normalized rig definition and never creates Pixi
  objects.
- `createUnitRigInstance(kind, definition, pixiFactory)` creates the runtime container and owns its
  Pixi children.
- `UnitRigInstance.update(entity, renderContext)` applies per-frame transforms, tint, alpha, and
  animation samples without reading global state.
- `sampleRigAnimation(definition, entity, renderContext)` is pure enough for unit tests and the
  equivalence harness.
- `renderContext` is narrow and explicit: team color, current time, recoil progress, setup visual,
  movement phase, fog/visibility flags, and any unit-kind-specific visual state already used by
  the renderer.

## Implementation and Handoff Rules

Implement one phase at a time. Each phase should be committed, merged to `main`, and pushed before
the next phase begins. When a phase is complete, mark that phase document as done in the same
implementation commit.

After each phase, the implementing agent must provide a handoff message describing what the next
agent should do and what should be manually tested. Manual testing notes should cover the core
features for that phase, not an exhaustive test matrix, and must call out any temporary legacy
equivalence thresholds that were added or changed.
