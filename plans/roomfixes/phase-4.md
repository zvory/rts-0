# Phase 4 - Room-Time Client Capability Cleanup

## Phase Status

- [ ] Not started.

## Objective

Remove stale product assumptions from the shared room-time client controls so dev scenario, replay,
and replay-branch affordances are driven by explicit capabilities and matching DOM contracts. This
phase is client-focused and should run only after PR #257, PR #258, and roomfixes Phases 1 through
3 have merged into `main`.

## Work

- Fix the dev scenario Step control contract:
  - `client/index.html` still marks the Step button with `data-step-dev-tick`;
  - `client/src/replay_controls.js` sends `stepRoomTime()` only when it sees
    `dataset.stepRoomTime`;
  - replace the stale product-specific attribute with the neutral room-time attribute and cover the
    real markup path, not only test-created elements.
- Make room-time controls obey the matching capability bits from `room_capabilities.js`:
  - positive speed buttons should be shown/clickable only when `roomTime.setSpeed` is true;
  - pause affordances should be shown/clickable only when `roomTime.pause` is true;
  - step controls should be shown/clickable only when `roomTime.step` is true;
  - relative seek buttons should be shown/clickable only when `roomTime.seekRelative` is true;
  - absolute timeline controls should be built/clickable only when `roomTime.timeline` and
    `roomTime.seekAbsolute` are both true.
- Separate generic room-time controls from replay-only affordances. Replay fog controls should
  still require `visibility.replayVision`, but the replay branch button should not be inferred from
  replay vision alone.
- Decide branch-button eligibility from implementation evidence:
  - first prefer an existing explicit client/server eligibility signal if one exists after Phases 1
    through 3;
  - if no explicit signal exists and the button cannot be made correct from existing payload data,
    add the smallest server protocol capability needed to advertise replay branch action
    eligibility;
  - do not add broader server or start-payload refactors in this phase.
- Preserve teardown behavior. Destroying controls should remove generated replay UI and restore the
  static controls to their neutral hidden/visible baseline without leaking listeners across match
  transitions or replay seeks.

## Expected Touch Points

- `client/index.html`
- `client/src/replay_controls.js`
- `client/src/room_capabilities.js`
- `client/src/app.js` only if capability plumbing from `StartPayload` to `ReplayControls` needs a
  narrow adjustment
- `client/src/match.js` only if room-time mounting or destroy sequencing needs a narrow adjustment
- `tests/client_contracts.mjs`
- `tests/protocol_parity.mjs`, `client/src/protocol.js`,
  `server/crates/protocol/src/lib.rs`, `server/src/protocol.rs`, and
  `docs/design/protocol.md` only if a new branch-action capability bit is required
- `server/src/lobby/session_policy.rs` only if that new capability bit must be populated from
  room/session policy

## Implementation Checklist

- [ ] Confirm PR #257 and PR #258 are merged before starting implementation.
- [ ] Confirm Phases 1, 2, and 3 have merged and start from fresh `origin/main`.
- [ ] Change the static dev scenario Step markup from `data-step-dev-tick` to
      `data-step-room-time`.
- [ ] Add or update client contract coverage that constructs controls from the real static
      attribute names and proves Step sends `net.stepRoomTime()`.
- [ ] Gate every rendered and clicked room-time action by the matching normalized capability bit,
      especially positive speeds by `setSpeed` and timeline creation/use by `timeline` plus
      `seekAbsolute`.
- [ ] Keep replay pause, replay status, replay timeline, and replay fog controls out of dev scenario
      mode unless their capabilities explicitly allow them.
- [ ] Stop using `visibility.replayVision` as the branch-button eligibility signal.
- [ ] Use an existing explicit branch/action eligibility signal if available; otherwise add the
      smallest mirrored capability bit and tests needed for the branch button.
- [ ] Preserve cleanup on `ReplayControls.destroy()` across replay seek `Start` resends and match
      teardown.
- [ ] Avoid replay start payload builder, branch admission, lab drain, live pause, prediction, or
      broader UI redesign work.
- [ ] Run focused verification and record exact commands.
- [ ] Mark this phase file done in the implementation commit.

## Verification

- `node tests/client_contracts.mjs`
- If a protocol capability bit is added: `node tests/protocol_parity.mjs`
- If `SessionPolicy` capability population changes: run the exact focused Rust policy test name,
  for example `cargo test --manifest-path server/Cargo.toml -p rts-server start_capabilities_are_policy_and_recipient_role_driven -- --nocapture`
- `git diff --check`

Do not run broad bundles by default. Rely on the PR `./tests/run-all.sh` gate for full-suite
coverage unless the implementation changes a wider protocol or server policy contract.

## Manual Test Focus

Open a dev scenario, pause it, click Step, and confirm one authoritative tick advances through the
visible Step button. Confirm replay-only seek, timeline, fog, and branch controls are not shown in
that dev scenario unless their advertised capabilities explicitly allow them.

Open a replay, use speed, pause/resume, relative seek, and timeline seek. After each seek-triggered
`Start` resend, confirm the same capability-eligible controls remain available, ineligible controls
stay hidden or inert, the pending seek indicator clears on room-time state, and no duplicate
generated controls appear.

From a replay tick, verify the branch button is present only when the room can actually accept a
branch request, then click it and confirm the branch-staging flow still opens. Also verify a replay
viewer without branch/action eligibility does not show a misleading branch button even if replay
vision controls are available.

## Handoff Expectations

Summarize the final room-time capability matrix, whether a new branch/action capability bit was
needed, and the exact static markup contract used for Step. Include focused verification commands,
manual dev scenario/replay/branch results, and any remaining client affordance ambiguity without
pulling Phase 5 start-payload builder work into this phase.
