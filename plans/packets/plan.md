# Snapshot Packet Budget Plan

## Purpose

Keep most player-visible snapshot payloads under a practical single-segment budget where that is
compatible with correctness and development velocity. The current transport started as compact JSON
text over WebSocket: it is optimized compared with object-shaped JSON, but it still resends full
per-player visible state. This plan first makes packet-budget pressure observable, then ships a
MessagePack binary full-snapshot baseline before any larger delta protocol work.

## Overall Constraints

- Preserve server-authoritative simulation, fog filtering, and the existing client semantic snapshot
  shape unless a phase explicitly changes that contract.
- Treat browser reports as advisory and untrusted. Packet-budget diagnostics are for logs, parser
  output, local harnesses, and decision-making, not gameplay authority.
- Use the existing WebSocket connection. gRPC, WebTransport, a second browser connection, and
  WebSocket-stack swaps are not goals for this plan.
- Prefer a direct MessagePack binary snapshot rollout over backwards-compatible negotiation. This is
  a pre-alpha game: stale clients may fail or require refresh, and rollback is a Git revert rather
  than a long-running compact JSON fallback mode.
- Delta work must happen after per-recipient fog projection. Do not diff from global simulation state
  or retain a baseline that contains data the recipient was not allowed to see.
- Delta baselines are per connection/recipient and are updated only after the writer successfully
  sends a frame. The latest-only pending snapshot slot should continue to hold a full semantic
  snapshot; it must not hold a chain of unsent deltas.
- Any stateful delta mode must force keyframes on match start, reconnect, unsupported version,
  compact/schema version change, replay seek, lab time/vision reset, projection-policy change, and a
  documented periodic cadence.
- The client-side delta reconstructor must return the same semantic snapshot shape consumed by
  `GameState.applySnapshot`; renderer, HUD, minimap, and input code should not learn about transport
  deltas.
- Use a conservative "single-segment budget" constant for payload bytes. The exact value must be
  chosen and documented in Phase 1, but it should account for WebSocket/TLS/TCP/IP overhead because
  existing snapshot byte logs count only payload bytes.
- Do not hide large snapshots by lowering fidelity, leaking fog data, dropping required events, or
  weakening command acknowledgement semantics.
- Keep all normal-match reporting bounded. Prefer p95, max, counts, percentages, and byte totals over
  raw per-snapshot arrays.
- Coordinate any protocol field or snapshot wire change with `server/crates/protocol/src/lib.rs`,
  `client/src/protocol.js`, `docs/design/protocol.md`, `docs/perf-tracing.md`, focused tests, and the
  incident parser where applicable.
- Transient events are not durable baseline state. If a delta phase changes event handling, it must
  state whether it preserves the current latest-only semantics or deliberately adds bounded event
  accumulation with fog-safe tests.
- Each phase must be implemented on its own `zvorygin/` branch, pushed as an owned PR with auto-merge
  armed, then waited on until GitHub reports the PR merged and the phase head is reachable from
  `origin/main`.
- After each phase, the implementing agent must provide a handoff message naming exact verification,
  behavior affected, remaining risks, next-phase guidance, and the core features that should be
  manually tested.
- When a phase is complete, mark that phase document as done in the implementation commit for that
  phase.

## Phase Summaries

### [Phase 1 - Packet Budget Measurement](phase-1.md)

Add first-class packet-budget metrics to the existing client network report, server structured log,
local harness, and incident parser. This phase should answer, for each player report window, whether
snapshot p95 and over-budget rate fit a documented payload-byte budget instead of only reporting
totals/max/average or the current very-large payload pressure thresholds. It should not change the
snapshot format or transport.

### [Phase 2 - Encoding And Compression Bake-off](phase-2.md)

Build an experiment path that compares compact JSON baseline, WebSocket compression when supported,
protobuf-style schema binary, MessagePack, CBOR, and a narrow custom binary snapshot encoding. This
phase keeps compact JSON as the historical baseline while producing reproducible size and CPU results
from the Matt/Alex replay, AI harness, and at least one current live/dev workload. Its original
deflate follow-up recommendation is superseded by the later product decision to ship MessagePack.

### [Phase 2.5 - MessagePack Snapshot Frames](phase-2.5.md)

Replace live snapshot text frames with MessagePack binary frames over the existing WebSocket. This
phase should make MessagePack the normal snapshot path after local validation, keep reliable
non-snapshot messages as JSON text, and avoid stale-client negotiation or compact JSON compatibility
fallback. It should produce local before/after payload and timing evidence and leave rollback as a
plain revert if the new path is not shippable.

### [Phase 2.6 - MessagePack Beta Smoke And Delta Gate](phase-2.6.md)

Do the short operational closeout for the MessagePack snapshot rollout before any delta work begins.
This phase should verify the default MessagePack path on beta once or twice when practical, decide
whether to keep or revert it, clean up stale compression wording, and state whether Phase 3 delta
work is recommended next. It should not become a long-running migration, opt-in rollout, or fallback
compatibility project.

### [Phase 3 - Delta Snapshot Envelope And Baseline Scaffold](phase-3.md)

Add the stateful snapshot frame envelope, per-writer baseline tracking, forced-keyframe rules, and
client reconstruction seam without trying to shrink payloads yet. This phase should prove that a
keyframe-only run through the new path behaves like current MessagePack full snapshots and that
baselines are updated only for frames actually sent. It stays gated on Phase 2.6's MessagePack
keep/revert decision and explicit user approval before delta work begins.

### [Phase 4 - Resource And Fog Delta Prototype](phase-4.md)

Implement the first real deltas for resource remaining updates and row-major `visibleTiles`, while
entities and other sections remain full/keyframed. This phase should use the Phase 3 baseline model,
fall back to a keyframe when a patch is larger or unsafe, and compare payload/reconstruction cost
against Phase 1/2 baselines. Its goal is to prove the full keyframe/reconstruct/recover loop on
lower-risk snapshot sections before entity state becomes stateful.

### [Phase 5 - Entity Record Delta Protocol](phase-5.md)

Add record-level entity add/update/remove deltas after fog projection, using full compact entity
records for changed entities rather than field-level patches. This phase should prove that entities
leaving visibility are removed from the client baseline, hidden target/tracer data is not retained,
and coalesced latest-only sends diff against the last sent frame, not skipped ticks. It keeps the
existing keyframe fallback and should not attempt auxiliary section deltas in the same PR.

### [Phase 6 - Auxiliary Section Deltas And Recovery](phase-6.md)

Extend the delta policy to smokes, ability objects, remembered buildings, optional spectator/replay
resource sections, upgrades, and recovery diagnostics. This phase should keep `events` and
`netStatus` deliberately full or explicitly document any bounded event accumulation change, then
harden stale-baseline, malformed-frame, replay/lab seek, vision-mode, and keyframe-request recovery.
It is where the implementation stops being a narrow prototype and proves compatibility across live,
spectator, replay, branch, lab, and dev-watch paths.

### [Phase 7 - Defaulting, Rollout, And Cleanup](phase-7.md)

Use Phase 1 through Phase 6 measurements to decide whether delta snapshots should become the default,
remain opt-in, or be reverted/deferred. This phase should update perf tooling and incident parsing
with delta/keyframe ratios, resync counts, and keyframe reasons, then choose a conservative rollout
flag and rollback path. MessagePack full snapshots are the default keyframe/full-snapshot baseline
unless Phase 2.6 reverted them.

## Phase Index

1. [Phase 1 - Packet Budget Measurement](phase-1.md)
2. [Phase 2 - Encoding And Compression Bake-off](phase-2.md)
2.5. [Phase 2.5 - MessagePack Snapshot Frames](phase-2.5.md)
2.6. [Phase 2.6 - MessagePack Beta Smoke And Delta Gate](phase-2.6.md)
3. [Phase 3 - Delta Snapshot Envelope And Baseline Scaffold](phase-3.md)
4. [Phase 4 - Resource And Fog Delta Prototype](phase-4.md)
5. [Phase 5 - Entity Record Delta Protocol](phase-5.md)
6. [Phase 6 - Auxiliary Section Deltas And Recovery](phase-6.md)
7. [Phase 7 - Defaulting, Rollout, And Cleanup](phase-7.md)

## Non-Goals

- Do not implement gRPC, WebTransport, UDP, rollback networking, or a second browser connection.
- Do not spend packet-plan budget on stale-client compatibility, compact JSON fallback migration, or
  `permessage-deflate` WebSocket-stack work unless the user explicitly reverses the MessagePack
  direction.
- Do not change gameplay command semantics, prediction authority, reconciliation acknowledgements, or
  server-side validation.
- Do not make the client authoritative for fog, visibility, entity lifetimes, resource values, or
  combat events.
- Do not add hard CI gates on absolute network byte counts yet. Early packet-budget metrics are
  evidence for comparisons, not portable guarantees across maps and workloads.
- Do not upload raw snapshots, raw command logs, entity id streams, player names, browser traces, or
  packet captures from normal clients.
- Do not implement delta phases until Phase 2.6 has merged, the MessagePack keep/revert decision
  recommends moving beyond encoding, and the user explicitly approves delta work.

## Implementation Process

Implement one phase at a time. Do not start a later phase from an assumed merge; use the PR wait gate
and confirm the phase head is reachable from `origin/main`. Phase 1, Phase 2, Phase 2.5, and Phase
2.6 can run normally:

```bash
scripts/phase-runner.sh --plan packets phase-1 --pr --wait
scripts/phase-runner.sh --plan packets phase-2 --pr --wait
scripts/phase-runner.sh --plan packets phase-2.5 --pr --wait
scripts/phase-runner.sh --plan packets phase-2.6 --pr --wait
```

Do not run delta phases until Phase 2.6 has kept or reverted MessagePack, recommends delta work as
the next step, and the user explicitly approves that direction. After that gate, run them one at a
time:

```bash
scripts/phase-runner.sh --plan packets phase-3 --pr --wait
scripts/phase-runner.sh --plan packets phase-4 --pr --wait
scripts/phase-runner.sh --plan packets phase-5 --pr --wait
scripts/phase-runner.sh --plan packets phase-6 --pr --wait
scripts/phase-runner.sh --plan packets phase-7 --pr --wait
```
