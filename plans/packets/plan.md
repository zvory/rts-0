# Snapshot Packet Budget Plan

## Purpose

Keep most player-visible snapshot payloads under a practical single-segment budget where that is
compatible with correctness and development velocity. The current transport is compact JSON text over
WebSocket: it is optimized compared with object-shaped JSON, but it is not compressed and it still
resends full per-player visible state. This plan first makes packet-budget pressure observable, then
runs a controlled encoding/compression bake-off before any larger delta protocol work.

## Overall Constraints

- Preserve server-authoritative simulation, fog filtering, and the existing client semantic snapshot
  shape unless a phase explicitly changes that contract.
- Treat browser reports as advisory and untrusted. Packet-budget diagnostics are for logs, parser
  output, local harnesses, and decision-making, not gameplay authority.
- Use the existing WebSocket connection unless Phase 2 evidence shows a transport change is worth a
  separate plan. gRPC is not a goal for this plan; protobuf is evaluated as a binary snapshot encoding
  over the existing WebSocket.
- Keep JSON compact snapshots as the default compatibility path until a phase proves a replacement is
  smaller, fast enough to parse/decode, and safe across live, replay, lab, observer, and spectator
  paths.
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
phase should keep JSON as the default while producing reproducible size and CPU results from the
Matt/Alex replay, AI harness, and at least one current live/dev workload. It should end with a
documented recommendation to ship one encoding path, defer all of them, or split follow-up work.

### [Phase 3 - Delta Snapshot Design Pass](phase-3.md)

Tentative and review-gated. A separate AI should turn the delta idea into a real design before any
implementation starts, including baseline ownership, keyframes, coalesced latest-only snapshots,
replay/lab compatibility, and fog safety. This placeholder exists so the likely next direction is
visible, but it is not runner-ready yet.

### [Phase 4 - Resource And Fog Delta Prototype](phase-4.md)

Tentative and review-gated. This placeholder covers the lower-risk state that currently contributes
recurring bytes, especially resource remaining updates and visible-tile runs. It needs Phase 3's
design before implementation because the client must reconstruct correct full semantic state after
skipped, replaced, or keyframed snapshots.

### [Phase 5 - Entity Delta And Keyframe Protocol](phase-5.md)

Tentative and review-gated. This placeholder covers the full stateful snapshot delta protocol for
entities, ability objects, remembered buildings, events, upgrades, and net status. It is expected to
be the durable path to consistently smaller snapshots, but it is complex enough to require a separate
fleshing-out pass and explicit user approval.

## Phase Index

1. [Phase 1 - Packet Budget Measurement](phase-1.md)
2. [Phase 2 - Encoding And Compression Bake-off](phase-2.md)
3. [Phase 3 - Delta Snapshot Design Pass](phase-3.md)
4. [Phase 4 - Resource And Fog Delta Prototype](phase-4.md)
5. [Phase 5 - Entity Delta And Keyframe Protocol](phase-5.md)

## Non-Goals

- Do not implement gRPC, WebTransport, UDP, rollback networking, or a second browser connection.
- Do not change gameplay command semantics, prediction authority, reconciliation acknowledgements, or
  server-side validation.
- Do not make the client authoritative for fog, visibility, entity lifetimes, resource values, or
  combat events.
- Do not add hard CI gates on absolute network byte counts yet. Early packet-budget metrics are
  evidence for comparisons, not portable guarantees across maps and workloads.
- Do not upload raw snapshots, raw command logs, entity id streams, player names, browser traces, or
  packet captures from normal clients.
- Do not implement tentative delta phases until Phase 3 is rewritten into an approved, fully fleshed
  out plan.

## Implementation Process

Implement one phase at a time. Do not start a later phase from an assumed merge; use the PR wait gate
and confirm the phase head is reachable from `origin/main`. For unattended executor passes, use only
the fully specified phases until the tentative delta phases are rewritten and approved:

```bash
scripts/phase-runner.sh --plan packets phase-1 --pr --wait
scripts/phase-runner.sh --plan packets phase-1 phase-2 --pr --wait
```

Do not run:

```bash
scripts/phase-runner.sh --plan packets phase-3 --pr --wait
```

until Phase 3 has been replaced with a runner-ready design document and the user has explicitly
approved moving into delta work.
