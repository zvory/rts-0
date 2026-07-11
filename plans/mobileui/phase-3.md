# Phase 3 - Mobile Debug Chrome and Desktop Parity

## Phase Status

- [x] Done.

Focused automated contracts passed; desktop and real-device visual QA remain required in PR review because the executor sandbox cannot bind a local server or attach a browser.

## Objective

Make the already-existing game/replay chrome reachable and non-overlapping on phone portrait, phone landscape, and tablet screens without changing the desktop game presentation. The mobile layout is for remote debugging and observation: it must surface game status, room-time/replay controls, minimap, settings, and existing in-game diagnostics, while lower-priority selection/command detail can be collapsed or scrolled rather than competing for the same pixels. This phase is the final scope boundary for the plan; it does not make a Lab component system.

## Work

- Introduce a mobile-debug presentation only under a small-viewport and coarse-pointer gate. Do not use a width-only media query that changes a narrow desktop browser's UI.
- Establish a safe-area-aware, non-overlapping layout for HUD, room-time/replay controls, minimap, settings, selection detail, command card, observer analysis, and in-game AI diagnostics. Essential controls must remain reachable; less important detail may be collapsed, scrollable, or surfaced one panel at a time, but must not become inaccessible.
- Give adjacent mobile-debug controls a practical target size and spacing, with a 36px minimum as a working floor and larger targets where the available space allows. Keep shipped desktop dimensions untouched outside the mobile-debug gate.
- Ensure overlay hit areas do not leak into the Pixi viewport and that scrollable panel bodies remain scrollable. Do not apply touch-action: none broadly to solve layout or tap issues.
- Make existing in-game observer analysis and AI diagnostics show/hide/tab controls reachable on mobile only if it can be done without changing LabPanelWindowChrome or Lab panel APIs. If that boundary cannot be respected, record it for the later Lab/component-library plan instead of expanding this phase.
- Add responsive DOM/contract coverage where practical and use browser visual checks to prove that desktop remains unchanged. Keep visual rules local to the game/replay chrome rather than adding a global CSS framework/reset.

## Expected Touch Points

- client/styles.css
- client/src/replay_controls.js and client/src/room_time_panel.js only for mobile presentation state that Phase 1 has already established
- client/src/settings_container.js, client/src/observer_analysis_overlay.js, or client/src/ai_diagnostics_panel.js only for narrow in-game mobile reachability fixes
- client/index.html only if an existing, semantic mobile-debug control requires a stable mount point; prefer generated/owned DOM where that avoids a broad pinned-markup change
- tests/client_contracts/settings_contracts.mjs
- tests/client_contracts/observer_analysis_contracts.mjs
- tests/client_contracts/match_shell_contracts.mjs
- tests/client_contracts/match_replay_contracts.mjs
- A focused responsive/browser contract as needed
- plans/mobileui/phase-3.md status update in the implementation commit

## Explicit Exclusions

- No Lab panel/catalog/map-editor work, no external component library, and no global CSS reset.
- No desktop control relocation, styling refresh, or desktop-only feature removal to make room for mobile chrome.
- No mobile world-command system, touch selection system, or new touch gameplay controls.
- No server/protocol/simulation change.

## Desktop Preservation Gate

- Capture and review the in-game/replay UI at 1440x900 and 1366x768 before and after changes.
- Run a desktop mouse/keyboard pass covering HUD readability, minimap interaction, room-time/replay actions, settings, command card, selection panel, observer analysis, and any visible AI diagnostic control.
- The desktop presentation must retain its current panel hierarchy and placement. Any material visual drift is a blocker, even if the mobile layout improves.

## Implementation Checklist

- [x] Inventory all game/replay overlays at 390x844, 844x390, and a tablet viewport, including safe-area cases.
- [x] Implement the coarse-pointer-gated mobile-debug layout without width-only desktop changes.
- [x] Verify responsive placement and reachability through focused layout contracts; browser QA remains required before release.
- [x] Add/adjust focused contracts for changed responsive behavior.
- [ ] Complete the desktop preservation gate and real-device Tailscale pass (blocked in this executor sandbox; required in PR review).
- [x] Mark this phase done in this file in the implementation commit.

## Verification

    node tests/client_contracts/settings_contracts.mjs
    node tests/client_contracts/observer_analysis_contracts.mjs
    node tests/client_contracts/match_shell_contracts.mjs
    node tests/client_contracts/match_replay_contracts.mjs
    node tests/minimap_input_contracts.mjs
    node scripts/check-client-architecture.mjs
    git diff --check

Run node tests/client_smoke.mjs or the selected browser suite against a running local server for desktop regression coverage. Run node tests/select-suites.mjs --verify if suite-selection files change.

## Manual Test Focus

Use a Tailscale-served local game/replay on a phone in portrait and landscape, plus a tablet if available. Confirm room time, minimap, settings, status/HUD, observer analysis, and AI diagnostics are reachable without overlap; confirm lower-priority detail can be reached if collapsed. Repeat the same flow with desktop mouse/keyboard at 1440x900 and 1366x768, and reject any material desktop visual or interaction change.

## Handoff Expectations

This is the terminal phase for the mobile game-debug effort. State the verified device/browser matrix, known mobile limitations, exact desktop parity evidence, and the Tailscale URL or replay setup used for the manual pass. Explicitly say that Lab component-library work remains out of scope and must begin from a separate plan.
