# Phase 8C - Client Branch Staging UX

## Objective

Build the client flow for requesting a branch from replay playback, moving into branch staging, and
claiming seats.

## Client Work

- Add a compact resume action to replay controls.
- On click, send the branch request for the current server replay tick. No additional explanatory
  competitive/rematch copy is needed.
- Handle the branch-created server message by tearing down replay viewer state and entering branch
  staging.
- Render branch staging as a focused room screen:
  - original replay seats in order
  - claim/release controls
  - occupant list for viewers who have not claimed seats
  - host-only start button
  - ready/start disabled state until all seats are claimed
- Keep player colors and original names visible so seat identity is obvious.
- Do not expose normal lobby controls that do not apply:
  - add/remove AI
  - debug/quickstart
  - map selection
  - spectator toggles that conflict with seat claiming
- Reuse existing client teardown discipline so replay listeners, replay controls, and WebGL state
  do not leak into branch staging.

## Protocol Mirror Work

- Add JS builders/parsers for branch request, seat claim, seat release, and start messages.
- Add branch staging server message decoding.
- Update client contract tests for new message shapes.

## Verification

- Client contract test that resume sends the branch request from replay mode.
- Client contract test that branch staging renders seats and claim controls.
- Client contract test that claimed seats update without duplicating listeners.
- Client smoke or integration coverage that replay viewer tears down when moved to branch staging.
- Test that normal replay controls are absent in branch staging.

## Player-Facing Outcome

Replay viewers can create a branch, move into a staging screen, and claim seats before launch.
