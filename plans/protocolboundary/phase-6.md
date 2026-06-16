# Phase 6 - Docs And Guardrails

## Phase Status

- [x] Done.

## Objective

Finalize documentation and lightweight checks for the protocol/config mirror boundary.

## Work

- Update `docs/design/protocol.md`, `docs/design/balance.md`, and context capsules with the final
  source-of-truth map and parity commands.
- Document intentional cross-surface guards, including palette parity if it remains in
  `tests/protocol_parity.mjs` even though ownership is lobby/config rather than the wire protocol.
- Add or adjust lightweight checks only if earlier phases reveal repeatable dependency or mirror
  mistakes.
- Keep this phase behavior-neutral.

## Expected Touch Points

- `docs/design/protocol.md`
- `docs/design/balance.md`
- `docs/context/protocol.md`
- `docs/context/balance.md`
- `tests/protocol_parity.mjs`
- `scripts/check-faction-catalog-parity.mjs`

## Implementation Checklist

- [x] Document the final authority map.
- [x] Document required commands for protocol and balance changes.
- [x] Tighten any parity guardrails justified by earlier phases.
- [x] Run verification and record exact results in the handoff.

## Verification

- `node tests/protocol_parity.mjs`
- `node scripts/check-faction-catalog-parity.mjs`
- `git diff --check`

## Manual Test Focus

One normal match start and command-card inspection if docs or checks caused config/protocol churn.

## Handoff Expectations

Summarize remaining manually mirrored data and any deferred generation work.
