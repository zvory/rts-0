# Phase 3 - Add the Typed Authoring Seam

## Phase Status

- [x] Done.

## Objective

Add a public, fog-safe `rts_ai::sdk` centered on a typed `AiFrame` and an object-safe `AiStrategy`
lifecycle. Keep raw transport DTOs and wire parsing inside one adapter, while a crate-private
`LegacyProfileStrategy` recreates the historical observation and action semantics for Jeff. This is
an API extraction only: no simulation, protocol, cadence, command-order, or gameplay changes.

## Public Surface

- Add a curated `server/crates/ai/src/sdk/` module with at least frame and strategy modules.
- Expose an object-safe `AiStrategy: Send` lifecycle with initialization exactly once per controller
  and deterministic `step` calls only on the canonical production cadence. Avoid generics,
  associated types, `async`, required constructors, or checkpoint requirements.
- Expose a small `AiActions` sink for Phase 3. It accepts a bounded SDK-owned action request enum in
  call order, while only the host adapter translates finalized requests into ordinary
  `SimCommand`s; transactional budgets, reservations, task handles, and richer typed helpers belong
  to later phases.
- Define an owned/normalized, read-only `AiFrame` with typed access to:
  - player, tick, team, alive/AI summaries, economy, and completed upgrades;
  - map dimensions, tile size, terrain, public start positions, and static resource locations;
  - owned entities, currently visible allies/enemies, and separately labeled remembered contacts;
  - typed kind/state/position/health/completion/production/latch/target information;
  - inferred submitted-build bookkeeping clearly labeled as controller observation, not acceptance;
  - resource knowledge where a public static location may have unknown remaining quantity.
- Do not expose synthetic `remaining = 1` or derived `free_for_combat` as truthful public facts. Use
  `Option` or a small knowledge enum for unknown amounts, and leave policy derivations to consumers.
- Keep `AiController` as the stable host-facing facade and add a supported custom-strategy
  construction path that uses the same runtime envelope as profiles.

## Legacy Compatibility Projection

- Keep `AiObservation` internal and add a crate-private `LegacyProfileStrategy` that projects
  `AiFrame` back into the exact old shape.
- Preserve missing-player/start behavior, ordering/deduplication, team-zero semantics, invalid-kind
  skips, `vision_only` filtering, production optionality, completion/state parsing, target handling,
  pending-build sorting, upgrade sorting, and `free_for_combat` derivation.
- Preserve the old synthetic unseen-resource amount and override order inside the compatibility
  projection even though public `AiFrame` represents the amount as unknown.
- Preserve the current legacy `is_ai` projection value even if `AiFrame` exposes the correct public
  start-payload value.
- Keep decision memory, map-analysis cache, live build search, cadence, retreat ordering, trace
  formatting, pending-build behavior, and stage suppression at their canonical Phase 2 owners.
- Retain the old direct observation constructor under tests for this phase so field-for-field
  projection equivalence can be proved before cleanup.

## Security and Determinism

- Construct frames only from public start data, `snapshot_for(player)`, the selected public/alive
  host state, and controller-owned state. A recipient-event stream is explicitly deferred rather
  than adding a new live/offline host contract in this phase.
- Never accept `snapshot_full_for`, entity stores, global event buckets, private fog/occupancy/path
  state, or detailed hidden-blocker reasons.
- Stable-sort every normalized collection and define deterministic tie/order behavior in the public
  API documentation.
- Custom strategy output remains untrusted and passes through ordinary simulation validation and
  replay logging.

## Expected Touch Points

- New `server/crates/ai/src/sdk/{mod.rs,frame.rs,strategy.rs}`.
- `server/crates/ai/src/lib.rs`.
- `server/crates/ai/src/live.rs` or the canonical controller/runtime module left by Phase 2.
- `server/crates/ai/src/ai_core/observation.rs`.
- Focused SDK and outside-module integration tests.
- `docs/design/ai.md`.

Do not touch `rts-sim`, wire DTOs, protocol mirrors, client code, balance, or lobby scheduling.

## Implementation Checklist

- [ ] Add the public object-safe strategy lifecycle and custom-strategy entry point.
- [ ] Add the honest typed `AiFrame` without raw wire strings or fake knowledge.
- [ ] Centralize raw start/snapshot parsing in one adapter.
- [ ] Add `LegacyProfileStrategy` and preserve every old observation quirk.
- [ ] Retain the old observation path temporarily as a test oracle.
- [ ] Add secrecy, lifecycle, projection-equivalence, and external-consumer tests.
- [ ] Pass the unchanged Phase 1 transcript fixture.
- [ ] Document the public lifecycle and fog boundary.
- [ ] Mark this phase done in the implementation commit.

## Verification

- Compare direct legacy observation against `AiFrame -> AiObservation` field for field across every
  transcript invocation plus focused teams, dead players, invalid kinds, `vision_only`, queues,
  pending-build, upgrades, and resource-visibility fixtures.
- Prove with a recording `Box<dyn AiStrategy>` that initialization happens once, steps occur only on
  canonical decision ticks, all controllers observe before enqueue, and retreat commands stay first.
- Add a real-`Game` fog A/B test: hidden entities never enter the frame, unseen resource amounts are
  unknown, and memories are not current contacts.
- Add an external-style integration test using only public SDK imports and the custom-strategy
  runtime entry.
- Run the exact Phase 1 normal/full transcript suite without fixture regeneration.
- Run focused `rts-ai` nextest, clippy, simulation archcheck, docs health, and diff check.

## Manual Test Focus

Run a fixed-seed release Jeff-versus-AI-2.1 matchup through the canonical runtime and inspect its
replay, decision trace, and map diagnostics. This is a smoke check only; exact transcript parity is
the acceptance authority.

## Non-Goals

- No Jeff policy rewrite or public `AiProfile` strategy language.
- No recipient-event stream, command acceptance/result lifecycle, task IDs, or idempotent goals.
- No rulebook, placement/path queries, transactional planner, or reservations.
- No behavior trees, GOAP, async strategies, checkpoints, plugin ABI, or dynamic loading.
- No new remembered-state prediction or simulation/protocol fields.

## Handoff Expectations

Report the public SDK imports and lifecycle, frame field/knowledge contract, adapter source allowlist,
legacy projection quirks, custom-strategy construction path, outside-crate smoke test, and unchanged
Jeff fixture identity. Tell Phase 4 which frame fields and compatibility hooks are safe for
observational task reconciliation.
