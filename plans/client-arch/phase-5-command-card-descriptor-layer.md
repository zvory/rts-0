# Phase 5 - Command Card Descriptor Layer

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

## Verification

- `node tests/client_contracts.mjs`
- `node scripts/check-client-architecture.mjs` if Phase 1 has landed
- Client smoke when practical.
- For any changed command-card HTML generation, add a test that compares important class/data/text
  properties rather than relying on screenshot inspection.

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
