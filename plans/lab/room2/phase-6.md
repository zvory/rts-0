# Phase 6 - Persistence, Export, Docs, And Guardrails

## Phase Status

- [ ] Pending.

## Objective

Close the room capability model by finishing the persistence/export axis, updating source-of-truth
docs, removing obsolete names, and adding precise guardrails for the stable boundaries created by
earlier phases.

## Work

- Make persistence/export policy explicit enough to cover match-history eligibility, replay artifact
  capture, replay branch suppression, dev scenario suppression, lab scenario import/export, lab
  operation logging, and rooms that write nothing.
- Replace remaining generic-room decisions that infer persistence/export behavior from replay/dev/lab
  mode names where the policy can express the behavior.
- Update `plans/lab/room2/capability-matrix.md` to match the final code.
- Update `plans/lab/architecture.md` only where the landed capability model confirms or refines its
  hypotheses. Do not reopen lab MVP scope.
- Update `docs/design/server-sim.md`, `docs/design/protocol.md`, `docs/design/client-ui.md`, and
  relevant context capsules so they describe the final capability names and the product-mode
  references that intentionally remain at setup/routing edges.
- Add or tighten precise guardrails. Good candidates are:
  - generic room tick/projection/launch/client-affordance code must not derive shared behavior from
    replay/dev/lab names when capability metadata applies;
  - snapshot fanout stays routed through projection policy;
  - diagnostics are requested through snapshot/projection options, not durable product-mode flags;
  - lab mutation remains centralized in room-task request handling and public `Game` lab APIs;
  - protocol and JavaScript mirrors stay synchronized after renamed room-time and capability fields.
- Remove obsolete compatibility names, comments, tests, and docs references introduced by earlier
  replay/dev/debug contracts.

## Expected Touch Points

- `server/src/lobby/session_policy.rs`
- `server/src/lobby/room_task.rs`
- `server/src/lobby/launch.rs`
- `server/src/lobby/projection.rs`
- `server/src/lobby/live_tick.rs`
- `server/src/lobby/replay_session.rs`
- `client/src/*`
- `docs/design/server-sim.md`
- `docs/design/protocol.md`
- `docs/design/client-ui.md`
- `docs/context/server-sim.md`
- `docs/context/protocol.md`
- `docs/context/client-ui.md`
- `plans/lab/architecture.md`
- `plans/lab/room2/capability-matrix.md`
- `scripts/check-lobby-architecture.mjs`
- `scripts/check-client-architecture.mjs`
- `tests/select-suites.mjs` if suite selection should learn the new guardrail
- `plans/lab/room2/phase-6.md`

## Implementation Checklist

- [ ] Finish persistence/export capability coverage and tests.
- [ ] Remove remaining obsolete replay/dev/debug compatibility names from generic room and client
      surfaces.
- [ ] Update design docs, context capsules, and the capability matrix.
- [ ] Add precise lobby/client/protocol guardrails for stable boundaries.
- [ ] Run focused server, protocol, client, and architecture checks.
- [ ] Mark this phase as done in this file.

## Verification

- `cargo test --manifest-path server/Cargo.toml -p rts-server session_policy`
- `cargo test --manifest-path server/Cargo.toml -p rts-server lab`
- `cargo test --manifest-path server/Cargo.toml -p rts-server replay`
- `cargo test --manifest-path server/Cargo.toml -p rts-server branch`
- `cargo test --manifest-path server/Cargo.toml -p rts-server dev`
- `node tests/protocol_parity.mjs`
- `node tests/client_contracts.mjs`
- `node scripts/check-lobby-architecture.mjs`
- `node scripts/check-client-architecture.mjs`
- `cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture`
- `node tests/select-suites.mjs --verify`
- `git diff --check`

## Manual Test Focus

Smoke the final capability model through one normal match, one spectator view, one replay with
vision and time controls, one replay branch staging/live launch, one dev scenario pause/step flow,
and one lab import/export plus operator setup flow.

## Handoff Expectations

Summarize the final room capability boundaries, the guardrails added, the compatibility names removed,
and any remaining product-mode references with their justification. State whether the plan is fully
closed or whether a new product plan should handle lab timeline controls or future room features.
