# Live Snapshot Delta Plan

## Purpose

Replace repeated full live snapshots with a small state-update stream while preserving the complete
fog-filtered semantic `Snapshot` seen by the rest of the client. The practical target is snapshot
application-payload P95 at or below 1,280 bytes on named representative release workloads; TCP/TLS
may still split or combine writes, and rare keyframes or mass-change frames may exceed that budget.
This is a narrow WebSocket/MessagePack evolution, not a transport rewrite, compatibility migration,
or client-authoritative simulation project.

## Overall Constraints

- Keep the existing ordered reliable WebSocket and MessagePack frame path. Do not add WebTransport,
  acknowledgements, retransmission buffers, compression, or a second connection without later
  evidence that the simple design cannot meet the target.
- The room task and latest-only pending slot continue to hold complete semantic snapshots. Never
  enqueue a chain of deltas or make the room task wait for encoding or socket progress.
- Diff only a recipient's already projected, fog-filtered snapshot. A baseline must never contain
  global state or information that recipient was not permitted to see.
- The connection writer owns the per-connection sequence and last successfully sent baseline. A
  pending replacement is compared with that sent baseline, not with the previous simulation tick or
  an overwritten pending snapshot.
- Patches carry absolute new values, not arithmetic increments. Skipped ticks must be equivalent to
  applying one patch from the last sent state directly to the newest state.
- The client reconstructs and validates a complete semantic snapshot below network dispatch before
  calling `GameState.applySnapshot`. Renderer, HUD, minimap, input, and prediction code must not
  learn transport-delta semantics.
- Full MessagePack keyframes remain the recovery and size-fallback path. Force them on lifecycle or
  projection resets and periodically; commit a keyframe or delta baseline only after its WebSocket
  send succeeds.
- Events and visible resource remaining updates are frame-local data, not durable baseline state.
  Do not accidentally replay old effects from a keyframe or silently change their current delivery
  semantics without explicit tests and documentation.
- Preserve 30 Hz state and presentation cadence, exact fog authority, command acknowledgement,
  current simulation values, and client-visible timing. Do not meet the byte target through stale
  presentation, reduced fidelity, quantization, viewport-only authority, or cross-frame staggering.
- Keep payload and timing diagnostics bounded. Absolute byte targets remain release-workload
  evidence, not a portable hard CI gate.
- This is a direct pre-alpha protocol rollout. Bump the compact version with each changed wire shape
  and update Rust, browser, tests, and `docs/design/protocol.md` together; do not maintain stale-client
  negotiation or a parallel legacy live mode.
- A fresh sub-agent must execute each phase through the repository's `phase-runner` workflow. Each
  phase lands on its own `zvorygin/` branch, is pushed as an owned PR with auto-merge armed, and waits
  until the PR is definitely merged and its head is reachable from `origin/main` before the next
  phase starts.
- When a phase is complete, mark its phase document done in that phase's implementation commit. The
  implementing sub-agent must provide a handoff describing what changed, what the next agent should
  do, and the core manual test focus.

## Measurement Contract

Phase 3 must produce comparable release-build evidence through the real writer and client
reconstructor for both:

1. A normal fog-filtered active-player AI/live workload.
2. A player-projected supply-scale workload derived from `supply-300-hellhole`.

The checked-in offline `.rtsstream` is useful for presentation regression testing but is not wire
payload evidence because it does not exercise per-connection writer baselines or live coalescing.
If existing harnesses bypass that path, Phase 3 must add the smallest measurement mode needed before
claiming the target. The 1,280-byte P95 target applies to both named lanes unless measured evidence
shows the supply-scale lane requires a materially more complex design; in that case stop with the
exact byte gap and request a user decision rather than quietly weakening the goal.

## Phase Summaries

### [Phase 1 - Delta Stream Foundation](phase-1.md)

Add the versioned keyframe/delta envelope, writer-owned sent baseline, reset generation, and client
reconstruction boundary while continuing to send only full keyframes. Prove that pending snapshot
replacement cannot advance the baseline and that start, reconnect, seek, and vision changes force an
independent state. Preserve simulation `f32` values in MessagePack without lossy quantization so the
later delta measurements start from the cheapest exact numeric representation.

### [Phase 2 - First Useful Live Deltas](phase-2.md)

Enable deltas for normal live active players using absolute hot-transform updates, complete entity
adds/replacements, explicit removals, and changed fog runs. Keep other sections deliberately simple,
fall back to a keyframe whenever a patch is unsafe or not smaller, and leave replay, Lab, spectator,
branch, and dev-watch connections on keyframes. The result must reconstruct exactly across skipped
ticks, deaths, fog exits, optional-field removal, and pending-slot replacement.

### [Phase 3 - Measure and Close the Byte Gap](phase-3.md)

Run the two named workloads through the actual writer/reconstructor path and use existing payload
composition diagnostics to identify what still controls P95. Apply only the first measured remedy
that is needed, such as absolute end ticks for a ticking countdown, one durable-section patch, or a
bounded entity field mask, rerunning the same evidence after each change. Stop as soon as both lanes
meet the target; if the remaining gap requires quantization, motion prediction, or another material
scope expansion, report that blocker for a user decision instead of inventing a generalized codec.

### [Phase 4 - Recovery, Viewing Modes, and Default Rollout](phase-4.md)

Harden sequence/reset recovery, decide explicitly which non-player viewing modes can safely use the
same delta path, and retain full keyframes for any mode whose reset semantics are not yet proven. Run
local and beta canaries with bounded mode/keyframe/fallback diagnostics and compare payload plus
server/client processing cost. Make live active-player deltas the default only when the Phase 3 byte
gate and this phase's correctness gates pass, with the existing full keyframe path as the rollback.

## Phase Index

1. [Phase 1 - Delta Stream Foundation](phase-1.md)
2. [Phase 2 - First Useful Live Deltas](phase-2.md)
3. [Phase 3 - Measure and Close the Byte Gap](phase-3.md)
4. [Phase 4 - Recovery, Viewing Modes, and Default Rollout](phase-4.md)

## Non-Goals

- Literal control over IP packet boundaries or a guarantee that every exceptional frame is one
  segment.
- Sending a historical update for every simulation tick after the server intentionally coalesces or
  skips work; the next sent update represents the newest authoritative state.
- UDP, WebTransport datagrams, rollback networking, lockstep, client authority, or a second event
  transport.
- Lossy position/angle quantization, lower snapshot cadence, viewport-dependent ownership state, or
  intentionally stale presentation.
- General-purpose schema negotiation, stale-client compatibility, or a long-lived full-vs-delta
  product setting.
- Deltas for every auxiliary section before measurements show that section matters.
- A hard CI failure on an absolute payload number across all machines, maps, and modes.

## Required Verification Themes

Each phase chooses the smallest relevant subset of:

- `cargo test --manifest-path server/Cargo.toml -p rts-protocol`
- Focused `rts-server` writer, connection, room, replay, and Lab tests.
- `node tests/client_contracts.mjs`
- `node tests/protocol_parity.mjs`
- Focused client frame-decoder and snapshot-reconstructor tests.
- Exact semantic reconstruction comparisons for keyframes, ordinary deltas, skipped ticks,
  removals, explicit clears, malformed frames, and resets.
- `node scripts/check-docs-health.mjs`
- `git diff --check`

## Implementation Process

After this plan is approved, run each phase with a fresh executor sub-agent and wait for its PR to
merge before starting the next:

```bash
scripts/phase-runner.sh --plan deltas phase-1 --pr --wait
scripts/phase-runner.sh --plan deltas phase-2 --pr --wait
scripts/phase-runner.sh --plan deltas phase-3 --pr --wait
scripts/phase-runner.sh --plan deltas phase-4 --pr --wait
```

Do not run the chain without `--wait`. Planning and final review remain manual, and a phase handoff
must carry forward its exact compact version, keyframe rules, measurements, unresolved risks, and
core manual tests.
