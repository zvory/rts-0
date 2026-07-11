# Phase 2 - Minimap Pointer Gestures

## Phase Status

- [x] Done.

Physical-device and desktop manual validation remain required before release.

## Objective

Give the existing minimap a first-class, predictable Pointer Events path for mobile inspection while preserving every current desktop mouse action. A touch/pen user must be able to tap or drag the minimap to inspect the world without relying on synthesized mouse events. This remains a debug surface, not a new phone RTS command scheme.

## Work

- Replace the minimap's mouse-only input dependency with an explicit pointer lifecycle that handles pointerdown, pointermove, pointerup, and pointercancel, owns/releases pointer capture, and cleans up on window blur/destroy.
- Retain desktop semantics exactly: primary mouse click/drag keeps its current camera behavior, right-click retains contextual orders where currently allowed, Shift queueing remains intact, and pointer-lock/router paths keep their existing source and priority behavior.
- Define touch/pen semantics deliberately: a primary tap recenters or uses the existing primary-target behavior, a drag continues camera inspection, and a cancelled or moved gesture must not issue a tap action. Do not invent a long-press/right-click substitute or direct touch move/attack/gather orders.
- Preserve the existing explicitly armed command-target behavior only after a clean primary tap; never let a pinch, drag, cancellation, or accidental minimap inspection trigger an armed command.
- Apply touch-action: none only to the minimap gesture canvas/handle as needed. Do not disable scrolling or native touch behavior on surrounding panels, settings, or the document just to make minimap gestures work.
- Cover CSS-scaled canvas coordinates, pointer capture outside the canvas, cancellation, teardown, normal mouse/right-click regression, replay read-only behavior, and Lab-policy behavior already represented by the minimap contract. Do not refactor Lab UI while preserving its policy seam.

## Expected Touch Points

- client/src/minimap.js
- client/styles.css
- client/src/input/router.js only if a backward-compatible router contract extension is necessary
- tests/minimap_input_contracts.mjs
- tests/client_contracts/input_contracts.mjs or a focused new pointer gesture contract
- tests/client_contracts/match_replay_contracts.mjs if room-time/replay minimap behavior changes
- plans/mobileui/phase-2.md status update in the implementation commit

## Explicit Exclusions

- No changes to canvas art, map rendering style, fog rules, camera movement rules outside minimap interaction, world touch selection, or mobile right-click emulation.
- No Lab component-library work or refactor of Lab panels.
- No change to desktop visual layout or right-click/order semantics.

## Desktop Preservation Gate

- At 1440x900 and 1366x768, manually verify minimap primary click, drag, right-click contextual order, Shift-right-click queued order, replay read-only minimap behavior, and pointer-lock routing.
- If minimap rendering or canvas scaling changes, capture an authoritative Lab Interact scene and inspect it once; the capture is review evidence only and must not be committed.
- A desktop mouse user must not see touch-specific affordances, altered minimap size, or changed command behavior.

## Implementation Checklist

- [x] Capture the existing desktop minimap behavior and coordinate baseline.
- [x] Implement pointer capture/cancel-safe minimap gestures without compatibility-mouse reliance.
- [x] Define and test clean-tap versus drag/cancel semantics for touch and pen.
- [x] Preserve and test desktop left/right/Shift-right minimap behavior, replay behavior, and router behavior.
- [ ] Complete real-phone portrait and landscape minimap verification over Tailscale.
- [x] Mark this phase done in this file in the implementation commit.

## Verification

    node tests/minimap_input_contracts.mjs
    node tests/client_contracts/input_contracts.mjs
    node tests/client_contracts/match_replay_contracts.mjs
    node scripts/check-client-architecture.mjs
    git diff --check

Run the appropriate client smoke/browser suite when the event route or canvas sizing changes.

## Manual Test Focus

On desktop, verify minimap camera movement and right-click orders are indistinguishable from current behavior. On phone and tablet, tap to inspect the camera, drag across/outside the minimap, cancel a gesture, rotate to landscape, and verify no accidental command fires. Confirm replay viewing stays read-only and an intentionally armed primary target only fires after a clean tap.

## Handoff Expectations

Describe the final touch/pen semantics and whether any browser required a compatibility fallback. Tell the Phase 3 agent which mobile dimensions or overlay collisions remain to resolve, and name the desktop minimap checks that passed. Include the local Lab Interact capture path if rendering changed.
