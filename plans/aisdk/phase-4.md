# Phase 4 - Add Observational Task Feedback

## Phase Status

- [ ] Ready for implementation after Phase 3 merges.

## Objective

Add a deliberately small first command-lifecycle surface without pretending the simulation
correlated or accepted an AI command. Provide stable handles and conservative own-state feedback
for build, gather, move, and setup actions, while consolidating Jeff's existing pending-construction
and combat-stage bookkeeping under one private compatibility owner. Jeff continues to consume the
exact legacy semantics and must remain transcript-identical.

## Public Task Model

- Add deterministic `AiIntentId`, `AiTaskRecord`, `AiTaskStatus`, `AiTaskUpdate`, and a bounded
  read-only ledger view under `rts_ai::sdk`.
- Extend `AiActions` with an explicit `submit_tracked(TrackedAction) -> AiIntentId` path. IDs combine
  player ID with a monotonically increasing per-controller sequence starting at one, so a custom
  strategy receives its handle at submission without reverse-engineering the later command batch.
- Cover only `Build`/`ResumeBuild`, `Gather`, `Move`, and `Setup` tracked actions in this phase. A
  grouped action receives one ID and retains its ordered subject list; do not split or coalesce it
  into inferred per-unit workflows.
- Use only `Issued`, `ObservedActive`, `Satisfied`, `Unknown`, and `TimedOut`. `Issued` means only
  that the runtime emitted the corresponding ordinary command; it does not mean accepted, paid,
  queued, executing, or valid.
- The internal legacy bridge may allocate diagnostic IDs after existing command suppression, so
  suppressed legacy commands receive no record. These IDs are not fed back into Jeff and do not
  need to pretend that an untyped legacy `Move` was semantically a stage directive.
- Bound terminal history deterministically by pruning oldest terminal records only; do not prune
  active records merely to meet the cap.

## Observational Semantics

- Reconcile tasks only from the fog-safe `AiFrame` and controller state already delivered through
  the canonical runtime. Do not add an event handoff or change the live/offline host contract.
- Build becomes active/satisfied only from matching owner-visible worker/building evidence.
- Gather is satisfied when the worker is observed gathering and latched to the requested node.
- Move uses matching own order/state and the existing deterministic arrival tolerance; setup uses
  the subject's own setup order/posture. Ambiguous evidence remains `ObservedActive` or `Unknown`.
- A missing owned subject before a postcondition becomes `Unknown`, not failed. Timeouts use
  simulation ticks, never wall time, and do not influence Jeff in this phase.
- At most one strongest transition is emitted per task per frame; direct `Issued -> Satisfied` is
  allowed when the postcondition is already visible.

## Compatibility Migration

- Move these responsibilities under one explicitly named private compatibility component owned by
  the runtime, distinct from the generic public task ledger:
  - `PendingBuildTracker` and its failed-site cache;
  - staged and held unit sets;
  - active-attack suppression state.
- Preserve the exact legacy update points and behavior:
  - pending builds are observed only when the old controller observed them, not on every sim tick;
  - four-pixel progress threshold and 300-tick staleness;
  - identical worker-state/absence/footprint success rules;
  - identical 16-entry failed-site clear behavior and per-kind successful clear;
  - identical command/unit order, one-shot Hold Position, attack-before-stage state updates,
    suppression horizon, ownership pruning, and same-think move/setup handling.
- Let any legacy-derived task diagnostics run in shadow. Public task status must not drive placement
  retries, suppression, or any Jeff decision.
- Remove duplicated controller/script bookkeeping only after characterization tests and the complete
  Jeff transcript pass.

## Expected Touch Points

- New `server/crates/ai/src/sdk/task.rs` plus focused task-ledger and private compatibility modules.
- The canonical controller/runtime and Phase 3 `AiActions` surface.
- `server/crates/ai/src/live.rs`.
- `server/crates/ai/src/selfplay/pending_build.rs` and `selfplay/scripts.rs` for migration/deletion.
- `server/crates/ai/src/ai_core/decision/mod.rs`, observation, and facts for bridge metadata only.
- Focused ledger/compatibility tests.
- `docs/design/ai.md`.

Do not touch `rts-sim`, the wire protocol, client, lobby, balance, or `Game` API.

## Implementation Checklist

- [ ] Define deterministic task IDs, the four tracked action families, cautious statuses, and
      bounded history.
- [ ] Return IDs directly from typed submission and retain grouped subject order.
- [ ] Add own-state observational reconciliation for the supported task families.
- [ ] Move pending-build and combat-stage state into one compatibility owner.
- [ ] Characterize and preserve every legacy timeout/filter/cache quirk.
- [ ] Keep generic task status out of Jeff decisions.
- [ ] Remove obsolete duplicate state only after exact parity.
- [ ] Document what task statuses do and do not prove.
- [ ] Mark this phase done in the implementation commit.

## Verification

- Test stable ID allocation/return, grouped subject order, update ordering, bounded pruning,
  unknown/timeout cases, and every supported task family.
- Prove that no hidden state or recipient event is needed to obtain the same updates.
- Port or expand exact compatibility tests for moving/stuck builders, progress epsilon, expiry,
  footprint success, failed-site cap/clear, stage/hold filtering, attack suppression, ownership
  pruning, and queued Anti-Tank Gun setup.
- Run the generic ledger in shadow first and the exact Phase 1 normal/full transcript throughout;
  any command or post-tick difference blocks completion.
- Run focused `rts-ai` nextest, simulation archcheck, docs health, and diff check.

## Manual Test Focus

Inspect a fixed-seed Jeff replay around a long-distance build, the home Machine Gunner screen, the
first Tank/Scout Car launch, regrouping/stage suppression, and a worker retreat. Separately run a
small custom strategy through each tracked action and confirm returned IDs remain correlated and
diagnostics say “observed active,” “unknown,” or “timed out,” never “accepted” or “rejected.”

## Non-Goals

- No train, research, attack, stage, supersession, recipient-event, or causal death tracking.
- No authoritative receipts, rejection reasons, sim/protocol changes, or guaranteed causality.
- No task status feeding Jeff, idempotent `ensure_*` layer, goal scheduler, behavior tree, or GOAP.
- No rulebook, placement, pathing, or combat-range queries.
- No checkpoint persistence or external plugin ABI.

## Handoff Expectations

Report the task/status schema, typed submission-to-ID path, grouped-action and pruning rules, exact
compatibility owner and methods, removed duplicate fields, frame fields used as evidence, parity
fixture identity, and core manual checks. Give Phase 5 a list of remaining duplicated rule,
footprint, coordinate, arrival-tolerance, production-capability, and setup-posture lookups, and keep
the deferred task families in the final backlog.
