# Phase 4 - Authoritative Presentation Event Contract

## Phase Status

- [ ] Not started.

## Depends On

- Phase 3 merged with the least-privilege renderer frame and borrowed-frame lifetime.

## Objective

Normalize real already-received fog-filtered presentation events before any renderer consumes them.
Give both backends one event identity, spatial pose, seed, lifetime, layering, deduplication, and
reset policy so a renderer cannot resolve old ids against future or hidden state. Preserve current
Pixi event visuals while preparing safe bounded retention for Phase 5.

## Work

- Inventory every current transient presentation source, including snapshot events, shot reveals,
  state visual-effect queues, decals, notices that carry world positions, and any renderer-local
  derivation. Classify persistent state separately from one-shot/finite events so the normalized
  stream does not duplicate smoke/ability objects already represented as current state.
- Create a plain `PresentationEvent` contract with stable diagnostic identity, kind, deterministic
  seed where randomness is visual, received/source timeline data, visual start, finite lifetime,
  semantic layer/fog policy, ownership/team presentation, and already-authorized payload.
- Make every retained-capable spatial event self-contained at receipt/reconciliation time. Capture
  the authorized world position, facing, muzzle/impact/attachment pose, dimensions, and other
  renderer inputs then; an event may retain a source id for diagnostics but later rendering or
  replay must never look that id up in a newer frame where it moved, disappeared, or became hidden.
- Derive missing identity deterministically from shared snapshot/replay context such as match/run
  generation, authoritative tick, event index/kind, and safe source discriminator. Do not add a
  protocol field unless current evidence proves a collision cannot be solved client-side; stop and
  request scope before any wire change.
- Reconcile and deduplicate once in shared client code before `RendererFrame` is finalized. Pixi
  and future Babylon receive the same active event records and cannot consume, start, seed, expire,
  or mutate a shared queue independently.
- Define pause and prediction behavior explicitly. Gameplay pause may freeze the visual clock as
  already designed, while receipt/network time remains real; optimistic presentation cannot create
  an event that later leaks or double-fires when authority arrives.
- Define replay seek/branch, vision perspective change, Lab reset, live reset, rematch, and destroy
  semantics. Old-timeline events and ids cannot survive into a new generation; same-tick replay
  reconstruction remains deterministic.
- Preserve visibility authority. Only received fog-filtered data and safe derived presentation
  values enter an event; no normalizer queries hidden/full-world state, and diagnostics omit data
  the recipient did not receive.
- Feed normalized active events through the Phase 3 least-privilege renderer submodel and the named
  Pixi compatibility adapter. Record any remaining renderer-local event derivation as explicit
  ledger debt rather than allowing Babylon access to it.
- Add bounded diagnostics for admitted/deduped/expired/reset/dropped-invalid events by kind, without
  logging positions or ids that are not part of the recipient's presentation data.

## Expected Touch Points

- a focused presentation-event normalizer/store module
- `client/src/state_visual_effects.js`
- `client/src/state_ground_decals.js` only where an event starts a persistent mark
- `client/src/match.js` snapshot/event handling
- `client/src/frame_recovery.js` and Phase 3 frame assembly
- Pixi feedback/effect compatibility adapter
- replay seek, Lab reset, pause, prediction, and rematch collaborators
- `tests/client_contracts/presentation_event_contracts.mjs`
- durable rendering/client design docs and parity ledger
- `plans/render3d/phase-4.md` status update in the implementation commit

## Event Contract Requirements

- Retained-capable events are spatially self-contained and immutable after admission.
- Event identity is stable within a match/replay generation and cannot collide silently.
- Visual randomness uses an explicit deterministic seed; renderer-global random state is not part
  of parity or capture semantics.
- Expiration is sampled from the injected visual clock and finite lifetime, not renderer frame count.
- Renderer failure or an extra capture render cannot consume or restart an event.
- Reset/seek/rematch changes generation and clears/rebuilds events exactly once.
- Semantic layering/fog policy is explicit; Babylon/Pixi do not decide visibility from effect kind
  or source mesh availability.

## Explicit Exclusions

- No retained history or replay API; Phase 5 owns it.
- No Babylon backend, particle system, showcase timer, or new visual effect.
- No artificial lifetime extension and no global time patch.
- No protocol/server visibility change.

## Implementation Checklist

- [ ] Inventory and classify transient versus persistent presentation sources.
- [ ] Implement immutable normalized event identity, pose, seed, timing, layer, and payload.
- [ ] Capture authorized spatial anchors at receipt; remove future-state id lookups for retained events.
- [ ] Reconcile/deduplicate/expire/reset once before renderer consumption.
- [ ] Preserve Pixi behavior through the compatibility adapter and ledger remaining derivation debt.
- [ ] Add pause/prediction/replay/Lab/rematch/no-leak contracts and diagnostics.
- [ ] Update durable docs/ledger and mark this phase done in the implementation commit.

## Verification

    node tests/client_contracts/presentation_event_contracts.mjs
    node tests/client_contracts/presentation_frame_contracts.mjs
    node tests/client_contracts/renderer_feedback_contracts.mjs
    node scripts/check-client-architecture.mjs
    tests/run-all.sh --only-browser-scenarios=smoke
    git diff --check

## Manual Test Focus

Run real Pixi combat events through live play, pause/unpause, replay seek backward/forward, vision
changes, Lab reset, and rematch. Confirm effects neither duplicate nor follow a later source pose,
expire at their current lifetimes, preserve recipient visibility, and leave persistent decals/state
only where the existing semantics require it.

## Handoff Expectations

Report event sources/classification, identity derivation, receipt-time spatial fields, seed/lifetime/
layer policy, reset generation, deduplication diagnostics, and remaining Pixi-local debt. Name Phase
5 as next and provide the chosen real short event kind, safe retention descriptor, proposed history
bound, monotonic capture offsets, and reset/error cases it must prove.
