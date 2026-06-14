# Phase 1 - Server Command Budget Validation

Status: Not started.

## Goal

Replace count-only multi-unit command hardening with authoritative command-budget validation. The
server should reject over-budget human unit-list commands while retaining an absolute defensive
id-list bound against huge malformed payloads.

## Scope

- Add named server-side command-budget constants:
  - base command supply cap: 24
  - Command Car cap bonus: 12 per admitted Command Car
  - defensive absolute unit-id processing cap, preserving the safety intent of
    `MAX_UNITS_PER_COMMAND`
- Add a command weight helper that uses authoritative supply where available and falls back to 1
  for selectable entities without supply.
- Replace or wrap `dedupe_cap_units` with a validation helper that:
  - dedupes while preserving first-seen order
  - resolves valid owned/controllable command entities
  - pre-admits Command Cars in the submitted unit set so each one reliably contributes its bonus
  - computes total used command supply and effective cap
  - rejects the command when the submitted legal unit set exceeds the effective cap
  - rejects or ignores duplicate/missing ids according to the existing command semantics, but does
    not silently trim valid overflow units
- Apply the helper consistently to human `SimCommand` variants that carry unit ids, including:
  - `move`
  - `attackMove`
  - `attack`
  - `gather`
  - unit build worker selection
  - `stop`
  - `setupAntiTankGuns`
  - `tearDownAntiTankGuns`
  - `useAbility`
- Keep AI out of scope unless the command application path cannot distinguish human/AI commands.
  If the existing path has no source distinction, document that as a blocker or add the narrowest
  source metadata needed before enforcing the limit.
- Update `docs/design/hardening.md` and `docs/design/server-sim.md` if command validation semantics
  change.

## Expected Deliverables

- Server command validation rejects over-budget human unit-list commands.
- Existing command validation still dedupes and bounds malformed id lists.
- Command Cars stack their +12 cap bonus.
- Overflow is rejected, not filtered.
- Any protocol rejection reason additions are mirrored in protocol files and docs.

## Verification

- Run focused Rust tests for command validation, for example targeted `cargo test` names under
  `server/crates/sim`.
- Add or update tests proving:
  - 24 one-supply units are legal
  - a fifth 6-supply Tank is rejected without a Command Car
  - one Command Car increases the legal cap by 12 while consuming its own supply
  - multiple Command Cars stack
  - huge duplicate id lists remain bounded
- Do not run broad bundles during development; rely on the commit hook when the phase is ready.

## Manual Testing Focus

Start a local match and issue normal small move, attack-move, stop, setup, and ability commands.
Confirm normal commands still work and malformed oversized commands do not crash the room task.

## Handoff Expectations

The handoff must identify how the server distinguishes human commands from AI commands, list the
command variants covered, and call out any validation paths left for Phase 2 or Phase 3.
