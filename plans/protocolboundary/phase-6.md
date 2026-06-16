# Phase 6 - Docs And Guardrails

## Phase Status

- [ ] Not implemented.

## Objective

Finalize documentation and lightweight checks for the protocol/config mirror boundary.

## Work

- Update `docs/design/protocol.md`, `docs/design/balance.md`, and context capsules with the final
  source-of-truth map and parity commands.
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

- [ ] Document the final authority map.
- [ ] Document required commands for protocol and balance changes.
- [ ] Tighten any parity guardrails justified by earlier phases.
- [ ] Run verification and record exact results in the handoff.

## Verification

- `node tests/protocol_parity.mjs`
- `node scripts/check-faction-catalog-parity.mjs`
- `git diff --check`

## Manual Test Focus

One normal match start and command-card inspection if docs or checks caused config/protocol churn.

## Handoff Expectations

Summarize remaining manually mirrored data and any deferred generation work.
