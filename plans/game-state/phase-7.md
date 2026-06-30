# Phase 7 - Ownership Guardrails And Release Audit

Status: Not started.

## Scope

After Phases 1 through 6 have established the ownership registry, `DerivedState`, `GameState`, the
internal cold checkpoint path, and checkpoint coverage for movement, economy, visibility, combat,
and effects, add the guardrails and final audit needed before any public checkpoint, replay, or lab
migration plan starts. This phase is behavior-preserving: it should make drift harder and document
remaining readiness gaps, not expose a new product surface.

Add or tighten architecture checks and lint-like checks so new stateful simulation owners must be
classified in the ownership registry and stored under the explicit state tree:

- durable authoritative or compatibility state belongs under `GameState`;
- rebuildable cache, index, or search state belongs under `DerivedState`;
- room/session state, AI controller memory, replay cursor/keyframe runtime, lab timeline history,
  sockets, UI/session capabilities, and other non-authoritative runtime state may stay outside
  `Game`, but must be documented as room/session state rather than silently becoming a second
  simulation owner;
- test-only fixtures or helpers may stay outside the production state tree, but should be clearly
  test-only and excluded intentionally;
- service modules may own invariants and narrow mutation APIs, but must not hide long-lived mutable
  authoritative simulation state outside `GameState`.

Use the Phase 1 registry and the final Phase 6 checkpoint/comparator coverage as the source of
truth. If the audit finds a state owner that is not classified, first update the registry and decide
whether it belongs in `GameState`, `DerivedState`, room/session state, or test-only state. If that
decision requires real implementation movement or new checkpoint coverage beyond a small guardrail
test, record it as a blocker in the readiness report instead of expanding this phase.

Also update the authoritative documentation:

- `docs/design/server-sim.md` should describe the final `GameState`/`DerivedState` ownership tree,
  checkpoint policy, service ownership rule, tick-boundary rule, guardrail/checker expectations, and
  remaining readiness blockers.
- `docs/context/server-sim.md` should point future agents to the final sections they must read when
  touching `Game`, services, state owners, checkpoint internals, or architecture guardrails.

Perform a final release-style audit that the ownership/checkpoint work stayed internal and
behavior-preserving:

- public `Game` API signatures and the documented `docs/design/server-sim.md` seam did not change
  unless an earlier phase explicitly documented and reviewed the contract change;
- wire protocol mirrors, protocol DTOs, client protocol code, snapshot shape, compact snapshot
  behavior, and start payload shape did not change for checkpoint ownership work;
- replay artifacts, replay seek/keyframe behavior, replay selected-vision behavior, lab timeline
  behavior, lab scenario import/export, lab scenario id remap behavior, and lab god mode behavior
  did not migrate to the private checkpoint path;
- private checkpoint export/import remains internal and does not create a public Rust checkpoint API,
  route, command, JSON schema, persisted file format, or UI affordance.

Produce a checkpoint-readiness report in the implementation handoff and, if useful, in
`docs/design/server-sim.md`. The report must list remaining blockers before any of these follow-up
programs start:

- public checkpoint schema/API and versioning;
- replay keyframe or replay artifact migration to checkpoints;
- lab timeline, lab scenario, or lab capture migration to checkpoints.

Explicit non-goals:

- No new checkpoint DTO coverage scenarios unless the audit finds a small missing guardrail test
  needed to prove the checker or registry rule.
- No public checkpoint schema, JSON format, endpoint, command, UI surface, or public Rust checkpoint
  API.
- No replay keyframe replacement, replay artifact migration, lab timeline migration, lab scenario
  migration, or lab product behavior change.
- No gameplay, balance, combat, fog, economy, production, pathing, projection, AI, or unit-stat
  changes.
- No blanket rewrite that forces every service to become stateless. Services may still own
  invariants and focused APIs; the guardrail is against hidden long-lived authoritative state.
- No direct main bypass. This phase must use the normal owned-PR workflow and wait-for-merge policy
  when implemented.

## Expected Touch Points

- `server/crates/archcheck/src/lib.rs`, `server/crates/archcheck/src/main.rs`, and
  `server/crates/archcheck/baselines/sim-architecture.json`: extend the existing sim architecture
  checks if they are the right home for state-owner classification, top-level `Game` state shape,
  hidden stateful service owner detection, broad mutable getter prevention, or registry-anchor
  enforcement. Prefer precise failures and explicit allowlists over broad pattern bans.
- `scripts/check-crate-boundaries.mjs`: update only if the ownership guardrail belongs with
  cross-crate boundaries rather than the sim archcheck. Do not duplicate the same rule in both
  places without a clear reason.
- `docs/design/server-sim.md`: document the final ownership tree, registry/checkpoint policy,
  service ownership rule, tick-boundary expectations, guardrail/checker commands, and
  checkpoint-readiness blockers.
- `docs/context/server-sim.md`: refresh section pointers and guardrail notes if the design-doc
  structure changed.
- Focused tests for `rts-archcheck` or checker fixtures if the guardrail logic needs executable
  coverage.
- Existing checkpoint, projection privacy, replay, and lab tests should be read-only verification
  evidence unless the audit finds a very small missing guardrail test.
- `server/crates/sim/src/game/**`, `server/src/lobby/**`, `server/crates/protocol/**`,
  `server/src/protocol.rs`, `client/src/protocol.js`, replay artifacts, and lab scenario schemas are
  audit evidence for this phase, not expected implementation targets.
- `plans/game-state/phase-7.md`: mark complete only in the implementation commit that lands this
  phase.

Implementation Rust/JS for gameplay, protocol, client, replay, lab, AI, balance, or room behavior
should be treated as out of scope. If the audit proves one of those areas must change before public
checkpoint work, record it as a blocker for the next plan instead of repairing it inside Phase 7.

## Verification

- The guardrail must fail when a production stateful simulation owner is added without a registry
  classification or without living under `GameState`/`DerivedState` or an explicitly documented
  room/session/test-only exception. Prefer a focused archcheck fixture or unit test over relying on
  manual review alone.
- Confirm the final ownership registry covers every durable state owner under `GameState`, every
  rebuildable owner under `DerivedState`, and every explicit exception for room/session/test-only
  state. No state owner should be silently omitted.
- Confirm the guardrail does not ban stateless services, pure policy modules, query/index services,
  or service-owned invariant helpers that store their durable state in `GameState`.
- Confirm public `Game` API signatures still match `docs/design/server-sim.md` and that room/lobby
  callers still go through the documented seam.
- Confirm wire protocol and client protocol mirrors did not change for this phase. If any protocol
  file changed unexpectedly, stop and re-scope before proceeding.
- Confirm replay artifact capture/playback, replay keyframe seek, lab timeline seek, lab scenario
  import/export, lab scenario id remap behavior, lab god mode, and selected/full-world projection
  behavior are not routed through public or private checkpoint APIs.
- Confirm the readiness report lists every blocker before public checkpoint schema/API, replay
  migration, or lab migration. At minimum, it should address schema/versioning, migration policy,
  compatibility with existing replay/lab artifacts, remaining comparator/coverage gaps, projection
  privacy risk, operational rollout, and any intentional room/session/test-only exclusions.

Suggested focused commands:

```bash
cargo fmt --manifest-path server/Cargo.toml
cargo test --manifest-path server/Cargo.toml -p rts-archcheck
cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture
node scripts/check-crate-boundaries.mjs
node scripts/check-docs-health.mjs
git diff --check -- server/crates/archcheck scripts/check-crate-boundaries.mjs docs/design/server-sim.md docs/context/server-sim.md plans/game-state/phase-7.md plans/game-state/plan.md
```

Also run the narrowest existing checkpoint/projection tests that prove the Phase 6 internal restore
coverage still works after guardrail changes. Suggested commands, if these filters exist after
Phase 6:

```bash
cargo test --manifest-path server/Cargo.toml -p rts-sim checkpoint
cargo test --manifest-path server/Cargo.toml -p rts-sim visibility_combat_checkpoint
cargo test --manifest-path server/Cargo.toml -p rts-sim projection_privacy
```

If the final test names differ, use the narrowest equivalent filters from the Phase 6 handoff. Run
`node scripts/check-wiki.mjs` as well if the docs changes affect wiki routing, doc-map entries, or
rendered-doc assumptions. No broad Node suite or full local test bundle is expected unless the
implementation escapes architecture/docs/checker code or changes a public contract; the PR
`./tests/run-all.sh` gate remains the authoritative full-suite check.

## Manual Testing Focus

No broad gameplay manual pass is expected for this guardrail/audit phase. Manual review should focus
on the ownership registry, guardrail failure messages, server-sim/context docs, and
checkpoint-readiness report.

If the audit touches any public-facing behavior unexpectedly, stop and re-scope. If a small
confidence smoke is still useful after purely internal guardrail changes, run one local replay seek
and one lab scenario restore only to confirm no new public checkpoint route, command, UI option, or
schema has appeared.

## Handoff

The implementation handoff must name:

- every guardrail added or tightened, including which script/crate owns it and what failure it
  produces for an unclassified state owner;
- how new durable state owners are required to live under `GameState`, how rebuildable owners are
  required to live under `DerivedState`, and how room/session/test-only exceptions are documented;
- the final docs sections updated in `docs/design/server-sim.md` and `docs/context/server-sim.md`;
- the final audit result for public `Game` APIs, wire protocol/client mirrors, replay behavior, lab
  behavior, projection privacy, and private checkpoint exposure;
- the checkpoint-readiness report, including blockers before public checkpoint schema/API, replay
  migration, and lab migration;
- any small guardrail test added, or confirmation that no new checkpoint DTO coverage scenario was
  added;
- the exact archcheck, crate-boundary, docs-health/docs-checker, focused checkpoint/projection, and
  `git diff --check` commands that passed;
- any residual risk that should be carried into the next public checkpoint/replay/lab migration plan.
