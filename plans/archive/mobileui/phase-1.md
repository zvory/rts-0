# Phase 1 - Authoritative Room-Time Controls

## Phase Status

- [x] Done. Physical-device manual validation remains required before release.

## Objective

Make the existing room-time/replay control surface reliable on touch, pen, mouse, and keyboard, and make its visible selected state honest about the server's authoritative room-time state. This phase addresses Reset/rewind, pause/resume, step, every speed, replay vision selection, branch action, and timeline seeking where each capability is advertised. It does not redesign the panel or broaden into Lab controls.

## Work

- Preserve native semantic button and click behavior as the keyboard and assistive-technology path. Add a narrowly scoped touch/pen activation path for room-time controls only when needed, using pointer identity, inside-release/cancel handling, and suppression of exactly the resulting duplicate synthetic click.
- Do not attach a pointer-up action indiscriminately to all client buttons. The implementation must prove one physical tap invokes one action, a cancelled/dragged/outside release invokes none, and keyboard Enter/Space plus ordinary mouse clicks still invoke exactly one action.
- Make Net room-time methods report whether the existing WebSocket send occurred, without changing their wire messages. RoomTimeControls must show a bounded pending state only after a successful send and must not commit active speed/pause/seek presentation as accepted until the next authoritative roomTimeState arrives.
- On an immediately blocked socket send, restore the prior authoritative state and expose a concise local failure. On a bounded lack of confirmation, revert pending presentation and expose an actionable unavailable/unchanged state; do not add a protocol field or silently claim success.
- Keep capabilities authoritative. Hidden/disabled controls must remain inactive, and Lab authorization no-ops must be visibly distinguishable from an accepted action timing out.
- Keep the existing desktop visual language and panel geometry. Any mobile target-size adjustment is limited to the eventual coarse-pointer mobile rules; do not change desktop button dimensions in this phase unless a desktop regression review explicitly approves it.

## Expected Touch Points

- client/src/replay_controls.js
- client/src/net.js
- client/src/panel_touch_activation.js or a narrowly named successor confined to core game/replay controls
- client/src/room_time_panel.js only if event ownership requires it
- client/styles.css
- tests/client_contracts/match_replay_contracts.mjs
- tests/client_contracts/room_time_panel_contracts.mjs
- tests/client_contracts/net_contracts.mjs
- A focused browser/mobile control contract if the existing fake DOM coverage cannot exercise the event sequence safely
- plans/mobileui/phase-1.md status update in the implementation commit

## Explicit Exclusions

- No Lab panel, Lab map editor, Lab catalog, AI-library, or third-party dependency work.
- No server/protocol change unless the existing roomTimeState cannot support truthful pending and failure presentation; stop and request a scope decision if that proves necessary.
- No desktop layout redesign and no global button-library migration.

## Desktop Preservation Gate

- Before and after the change, review room-time/replay controls at 1440x900 and 1366x768.
- Verify mouse click, keyboard focus/Enter/Space, speed selection, pause/resume, replay seeking, timeline seeking, vision toggles, and branch action retain their current desktop behavior.
- If the desktop panel's position, wrapping, visual hierarchy, or keyboard behavior materially changes, fix or revert it before the phase PR is opened.

## Implementation Checklist

- [x] Record the current event/send/roomTimeState sequence for the reported speed-control case.
- [x] Add reliable, de-duplicated touch/pen activation for every eligible room-time action.
- [x] Add truthful pending, confirmation, failed-send, and no-confirmation presentation using the current authority message.
- [x] Add focused coverage for touch, pen, mouse, keyboard, duplicate synthetic click, cancellation, blocked socket send, and authoritative confirmation.
- [ ] Complete desktop preservation checks and a real iPhone Safari or Android Chrome replay pass.
- [x] Mark this phase done in this file in the implementation commit.

## Verification

    node tests/client_contracts/net_contracts.mjs
    node tests/client_contracts/room_time_panel_contracts.mjs
    node tests/client_contracts/match_replay_contracts.mjs
    node scripts/check-client-architecture.mjs
    git diff --check

Run the selected browser/client smoke suite when the implementation changes browser event wiring.

## Manual Test Focus

On desktop, use mouse and keyboard to operate every visible replay speed, pause/resume, rewind, timeline, vision, and branch control. On a phone over Tailscale, tap the same controls, deliberately drag/cancel one tap, and confirm each accepted action results in the authoritative status changing exactly once. Confirm a disconnected or unauthorized case does not leave an incorrect selected speed behind.

## Handoff Expectations

State which real devices/browsers were tested, whether the original lost-click symptom reproduced, and how the final pending/failure state behaves. Name Phase 2 as the next work, including the minimap touch interactions that must be manually tested. State explicitly that the desktop preservation gate passed before handing off.
