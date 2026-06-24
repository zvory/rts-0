# Phase 5 - Protocol Cleanup Closeout

Status: done.

## Goal

Confirm the protocol mirror remains behaviorally unchanged and trackable as one hotspot group after
the module splits.

## Scope

- Remove only stale comments, stale internal import paths, or obsolete boundary notes created by
  Phases 1-4.
- Inspect external Rust and JS imports to confirm normal callers still import through
  `rts_protocol`, `server/src/protocol.rs`, `server/crates/sim/src/protocol.rs`, or
  `client/src/protocol.js`.
- Rerun hotspot analysis with the protocol split paths included in the `protocol-and-contracts`
  group and summarize whether the logical group stayed trackable.
- Update `docs/design/protocol.md`, `plans/hotspots/group-map.md`, or
  `scripts/hotspot-analysis.mjs` only if earlier phases left stale references.
- Do not move additional protocol logic unless a previous phase explicitly deferred a small
  mechanical cleanup that still preserves this plan's invariants.

## Touch Points

- `docs/design/protocol.md`
- `plans/hotspots/group-map.md`
- `scripts/hotspot-analysis.mjs`
- protocol split files created in previous phases, only for stale comments or import cleanup
- `plans/archive/protocolcleanup/phase-5.md`

## Constraints

- Do not change protocol behavior, exported names, compact schemas, parity fixtures, or runtime
  import surfaces.
- Do not broaden this closeout into a new cleanup phase. If new protocol movement still looks
  worthwhile, write a follow-up plan instead.
- Keep unrelated gameplay, balance, UI, lobby, replay, and room-runtime code out of this phase.

## Verification

- `node tests/protocol_parity.mjs`
- `node tests/client_contracts.mjs`
- focused Rust protocol tests if Rust protocol comments/imports are touched
- `node scripts/check-docs-health.mjs`
- `node scripts/hotspot-analysis.mjs --base-ref HEAD --recent-days 14 --limit 0 --output /tmp/rts-protocol-hotspots-after.json`
- `git diff --check`

## Manual Testing Focus

No gameplay manual test is expected unless Phase 5 touches runtime imports. Manually review the
hotspot output and confirm the protocol mirror still appears as one logical group instead of
separate Rust and JS cleanup leftovers.

## Handoff

Mark this phase done only after committing the closeout. Summarize hotspot-analysis output, any
stale references removed, verification, and whether any future protocol cleanup needs a new design
gate.
