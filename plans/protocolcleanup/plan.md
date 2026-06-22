# Protocol Mirror Cleanup Plan

## Purpose

Reduce the review and merge risk around the protocol mirror without changing wire behavior. This
plan exists because `plans/hotspotcleanup/phase-9.md` found a safe cleanup path only if Rust
protocol code, browser protocol code, parity tests, and protocol design notes move as one mirrored
surface. Every phase is behavior-preserving: no gameplay, fog, balance, protocol tag, field, compact
code, compact slot, version, enum vocabulary, optional slot, or exported API change is allowed.

## Gate Evidence

- `docs/design/protocol.md` names `server/crates/protocol/src/lib.rs` as the owner of wire DTOs,
  compact codes, slot schemas, `COMPACT_SNAPSHOT_VERSION`, `PREDICTION_PROTOCOL_VERSION`, and codec
  metadata, with `client/src/protocol.js` as the browser mirror.
- `docs/design/protocol.md` also records a boundary inventory for semantic DTOs, code tables,
  compact slots, adapter kind conversion, `DEFAULT_FACTION_ID`, and `PLAYER_PALETTE`.
- `tests/protocol_parity.mjs` already compares the Rust protocol contract dump to JS tags,
  vocabularies, compact codes, codec/version metadata, docs code tables, selected builders, and
  compact decode fixtures.
- The current source shape has one large Rust protocol crate entry point, one large JS mirror, small
  Rust adapter files, and separate protocol client contracts, so mechanical extraction can keep the
  public import surface stable while moving internals.

## Overall Constraints

- Keep `server/crates/protocol/src/lib.rs`, `server/src/protocol.rs`,
  `server/crates/sim/src/protocol.rs`, and `client/src/protocol.js` as stable public import
  surfaces unless a phase explicitly stops for a new plan.
- Do not split Rust and JS mirrors independently. A code-moving phase must preserve the paired Rust
  owner, JS mirror, parity test, and design-doc boundary in the same commit.
- Do not change the serialized shape of `ClientMessage`, `ServerMessage`, `Command`, start payloads,
  snapshots, replay/lab/branch payloads, events, `ClientNetReport`, or observer analysis payloads.
- Do not change compact snapshot version `23`, snapshot codec version `1`, MessagePack frame magic,
  compact field order, compact codes, optional slot omission behavior, or the unknown-code sentinel
  `255`.
- Do not change exported Rust names such as DTO types, `protocol_contract`,
  `encode_snapshot_frame`, `serialize_compact_snapshot`, `serialize_messagepack_compact_snapshot`,
  `default_snapshot_codec`, `SnapshotCodec`, and `SnapshotFrame`.
- Do not change exported JS names such as `S`, `C`, `CMD`, `KIND`, `*_CODE`,
  `COMPACT_SNAPSHOT_VERSION`, `PREDICTION_PROTOCOL_VERSION`, `parseServerFrame`,
  `decodeServerMessage`, `msg`, and `cmd`.
- Keep `rts-protocol` dependent only on `rts-contract` among workspace crates. Rules- or sim-aware
  kind conversion stays in `server/src/protocol.rs` and `server/crates/sim/src/protocol.rs`.
- If a phase creates new split files, update `scripts/hotspot-analysis.mjs` and
  `plans/hotspots/group-map.md` in that same phase so the protocol mirror remains one logical
  hotspot group.
- If a phase discovers that cleanup requires a compact version bump, field rename, exported-name
  change, stale-client compatibility shim, or protocol migration, stop and report blocked instead of
  converting the phase into a behavior change.
- Use focused verification. At minimum, code-moving phases run `node tests/protocol_parity.mjs`,
  `node tests/client_contracts.mjs`, focused `rts-protocol` tests, and `git diff --check`.
- Each phase must be implemented on its own `zvorygin/` branch, pushed as an owned PR with
  auto-merge armed, and waited on until GitHub reports the PR merged and the phase head is reachable
  from `origin/main`.
- After each phase, mark that phase document done in the implementation commit and provide a handoff
  that names exact verification, what moved, unchanged public APIs, remaining risk, next-phase
  guidance, and manual testing focus.

## Phase Summaries

### [Phase 1 - Mirror Guardrail Baseline](phase-1.md)

Strengthen the tests and hotspot grouping that make later protocol movement reviewable. This phase
should not move protocol source code; it records the stable Rust and JS public surfaces, ensures
future protocol split paths stay in the `protocol-and-contracts` hotspot group, and verifies the
current parity suite still covers the mirrored contract. If the current guardrails cannot be made
clear without changing protocol behavior, stop with a no-go handoff.

### [Phase 2 - Frame Transport Split](phase-2.md)

Extract low-level snapshot frame transport helpers while preserving the same public frame functions.
On Rust, move MessagePack frame-writing internals behind `encode_snapshot_frame`; on JS, move binary
frame parsing and the MessagePack reader behind `parseServerFrame`. Keep compact snapshot semantics,
codec names, versions, frame magic, and caller imports unchanged.

### [Phase 3 - Constants And Contract Metadata Split](phase-3.md)

Extract protocol constants, compact code tables, and contract metadata into focused Rust and JS
modules while re-exporting the same public names. This phase keeps `protocol_contract()` byte-for-byte
compatible in structure and keeps JS `S`, `C`, `CMD`, vocabularies, and `*_CODE` tables available
from `client/src/protocol.js`. It should update the design boundary inventory only to mention the
new internal module paths.

### [Phase 4 - Compact Snapshot Codec Split](phase-4.md)

Extract compact snapshot semantic serialization and decoding into focused Rust and JS modules. The
Rust public serializers and JS `decodeServerMessage` remain the stable entry points, while the
private entity, event, ability-object, visible-tile, remembered-building, and net-status helpers move
behind them. This is the highest-risk movement phase and must preserve representative compact
fixture decode/encode behavior before continuing.

### [Phase 5 - Protocol Cleanup Closeout](phase-5.md)

Run the compatibility sweep after the module splits and clean up only stale comments, import paths,
and design boundary references created by this plan. This phase reruns hotspot analysis to confirm
the protocol mirror remains trackable as one group and checks that external client/server imports
still flow through the stable surfaces. It does not move additional protocol logic unless one of the
previous phases explicitly deferred a small mechanical cleanup.

## Phase Index

1. [Phase 1 - Mirror Guardrail Baseline](phase-1.md)
2. [Phase 2 - Frame Transport Split](phase-2.md)
3. [Phase 3 - Constants And Contract Metadata Split](phase-3.md)
4. [Phase 4 - Compact Snapshot Codec Split](phase-4.md)
5. [Phase 5 - Protocol Cleanup Closeout](phase-5.md)

## Non-Goals

- Do not implement a protocol migration, stale-client compatibility layer, snapshot delta protocol,
  or codec negotiation.
- Do not move command scheduling, fog projection, replay policy, lab policy, lobby lifecycle, or
  match-history behavior as part of protocol cleanup.
- Do not generate protocol code or replace hand-written mirrors in this plan; generation would need
  its own design gate.
- Do not change balance, faction catalog rules, UI affordances, prediction behavior, or gameplay
  command semantics.

## Suggested Execution

Run one phase at a time and wait for each PR to merge before starting the next phase:

```bash
scripts/phase-runner.sh --plan protocolcleanup phase-1 phase-2 phase-3 phase-4 phase-5 --pr --wait
```
