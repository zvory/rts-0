# Phase 2 - Inventory Refresh And Boundary Ratchets

## Phase Status

- [x] Done.

## Objective

Refresh faction inventory docs and ratchet against contradictory boundary language.

## Work

- Update inventory sections for catalog ids, protocol fields, command cards, AI, prediction/WASM,
  replay, and lifecycle paths.
- Separate playable, fixture-only, reserved/future, and historical claims.
- Add checker anchors that fail if active docs regress into contradictory language.
- Ratchet known high-risk direct faction special cases without expanding approved lists casually.

## Expected Touch Points

- `docs/design/faction-architecture-inventory.md`
- `docs/design/protocol.md`
- `docs/design/balance.md`
- `docs/design/client-ui.md`
- `scripts/check-faction-assumptions.mjs`

## Implementation Checklist

- [x] Refresh inventory to current code-confirmed facts.
- [x] Add playable/fixture/reserved/historical boundary language.
- [x] Add checker anchors for boundary language.
- [x] Ratchet direct special-case growth carefully.
- [x] Run verification and record exact results in the handoff.

## Verification

- `node scripts/check-faction-assumptions.mjs`
- `node tests/protocol_parity.mjs`
- `node tests/hud_command_card.mjs`

## Manual Test Focus

Human docs review only: confirm wording does not reintroduce old reserved-boundary drift.

## Handoff Expectations

List intentional checker ratchet changes and any remaining boundary ambiguity.
