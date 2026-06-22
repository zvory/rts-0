# Phase 1 - Mirror Guardrail Baseline

Status: done.

## Goal

Make later protocol source movement safer to review by tightening the current guardrails and group
tracking before any protocol code is moved.

## Scope

- Inspect current exports from `server/crates/protocol/src/lib.rs` and `client/src/protocol.js`.
- Add or strengthen focused assertions that lock the stable public protocol surface used by
  downstream modules.
- Update `scripts/hotspot-analysis.mjs` and `plans/hotspots/group-map.md` so future
  `client/src/protocol_*.js` or `client/src/protocol/**` split files stay in the
  `protocol-and-contracts` group.
- Update `docs/design/protocol.md` only if the boundary inventory needs to name the future internal
  split convention.
- Do not move protocol Rust or JS code in this phase.

## Touch Points

- `tests/protocol_parity.mjs`
- `tests/client_contracts/protocol_contracts.mjs`, only if a client public-surface assertion belongs
  there instead of parity
- `scripts/hotspot-analysis.mjs`
- `plans/hotspots/group-map.md`
- `docs/design/protocol.md`, only for boundary wording

## Constraints

- Preserve every protocol tag, field, compact code, version, enum vocabulary, optional slot, and
  exported API.
- Do not add brittle source-order snapshots that make future mechanical moves noisy without checking
  behavior.
- Prefer structured assertions against imported Rust/JS exports and the protocol contract dump.
- If the needed guardrail would require source generation or a new contract schema, stop and report a
  follow-up gate instead of forcing it into this cleanup phase.

## Verification

- `node tests/protocol_parity.mjs`
- `node tests/client_contracts.mjs` if protocol client contracts are touched
- `node scripts/check-docs-health.mjs` if docs are touched
- `git diff --check`

## Manual Testing Focus

No gameplay manual test is expected. Manually review that the guardrails cover public Rust exports,
public JS exports, compact version/code metadata, and hotspot group tracking before protocol source
files move.

## Handoff

Mark this phase done only after committing the guardrail and group-map changes. Summarize which
exports are now protected, which future split paths remain grouped, which verification passed, and
whether Phase 2 can start moving frame transport internals.
