# Phase 13 - Representative GLB Integration

## Phase Status

- [ ] Not started.

## Depends On

- Phase 12.5 merged with measured batching/pooling, vegetation, shadows, quality tiers, and secrecy.

## Objective

Generate and integrate the exact repository-authored neutral tracked-vehicle fixture locked by this
plan. Prove the real manifest/loader/articulation/team/effect/shadow/resource/fallback/budget path
without selecting third-party art or expanding into a faction. Leave the all-scenario and ten-cycle
foundation gate to Phase 13.5.

## Work

- Add `scripts/art/generate-render3d-foundation-glb.mjs`, which deterministically emits one neutral
  tracked vehicle with the locked 50.4-by-28.8-world-px semantic hull, turret, independently
  articulated barrel, team-color material slot, muzzle/selection/HP anchors, visible bounds, and
  shadow proxy. Use repository-authored primitive geometry/material data only: no network, AI
  generation, third-party model, remote/data URI, compressed extension, or decoder.
- Check in the generator, human-readable source parameters, provenance/license record, manifest,
  and generated GLB. Regeneration into a temporary path must byte-match the checked-in GLB and its
  manifest checksum.
- Validate runtime scale/pivot/axes, visible bounds/anchors, hierarchy/part transforms, team slot,
  declared animation/part behavior, muzzle/effect anchor, shadow proxy, fallback, and Phase 7
  security/budget rules. Gameplay selection continues to use Phase 2 data.
- Load/instantiate/share/remove/recreate the asset through Phase 8 scopes and Phase 11.5 content
  route. A malformed/missing copy falls back truthfully without changing selection or stopping the
  frame.
- Drive the normalized attack event through the muzzle anchor and Phase 10.5 effect path at
  `0/80/160/240` ms. Prove fog/reveal secrecy, finite lifetime, pooled/shared resource ownership,
  and later-effect survival.
- Run the representative-asset scenario three times at the Phase 0 fixed environment and compare
  incremental draws/materials/textures/triangles/memory/casters/registry/timing with the generic
  fallback in the same scene/device/tier. Apply existing formula-based budgets; do not relax a
  ceiling to make the asset pass.
- Use `lab-interact` to capture the asset at normal gameplay distance in the exact fog/core-overlay/
  event/shadow scene and inspect one PNG once.
- Update durable contract/parity/budget evidence for the asset only.

## Expected Touch Points

- `scripts/art/generate-render3d-foundation-glb.mjs`
- one neutral generated source/GLB/manifest/provenance asset directory
- Babylon asset/entity/articulation/team/effect/shadow integration
- representative asset benchmark scenario
- `tests/client_contracts/babylon_foundation_asset_contracts.mjs` (create it in this phase)
- asset validator/resource/fog/interaction/shadow/capture contracts
- dedicated Babylon asset browser coverage wired into the authoritative runner
- durable rendering docs/parity ledger
- `plans/render3d/phase-13.md` status update in the implementation commit

## Asset Acceptance Requirements

- Generator output and checksum are deterministic and network-free.
- Provenance states repository-authored source and applicable repository license; no external art is acquired.
- Articulation, team slot, anchors, shadow proxy, fallback, selection independence, and budgets pass.
- Asset versus generic deltas use identical scenario/device/tier metadata.

## Explicit Exclusions

- No second asset, faction conversion, broad content wave, finished-art claim, default switch, Pixi
  freeze/removal, or final go/revise/stop decision.

## Implementation Checklist

- [ ] Add deterministic generator/source/provenance/manifest/GLB and byte-match verification.
- [ ] Integrate articulation/team/anchors/effect/shadow/fallback without asset-driven selection.
- [ ] Measure three same-environment asset-versus-placeholder repetitions against existing budgets.
- [ ] Capture/inspect one final asset PNG and update durable evidence.
- [ ] Mark this phase done in the implementation commit.

## Verification

    node scripts/art/generate-render3d-foundation-glb.mjs --check
    node scripts/validate-rendering-assets.mjs --all
    node tests/client_contracts/babylon_foundation_asset_contracts.mjs
    node tests/client_contracts/babylon_resource_contracts.mjs
    node tests/client_contracts/babylon_visibility_contracts.mjs
    node tests/client_contracts/babylon_shadow_contracts.mjs
    node tests/client_contracts/presentation_capture_contracts.mjs
    node scripts/rendering-benchmark.mjs --backend babylon --scenario representative-asset --repeat 3 --output target/rendering-benchmarks/phase-13.json
    node scripts/check-client-architecture.mjs
    tests/run-all.sh --only-browser-scenarios=smoke
    git diff --check

## Manual Test Focus

At normal gameplay distance, inspect articulation/facing/team color/anchors, selection independence,
fog/reveals, event offsets, shadow proxy, quality tiers, resize/DPR, fallback, and recreate. Use
automated regeneration, budget, secrecy, and resource assertions as the acceptance gate.

## Handoff Expectations

Include generator/source/provenance/manifest/GLB paths and checksums, scenario/report hash,
placeholder/asset deltas, budget result, resource/effect evidence, preview URL/command, and inspected
PNG. Name Phase 13.5 as next and identify all-scenario reports, true same-page ten-cycle lifecycle,
ledger audit, and block-on-failed-gate behavior.
