# Phase 6 - Gate Wiring And Selector Policy

## Phase Status

- [ ] Not implemented.

## Objective

Run faction guardrails through normal local gates and targeted suite selection.

## Work

- Add the faction assumption checker and catalog parity checker to `tests/run-all.sh` at the
  architecture/contract layer.
- Teach `tests/select-suites.mjs` that faction catalog, protocol/config mirror, faction docs, and
  checker script changes select the right focused suites.
- Keep live-server test selection out of docs-only changes unless required by touched files.

## Expected Touch Points

- `tests/run-all.sh`
- `tests/select-suites.mjs`
- `docs/design/testing.md` if test policy is documented there

## Implementation Checklist

- [ ] Wire checker scripts into the full local gate.
- [ ] Add selector cases for faction-sensitive files.
- [ ] Verify selector examples.
- [ ] Confirm failure output is readable.
- [ ] Run verification and record exact results in the handoff.

## Verification

- `node tests/select-suites.mjs --verify`
- `node scripts/check-faction-assumptions.mjs`
- `node scripts/check-faction-catalog-parity.mjs`
- `tests/run-all.sh --no-client` only if runner wiring risk is high

## Manual Test Focus

No gameplay test expected. Confirm command output is understandable when a checker fails.

## Handoff Expectations

State where guard scripts run in the full gate and list selector mappings added for future agents.
