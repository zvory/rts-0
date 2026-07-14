# Mobile Game Debug UI Plan

## Purpose

Make the existing in-game and replay UI usable for secondary mobile debugging and observation without turning Bewegungskrieg into a touch-first RTS. The deliverable is reliable room-time and minimap interaction plus a non-overlapping, touch-reachable mobile debug layout for the existing game/replay chrome. This plan ends there: a future, separately owned plan may choose a Lab component library, but it must not be pulled into this work.

## Current Evidence

- RoomTimeControls delegates speed, seek, pause, vision, and timeline activation through click, while the existing touch/pen release workaround is used only by floating-panel collapse buttons.
- The historical mobile fix that introduced that workaround explicitly described press feedback with a delayed or lost synthesized click.
- The minimap canvas installs mousedown plus window mousemove/mouseup; it has no first-class Pointer Events or Touch Events path. The viewport has a separate touch pan/pinch path, so input behavior is inconsistent by surface.
- Room-time speed buttons optimistically change their active state before an authoritative roomTimeState arrives, and Net.setRoomTimeSpeed currently hides a failed socket send. A persistent highlight can therefore be an acknowledgement or authorization failure rather than a missed tap.
- The desktop-oriented overlay layout leaves speed controls below a practical touch target size and can overlap minimap, selection, command-card, and diagnostic surfaces at phone dimensions.
- The client is plain ES modules and global PixiJS, with no frontend framework or component package. Existing panel helpers are evidence for narrowly shared behavior, not a reason to introduce a product-wide library during this effort.

## Overall Constraints

- Desktop preservation is a hard release requirement. The desktop game is the product; its visual hierarchy, panel placement, mouse/right-click behavior, keyboard/hotkey behavior, pointer-lock behavior, and normal game/replay flows must not regress or be redesigned for mobile. A phase is blocked if its desktop capture or desktop manual pass shows a material regression.
- Gate mobile presentation rules on a coarse-pointer capability as well as the relevant small viewport condition. Resizing a normal mouse/keyboard desktop browser to a narrow width must not silently switch it to a mobile UI.
- Mobile is a deliberately second-class debugging/observation surface. Support reliable camera and minimap inspection, room-time/replay controls, settings, HUD status, and existing in-game diagnostics; do not add a full touch gameplay scheme for world selection, right-click orders, or competitive play.
- Keep this client-only where possible. Do not change simulation, balance, replay format, or the wire protocol; if existing roomTimeState cannot give an honest accepted/rejected result, stop and request an explicit scope decision before proposing a protocol change.
- Do not add a UI framework, a component library, a CSS reset, or a third-party dependency. Do not edit Lab catalog/panel/map-editor code or make Lab styling decisions. In-game AI diagnostics and observer analysis may receive narrowly scoped mobile-debug reachability fixes, but must not be used to refactor the Lab panel system.
- Prefer native semantic controls and small behavior helpers over a new custom-element suite. A touch fallback must preserve native keyboard/click activation, handle cancel/outside release, and guarantee exactly one action for one tap.
- Maintain teardown discipline for every new listener or timer. Keep client dependencies injected through the existing Match/App seams and run the architecture checker after client changes.
- For every phase, capture a before/after desktop baseline at 1440x900 and 1366x768, then exercise mouse, keyboard, and right-click behavior relevant to the changed surface. When a phase changes minimap/canvas rendering, use the project Interact capture workflow; do not commit capture bytes.
- Every implementation phase gets its own zvorygin branch, owned PR, auto-merge, and definite merge verification before the next phase starts. The implementation commit marks the phase file done and the handoff names the next phase plus core manual tests.

## Phase Summaries

### [Phase 1 - Authoritative Room-Time Controls](phase-1.md)

Make every existing room-time/replay control dependable on touch, pen, mouse, and keyboard without changing its desktop look or semantics. The controls must show pending state after a successful send and become selected only when the server's existing roomTimeState confirms the action; failed sends and bounded missing confirmations must visibly revert rather than masquerade as success. The phase adds focused tests and a real-phone replay pass for speeds, pause, seek, vision, and timeline use.

### [Phase 2 - Minimap Pointer Gestures](phase-2.md)

Replace the minimap's mouse-only dependency with an explicit, capture-safe Pointer Events path while retaining all desktop mouse behavior. Touch gets predictable primary-pointer tap/drag camera inspection and no invented touch equivalent for desktop right-click orders; explicit armed target behavior remains deliberate and must not fire after a drag. The phase covers scaled coordinates, cancel/capture cleanup, desktop right-click regression, and a real-phone minimap pass.

### [Phase 3 - Mobile Debug Chrome and Desktop Parity](phase-3.md)

Lay out the existing game/replay debug chrome so essential surfaces remain reachable and do not overlap on common phone portrait, phone landscape, and tablet sizes. This is a mobile-only presentation mode: desktop geometry and visual language remain unchanged, while lower-priority selection/command/diagnostic surfaces may be collapsed or made scrollable but never lost. The phase finishes with desktop visual/input parity evidence and a Tailscale-served phone verification pass.

## Phase Index

1. [Phase 1 - Authoritative Room-Time Controls](phase-1.md)
2. [Phase 2 - Minimap Pointer Gestures](phase-2.md)
3. [Phase 3 - Mobile Debug Chrome and Desktop Parity](phase-3.md)

## Non-Goals

- Do not make live competitive RTS play workable on a phone.
- Do not add touch world selection, touch move/attack/gather orders, a long-press right-click substitute, or a redesigned command language.
- Do not change Lab panels, Lab setup/catalog/map editing, or choose/install a Lab component library. Those are explicitly reserved for the follow-up planning effort.
- Do not replace the player-facing game/replay chrome with Web Awesome, Pico, Spectrum, Bootstrap, React, or another external UI system.
- Do not change server simulation, balance, replay data, fog, protocol fields, or room-time authority rules without a separately approved scope expansion.
- Do not relax desktop UI quality, hide desktop controls, or use a viewport-width-only rule that changes ordinary desktop behavior.

## Required Verification Themes

- Focused contracts for the touched control/input modules, including new touch/pen cancellation and duplicate-activation coverage.
- Run minimap input contracts for minimap changes, room-time and replay contracts for room-time changes, and the client architecture checker for every client behavior/import change.
- Run the selected browser/client smoke suite against a local server for desktop regression coverage, plus git diff --check.
- Manually test desktop mouse/keyboard/right-click at 1440x900 and 1366x768 before accepting each phase.
- Manually test a phone/tablet through a local Tailscale URL. Desktop touch emulation is not sufficient proof for iPhone Safari or Android Chrome behavior.

## Implementation Process

Implement phases serially. After each phase, the implementing agent must provide a handoff that says what changed, what the next phase should do, what core features to manually test, and whether the desktop-preservation gate passed. Open each phase PR with scripts/agent-pr.sh, arm auto-merge, run scripts/wait-pr.sh with the PR number, and verify the merged phase head is reachable from origin/main before starting the next phase.

After approval, executor passes may use the existing phase runner with explicit phase ids, for example:

    scripts/phase-runner.sh --plan mobileui phase-1 --pr --wait
