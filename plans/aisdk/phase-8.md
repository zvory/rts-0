# Phase 8 - Clean Up and Ratchet the SDK

## Phase Status

- [ ] Ready for implementation after Phase 7 merges.

## Objective

Prove the delivered surfaces form a practical outside-authoring path, remove only compatibility code
made obsolete by the merged Jeff cutover, and install narrow architecture checks that keep policy
code on the SDK. This phase starts with an independent parity rerun and retains every low-level
exception that still has a legitimate host, replay, scorecard, or synthetic-harness responsibility.

## Entry Criteria

- Phase 7 is merged and its head is reachable from `origin/main`.
- The Phase 1 fixture is byte-for-byte unchanged and passes normal/full reruns from that merged head.
- Jeff and the shared profile engine already consume the public SDK surfaces; this phase does not
  complete or repair the cutover.

## Work

- Add an outside-crate `sdk_conformance` strategy that uses the supported public runtime to:
  - read an `AiFrame`;
  - ask an `AiRulebook` question and make a `WorldQueries` call;
  - submit and inspect one tracked task;
  - form a `UnitGroup` and submit actions through `AiActionPlanner`;
  - reach ordinary simulation validation without private imports or raw protocol/command types.
- Delete or collapse only one-for-one obsolete compatibility implementations proven unused after
  the Phase 7 cutover, such as old raw observation projection, command-emitting contexts, duplicate
  placement helpers, profile-backed runtime remnants, and superseded pending/stage copies.
- Retain compatibility projections still required to reproduce Jeff's historical policy inputs.
  Retain synthetic scripts, replay decoding, scorecard inspection, and other infrastructure that
  legitimately reads or matches low-level commands.
- Add `scripts/check-ai-sdk-boundaries.mjs` with narrow file-role allowlists. Ratchet production
  strategy/profile/decision code against direct `StartPayload`, `Snapshot`, `EntityView`, protocol
  string parsing, `rts_sim::protocol` imports, `SimCommand` construction, or new snapshot-derived
  placement helpers.
- Allow raw protocol and command work only in documented adapter/runtime (`AiController`/`live.rs`),
  planner-emitter, replay, scorecard, host, and synthetic-test roles. Public `UncommonAction`
  remains SDK-owned and does not exempt strategies from the raw-command rule.
- Wire the checker into the normal policy lane in `tests/run-all.sh` and update
  `tests/select-suites.mjs` so SDK and checker changes select it.
- Update `docs/design/ai.md` with the final public lifecycle, ownership, fog/determinism guarantees,
  custom-strategy path, task-versus-command-authority distinction, low-level allowlist, and deferred
  gold-architecture backlog. Refresh context pointers only if document structure changes.

## Expected Touch Points

- `server/crates/ai/src/lib.rs`, completed `sdk/**`, and obsolete compatibility modules identified
  by Phase 7.
- `server/crates/ai/src/ai_core/**`, `ai_shared.rs`, and `selfplay/**` only where a proven duplicate
  is removed or a legitimate low-level exception is documented.
- New `server/crates/ai/tests/sdk_conformance.rs`.
- New `scripts/check-ai-sdk-boundaries.mjs`.
- `tests/run-all.sh`, `tests/select-suites.mjs`, and their focused tests.
- `docs/design/ai.md` and capsule pointers only if needed.

No gameplay policy, balance, protocol, client, or simulation-command-processing change belongs in
this phase.

## Implementation Checklist

- [ ] Independently rerun Phase 1 normal/full parity before cleanup.
- [ ] Add an actual outside-crate conformance strategy using every first-80% surface.
- [ ] Delete only proven-unused one-for-one compatibility implementations.
- [ ] Keep legitimate low-level host/harness/replay/scorecard exceptions.
- [ ] Add and wire the architecture checker with narrow role allowlists.
- [ ] Verify public strategies never import or construct `SimCommand`.
- [ ] Document final ownership, supported entry points, exceptions, and deferred work.
- [ ] Pass the unchanged transcript after each deletion cluster and before delivery.
- [ ] Mark this phase done in the implementation commit.

## Verification

```bash
node scripts/check-ai-sdk-boundaries.mjs
cargo test --manifest-path server/Cargo.toml -p rts-ai --test sdk_conformance
cargo nextest run --config-file .config/nextest.toml \
  --manifest-path server/Cargo.toml --profile default -p rts-ai
RTS_FULL_AI_TESTS=1 cargo nextest run --config-file .config/nextest.toml \
  --manifest-path server/Cargo.toml --profile default -p rts-ai
cargo clippy --manifest-path server/Cargo.toml -p rts-ai --all-targets
```

- Run the exact Phase 1 transcript after each deletion cluster and verify the fixture checksum is
  unchanged.
- Run crate/sim architecture checks, suite-selection tests, docs health, and diff check.
- The GitHub `Main test gate` remains authoritative.

## Manual Test Focus

Read the conformance strategy as an external AI author and confirm it uses only public semantic
types. Run one deterministic Jeff matchup and inspect representative build, production, gathering,
staging, setup, attack, and retreat entries; the transcript, not visual similarity, is the parity
authority.

## Non-Goals

- No policy improvement, new task family, balance change, or transcript regeneration.
- No conversion of replay/scorecard/synthetic harness code with legitimate raw duties.
- No behavior tree, GOAP, goal scheduler, authoritative command-result protocol, general tactical
  solver, stable plugin ABI, or non-Rust interface.
- No broad aesthetic rewrite or deletion justified only by textual duplication.

## Completion and Handoff Expectations

Report the merged head, unchanged fixture path/checksum, exact transcript/conformance/checker/full-AI
commands, public strategy entry points, deleted compatibility implementations, retained low-level
exceptions with owners, and manual replay artifact. The final reviewer must rerun the transcript and
boundary checker from current `origin/main`, inspect the conformance test as an actual outside-crate
consumer, verify raw access is confined to documented boundaries, and audit the completed plan for
archival.
