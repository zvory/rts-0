# Phase 8 - Renderer-Owned Resource Registry

## Phase Status

- [ ] Not started.

## Depends On

- Phase 7 merged with the production coordinate/loader path and validated minimal asset fixtures.

## Objective

Make Babylon resource ownership explicit before fog, effects, shadows, vegetation, or broad assets
exist. Implement hierarchical scopes and generation-safe async loading so child teardown cannot
destroy shared dependencies or resurrect a destroyed scene. Prove the specific later-effect shared
particle-texture failure mode and return live diagnostics to baseline across repeated cycles.

## Work

- Implement explicit registry scopes:
  - backend/scene root for engine, scene, canvas, caches, loaders, shaders, and future shadow infrastructure;
  - shared asset for source containers/GLBs, textures, materials, base meshes, and templates;
  - entity instance for instantiated nodes, per-entity animation state, and unique overrides;
  - effect instance for emitter/system state and finite lifetime; and
  - named pools for reusable systems/instances whose dependencies remain root/shared-owned.
- Give every disposable object exactly one owning scope and documented destroy order. Prefer
  explicit handles/leases to recursive engine disposal; reference counts are allowed only for
  genuinely shared lifetime and must cover double/missing release.
- Entity/effect release may stop/dispose only owned instances. It cannot dispose shared texture,
  material, source mesh/container, shader, loader resource, or future shadow infrastructure; final
  root destroy releases children, pools, shared assets, and root in documented order.
- Guard every asset/texture/shader load with backend generation/cancellation. Completion after
  reset/destroy releases or ignores results without attaching to a new generation that reused a key.
- Report key/type, scope/owner, lease count, instances, pooled/active state, pending loads, last
  acquire/release, generation, and bounded leak/error summary. Scoped tests return to documented
  root-only state; backend destroy returns to zero.
- Implement a minimal finite event-effect fixture through the registry with one shared particle
  texture/material. Configure disposal so stopping/removing the first effect cannot destroy the
  shared dependency; create later effects repeatedly and verify their visuals/diagnostics.
- Distinguish pool return from disposal and reset emitter, transform, visibility, callbacks, event
  data, clock state, and diagnostics on return. Pool capacity/overflow optimization remains Phase 11.5.
- Exercise missing/malformed assets, failed loads, destroy during load, reset, entity replacement,
  effect completion, double destroy, and rematch. Use fake-disposable pure tests plus browser cycles.
- Use `lab-interact` with explicit `RTS_CLIENT_DIR` to inspect repeated effect/entity creation and
  disposal once; no capture bytes are committed.

## Expected Touch Points

- `client/src/renderer_babylon/resources/`
- Babylon kernel, asset loader, entity fixture, effect fixture, and readiness diagnostics
- `tests/client_contracts/babylon_resource_contracts.mjs`
- `tests/client_contracts/babylon_lifecycle_contracts.mjs`
- browser lifecycle/effect smoke coverage
- durable rendering docs/parity ledger
- `plans/render3d/phase-8.md` status update in the implementation commit

## Ownership Requirements

- Every resource has one owning scope and one idempotent release path.
- Child disposal never recursively owns a shared dependency.
- Pool return is not disposal and leaves no entity/event-specific state.
- Late completion cannot attach across generation or keep a destroyed canvas/scene alive.
- Fake-disposable tests prove destroy order/counts independently of browser garbage collection.
- Registry diagnostics reach expected root-only/zero baselines after scoped/root destruction.

## Explicit Exclusions

- No fog/remembered/reveal resources; Phases 9 and 9.5 own them.
- No production overlay/effect library; Phase 10.5 owns one real event effect.
- No capacity-tuned pooling/batching, vegetation, shadows, quality tiers, or representative GLB.
- No faction conversion or default switch.

## Implementation Checklist

- [ ] Implement root/shared/entity/effect/pool scopes and documented destroy order.
- [ ] Add generation-safe late-load cancellation and live ownership diagnostics.
- [ ] Cover double/missing release, partial construction, reset, and idempotent root destroy.
- [ ] Prove later effects survive earlier effect disposal with shared resources intact.
- [ ] Exercise repeated browser lifecycle and inspect one Lab Interact artifact.
- [ ] Update durable docs/ledger and mark this phase done in the implementation commit.

## Verification

    node tests/client_contracts/babylon_resource_contracts.mjs
    node tests/client_contracts/babylon_lifecycle_contracts.mjs
    node scripts/check-client-architecture.mjs
    tests/run-all.sh --only-browser-scenarios=smoke
    git diff --check

## Manual Test Focus

Create/finish/recreate the finite effect repeatedly, remove/recreate its entity fixture, reset the
map, destroy during one late load, and leave/re-enter twice. Confirm later effects retain their
texture/material, fallbacks remain bounded, pool returns are clean, and registry/canvas/context/
pending-load counts reach their documented baselines.

## Handoff Expectations

Report the scope table/destroy order, handles/leases, generation policy, before/after registry
counts, shared-effect survival, exact preview command/URL, and inspected artifact. Name Phase 9 as
next and identify fog texture ownership, locked semantic layers, current/client-explored state,
view generation, replay/spectator/Lab resets, and controlled core-fog capture.
