# Phase 1 - Server Command Budget Validation

Status: Not started.

## Goal

Replace count-only multi-unit command hardening with authoritative command-budget validation and a
minimal client command-send guard. The server should reject over-budget submitted human unit-list
commands while retaining an absolute defensive id-list bound against huge malformed payloads, and the
current count-capped client should avoid sending commands this phase makes illegal.

## Scope

- Add named server-side command-budget constants:
  - base command supply cap: 24
  - Command Car cap bonus: 12 per admitted Command Car
  - defensive absolute unit-id processing cap, preserving the safety intent of
    `MAX_UNITS_PER_COMMAND`
- Add a command weight helper that uses authoritative supply where available and falls back to 1
  for selectable entities without supply.
- Before enforcing the budget, resolve the live AI source policy explicitly. The current `Game`
  queue carries only `(player, SimCommand)`, and live AI uses the same `Game::enqueue` path as
  human commands. Choose one of these strategies and document it in the phase notes:
  - exempt commands issued by players whose `PlayerState.is_ai` is true
  - add narrow command-source metadata at the `Game::enqueue` boundary
  - intentionally apply the same budget to AI and update this plan plus AI expectations
- Replace or wrap `dedupe_cap_units` with a validation helper that:
  - dedupes while preserving first-seen order
  - resolves valid owned/controllable command entities
  - pre-admits Command Cars in the submitted unit set so each one reliably contributes its bonus
  - computes total used command supply and effective cap
  - rejects the command when the submitted legal unit set exceeds the effective cap
  - rejects or ignores duplicate/missing ids according to the existing command semantics, but does
    not silently trim valid overflow units
- Validate command budget against the submitted command unit ids only. Do not depend on local
  selection context that the server cannot verify. Command Cars increase a command's budget only when
  they are included in that submitted unit list.
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
  - `setAutocast`
- Add a narrow client command-send guard in the existing command issuing paths so honest clients do
  not send over-budget unit lists after this phase. This is not the full selection overhaul; it
  should:
  - reuse or mirror the same base cap, Command Car bonus, and weight rules
  - validate the exact ids being submitted for each command, including subset commands such as
    worker-only build/gather, ability carriers, setup/teardown-capable units, and minimap commands
  - cover right-click, targeted command-card commands, stop/setup/teardown/ability/autocast command
    sends, and minimap command sends
  - block the outgoing command and trigger a lightweight overflow signal or existing notice path
  - leave selection size, control-group storage, and HUD rendering otherwise unchanged until later
    phases
- Define server rejection feedback before adding new rejection reasons. Prefer a local player notice
  or existing command-rejection path that is visible enough for debugging, but do not introduce a
  broad protocol change unless the existing event/notice path cannot express the rejection.
- Update `docs/design/hardening.md` and `docs/design/server-sim.md` if command validation semantics
  change.

## Expected Deliverables

- Server command validation rejects over-budget human unit-list commands.
- Existing command validation still dedupes and bounds malformed id lists.
- Command Cars stack their +12 cap bonus.
- Overflow is rejected, not filtered.
- Existing honest-client command paths do not send over-budget unit lists after this phase, even
  before Phase 2 removes the old selected-count cap.
- The AI source policy is documented and implemented consistently.
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
- Add or update focused client tests proving the interim command-send guard blocks an over-budget
  Tank-heavy selected command, still allows a legal command, and validates subset command ids rather
  than broader selected context.
- Do not run broad bundles during development; rely on the commit hook when the phase is ready.

## Manual Testing Focus

Start a local match and issue normal small move, attack-move, stop, setup, autocast, and ability
commands. Select an over-budget Tank-heavy group through the existing 12-count client behavior and
confirm the client does not send the command, while malformed oversized commands still do not crash
the room task.

## Handoff Expectations

The handoff must identify the chosen AI/source policy, list the command variants covered on the
server, list the client command-send paths guarded, confirm the submitted-id validation rule for
subset commands, and call out any validation paths left for Phase 2 or Phase 3.
