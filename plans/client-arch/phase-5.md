# Phase 5 - Command Card Descriptor Layer

## Phase Status

- [ ] Pending implementation.

## Objective

Prepare `HUD` for safer long-term improvement by separating command-card decision logic from DOM
button rendering. This phase should preserve the visible command card exactly while making command
availability easier to test.

## Work

- Add a pure descriptor layer for command-card buttons, either in:
  - `client/src/hud_command_card.js`, or
  - a small `client/src/hud/` folder if the repo is ready for that split.
- The descriptor layer should return plain objects for the current selection/state:
  - id/action kind
  - label
  - icon
  - hotkey
  - enabled/disabled state
  - tooltip HTML or structured tooltip fields
  - cost/cooldown/count metadata
  - command intent callback or serializable command intent
- Keep the existing DOM renderer in `HUD` initially. It should consume descriptors but continue to
  create the same HTML, CSS classes, titles, hotkeys, repeatability flags, and click behavior.
- Add descriptor-level tests for high-risk command-card cases:
  - worker build menu
  - production building train buttons
  - repeatable train/cancel hotkeys
  - ability targeting buttons
  - upgrade requirements and affordability
  - spectator/replay hidden command card behavior if applicable
- Avoid broad selected-panel or resource-bar changes in this phase.

## Implementation Segments

Mark each segment complete as it lands:

- [ ] Add the pure command-card descriptor layer with plain-object button descriptors.
- [ ] Convert `HUD` command-card rendering to consume descriptors while preserving existing output.
- [ ] Preserve command dispatch, hotkeys, repeatability, enabled/disabled behavior, and tooltips.
- [ ] Add descriptor-level tests for worker build, production, hotkeys, targeting, upgrades, and
  spectator/replay hidden states.
- [ ] Add DOM-property tests for any changed command-card HTML generation.
- [ ] Run verification and record the exact changed command families in the final handoff.

## Verification

- `node tests/client_contracts.mjs`
- `node scripts/check-client-architecture.mjs` if Phase 1 has landed
- Client smoke when practical.
- For any changed command-card HTML generation, add a test that compares important class/data/text
  properties rather than relying on screenshot inspection.

## Manual Test Prompt

At handoff, ask the user to do this command-card pass:

> Manual testing requested, 20-30 minutes:
> 1. Select a worker, open/close the build menu, place a building, and confirm affordability and
>    disabled states look unchanged.
> 2. Select one production building, train units by click and hotkey, then hold a repeatable train
>    hotkey long enough to confirm repeat behavior.
> 3. Select multiple compatible production buildings and confirm training still spreads across them.
> 4. Queue and cancel production, including cancel hotkeys where available.
> 5. Use at least one targeted ability/order button and confirm the targeting cursor and issued
>    command are unchanged.
> 6. Check upgrade-gated buttons before and after requirements are met if the current scenario makes
>    that practical.
> 7. Enter spectator/replay mode if available and confirm command cards stay hidden or inert.
> 8. Report changed labels, missing icons/hotkeys/tooltips, wrong disabled states, misfired
>    commands, or console errors.

## Handoff Expectations

In the final handoff, include the completed segment checklist, exact verification output summary,
and the filled manual testing prompt above. Tell the next agent to start Phase 6 only after this
phase is committed, merged to `main`, and pushed.

## Safety Notes

This is the scariest direct investment because it is close to visible UI and gameplay commands.
Keep the first pass thin: descriptors first, rendering unchanged. Do not redesign the HUD, move
panels, change CSS, or change command dispatch semantics.

If the descriptor layer requires too much churn in `hud.js`, split out only one low-risk command
family first, such as pure formatting helpers or production button descriptors, and leave the rest
for a later phase.

## Outcome

No intentional gameplay or visual change. Future HUD work can add tests against command descriptors
before touching DOM rendering, reducing the chance that agents break player-facing controls.
