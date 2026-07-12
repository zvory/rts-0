# Phase 6 - Representative Asset and Effect Spine

## Phase Status

- [ ] Not started.

## Depends On

- Phase 5 merged with an opt-in playable Babylon route, and the user explicitly approved this
  content slice after reviewing the Phase 5 playtest.

## Objective

Exercise the content boundaries with one representative repository-owned vehicle and one finite
attack/muzzle effect. Establish only the asset metadata, resource ownership, and event data needed
by those real examples; do not generalize beyond demonstrated needs.

## Work

- Add a small trusted asset descriptor containing id/path, source/license note, scene scale,
  up/forward convention, ground pivot, visible bounds, team-material slot, and named muzzle/HP
  anchors. Reject missing required fields with a truthful generic fallback.
- Integrate one repository-owned representative tracked vehicle with hull/turret/barrel hierarchy
  and team color. Gameplay selection continues to use semantic proxies, not asset bounds.
- Keep ownership explicit and small: backend root owns the loaded source asset, materials, and
  textures; entity instances own only instantiated nodes. Destroying an entity cannot destroy a
  shared source. Add a registry only if this implementation proves simple ownership insufficient.
- Add a minimal immutable presentation-event record for the attack/muzzle effect: kind, authorized
  position/facing/anchor, start time, finite lifetime, seed, layer, and payload. It is reconciled
  before the backend and never resolves a hidden or future entity id.
- Render one finite muzzle/attack effect and prove it expires, respects fog/layer policy, survives
  removing/recreating the representative entity, and does not start another clock or loop.
- Record simple scene counters before/after the representative asset/effect on the same machine.
  Optimize only an obvious per-entity resource duplication; do not create benchmark schemas or
  budgets.
- Use `lab-interact` to capture and inspect the representative fogged asset/effect scene.

## Keep Small

- No deterministic asset generator, byte-identical GLB gate, hostile/untrusted-input validator,
  compression/decoder pipeline, broad asset catalog, retained event history, effect-capture tool,
  generalized pool, fixed benchmark suite, vegetation, shadows, or quality tiers.
- No claim of final art or Pixi parity.

## Acceptance

- One trusted asset validates scale, hierarchy, anchors, team color, fallback, selection
  independence, and simple shared-resource ownership.
- One self-contained received event validates the cross-backend event boundary and finite effect
  lifecycle without hidden-state lookup.
- Simple observed counters show no accidental per-entity source/material/texture duplication.
- The inspected capture and playtest handoff identify any justified next plan; this plan ends here.

## Verification

Run focused asset-descriptor, ownership, event, fog, and lifecycle contracts added by the phase,
then:

    node scripts/check-client-architecture.mjs
    tests/run-all.sh --only-browser-scenarios=smoke
    git diff --check

## Manual Test and Handoff

Inspect the representative vehicle at ordinary gameplay distance, select and command it, trigger
the attack effect near a fog edge, remove/recreate it, and leave/re-enter once. Report the asset and
event shapes, ownership behavior, fallback, observed counters, capture path, visual limitations,
and any evidence-backed follow-up. Mark this phase done and let the normal PR workflow archive the
plan.
