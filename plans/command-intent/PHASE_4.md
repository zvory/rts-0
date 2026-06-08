# Phase 4 - Client Command Composer

Status: Planned.

Goal: replace one-off hotkey/Shift exceptions with a legible client-side command composer that
turns input state into server commands matching the authoritative planner rules.

## Scope

- Add a pure client module, e.g. `client/src/input/command_composer.js`, with no DOM or network
  dependency.
- Model prepared command state:
  - currently armed order or ability
  - whether the arming came from tap or held key
  - whether Shift is keeping the command alive
  - quick-cast/double-tap handling
- Support these UX rules:
  - tapping an ability/order key arms the command
  - clicking issues the armed command
  - tapping the same ability/order again quick-casts at the current cursor position where applicable
  - holding an ability/order key and clicking multiple times issues multiple commands
  - tapping a command, then holding Shift and clicking multiple times queues multiple commands
  - releasing Shift clears a Shift-preserved arming
  - Shift can keep the last pressed ability alive after the ability key is released
- Keep right-click context orders working for move/gather/attack/resume-build.
- Keep minimap command issuance consistent with viewport command issuance.

## Non-Goals

- Do not let the client decide final eligibility; it only sends intent.
- Do not add speculative future-cooldown projection to previews.
- Do not change protocol shape unless required by Phase 2 server work.

## Tests

- Unit tests for command arming lifetime: tap, hold, release, Shift preservation, Esc/right-click
  cancel.
- Unit tests for quick-cast/double-tap issuing at the current cursor.
- Unit tests that held Smoke plus repeated clicks emits repeated `useAbility(smoke, ..., queued)`
  commands.
- Unit tests that tapped Attack plus Shift repeated clicks emits queued attack/attack-move commands
  until Shift release.
- Smoke/Attack interleaving while Shift-held emits commands in the clicked order.

## Done

- Input code delegates arming/clearing decisions to the composer instead of hand-written special
  cases.
- Existing selection, placement, control groups, camera panning, and command-card behavior still
  work.
- Client smoke/manual tests pass.
