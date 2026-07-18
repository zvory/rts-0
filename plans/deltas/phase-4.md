# Phase 4 - Recovery, Viewing Modes, and Default Rollout

## Phase Status

- [ ] Ready after Phase 3 is merged with its byte and correctness gates satisfied.

## Objective

Turn the measured active-player delta path into the normal live default, harden bounded recovery,
and decide explicitly which spectator/replay/branch/Lab/dev views can use the same stream without
making them a prerequisite for the player-facing win. Retain complete MessagePack keyframes as the
recovery and rollback path.

## Entry Gate

- Phase 3 is merged and reachable from `origin/main`.
- Both named workloads meet the accepted 1,280-byte P95 contract, including any user-approved
  supply-scale decision.
- Delta reconstruction, explicit removal/clear, skipped-tick coalescing, event policy, fog/privacy,
  and periodic/forced keyframe tests pass.
- Server diff/serialize and client decode/reconstruct/apply costs show no material tick/frame
  regression.
- The handoff identifies the final compact version, keyframe cadence, reset reasons, size fallback,
  and any mode still intentionally keyframe-only.

## Recovery Rules

- The client accepts a delta only when its version and `baseSequence` match its fully validated
  baseline. On mismatch or malformed data, do not mutate semantic state and ignore deltas until the
  next independent keyframe.
- Keep the 60-successful-send periodic keyframe and all lifecycle/projection forced keyframes from
  Phase 1. Recovery on this ordered reliable stream does not require client acknowledgements or a
  retransmission buffer.
- Add a bounded mismatch/resync counter and keyframe-recovery timing diagnostic. Add a rate-limited
  `requestSnapshotKeyframe` message only if canary evidence shows waiting for the periodic keyframe
  creates a material player-visible freeze; do not add it speculatively.
- A full keyframe remains the immediate fallback when the delta is unsafe or not smaller.

## Viewing-Mode Policy

Use one central server-owned allowlist rather than letting room modes opt themselves into deltas:

| Mode | Phase 4 policy |
| --- | --- |
| Normal live active player | Delta-eligible after the canary gate. |
| Occupied replay-branch seat after live launch | Delta-eligible after a forced launch keyframe. |
| Live spectator, including branch spectators | Keyframe-only. |
| Replay playback | Keyframe-only. |
| Lab operator/viewer | Keyframe-only. |
| Dev-watch/dev scenario | Keyframe-only. |
| Checked-in snapshot streams | Keyframe-only fixture compatibility. |

- Reuse the same post-projection encoder and reconstructor; do not create mode-specific delta
  protocols.
- Force a keyframe before the first snapshot after every role, seek, vision, seat, or projection
  transition, even for modes that remain keyframe-only. Clear pending snapshots and advance the
  reset generation through one shared helper.
- Do not broaden this allowlist merely for uniformity. A later small plan may enable deltas for a
  non-player mode only if its traffic becomes important and its nonlinear time/projection behavior
  has direct tests.

## Diagnostics and Rollout

- Add or finalize bounded fields for active snapshot mode, keyframe/delta counts, keyframe/reset
  reasons, size fallbacks, baseline mismatches, recovered keyframes, and reconstruction P95. Extend
  existing incident parsing only where it makes these decisions visible.
- Run local full-keyframe and delta canaries on identical release workloads, then one bounded beta
  active-player canary long enough to collect client/server reports and normal gameplay evidence.
- Compare payload P95/over-budget rate, snapshot gaps/bursts, slot replacement/send age, command
  acknowledgement health, server tick/diff/serialize time, and client decode/reconstruct/apply/frame
  time.
- Add one startup-fixed canary/rollback switch such as `RTS_SNAPSHOT_DELTAS=0|1`; do not negotiate it
  with stale clients or hot-switch an active connection. Deploy beta with deltas explicitly enabled
  before changing the code default.
- Make active-player deltas the direct default only if the byte, correctness, and cost gates pass.
  Keep full keyframe encoding as the automatic per-frame fallback, the startup switch as an immediate
  operational rollback, and Git revert as the code rollback; do not turn the switch into a long-lived
  product mode.
- If beta exposes stale state, privacy leakage, recovery failure, or material CPU/frame regression,
  fix the bounded defect or leave delta defaulting blocked with the exact evidence. Do not paper over
  it by weakening cadence or state fidelity.
- Update protocol and performance documentation with the final default, mode matrix, recovery
  behavior, canary evidence, and rollback rule.

The beta canary must include at least two active fog-filtered recipients and at least 1,000 received
snapshot samples per measured recipient. Treat a server or client processing regression as material
when the relevant P95 worsens by both more than 10% and more than 0.25 ms on the same workload and
host; do not tune that threshold after seeing the result.

## Expected Touch Points

- Phase 1-3 stream encoder and client reconstructor modules
- `server/src/connection_writer.rs`
- `server/src/lobby/connection.rs`
- relevant reset/view seams under `server/src/lobby/room_task/`
- `server/src/structured_log.rs`
- `server/crates/protocol/src/lib.rs` only if recovery/report messages change
- `client/src/net.js`
- `client/src/client_perf_report.js`
- `scripts/parse-net-report-logs.mjs`
- `scripts/net-report-snapshot-payload.mjs`
- `docs/design/protocol.md`
- `docs/perf-tracing.md`
- focused live/spectator/replay/branch/Lab/dev writer and reconstructor tests

## Implementation Checklist

- [ ] Confirm the Phase 3 byte/correctness/cost gate.
- [ ] Finalize bounded baseline-mismatch and keyframe-recovery diagnostics.
- [ ] Enforce the central eligible-mode allowlist and keyframe every role/projection transition.
- [ ] Prove live active and occupied branch-seat deltas reconstruct exactly through their starts.
- [ ] Prove spectator, replay, Lab, dev, and fixture modes remain correct keyframe consumers.
- [ ] Run identical local full-keyframe and delta release canaries.
- [ ] Run a bounded beta active-player canary and inspect client/server reports.
- [ ] Default normal live active players to deltas only after every gate passes.
- [ ] Document final mode matrix, default, recovery, fallback, evidence, and rollback.
- [ ] Keep focused malformed/recovery/privacy tests even if some modes remain keyframe-only.
- [ ] Mark this phase done in the implementation commit.

## Verification

- `cargo test --manifest-path server/Cargo.toml -p rts-protocol`
- Focused `rts-server` writer plus live/spectator/replay/branch/Lab reset tests.
- `node tests/client_contracts.mjs`
- `node tests/protocol_parity.mjs`
- Focused client reconstructor mismatch/recovery and enabled-mode tests.
- `node tests/net_report_log_parser.mjs` if report/parser fields change.
- Repeat the two accepted Phase 3 release workloads in full-keyframe and delta modes.
- `node scripts/check-docs-health.mjs`
- `git diff --check`

## Manual Test Focus

Play a normal fog match through movement, combat, production, reconnection, and artificial writer
delay. Confirm selection, interpolation, prediction acknowledgements, fog exits, transient effects,
and periodic recovery remain normal. Exercise each non-player policy once: branch start for the one
delta-eligible live seat, plus spectator join, replay seek/vision switch, Lab reset/seek/vision, and
dev full-world start through their retained keyframe paths.

## Completion and Handoff Expectations

State the rollout decision first: default-on or blocked. Include the final per-mode policy, local and
beta comparison table, target P95/over-budget result, keyframe/fallback/resync counts, server/client
processing cost, exact recovery behavior, rollback rule, and remaining caveats. Name the core manual
tests completed and any keyframe-only mode intentionally deferred; when every phase is done, let
`scripts/agent-pr.sh` archive this plan in the final phase PR.
