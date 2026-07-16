# Phase 8 - Make Client Command Interaction Explicit

Status: Incomplete.

## Objective

Give command-producing client surfaces one explicit owner for policy-aware issuance and local
planned feedback. Remove hidden command-policy discovery through `GameState` without granting
command authority to rendering, audio, selection, or replay consumers that only need read-only
answers.

## Work

- Have `Match` compose a narrow command interaction for Input, HUD, and Minimap. Its central
  operation should issue a gameplay command and record the selected-unit planned feedback exactly
  once.
- Preserve command budget checks, Lab issue-as routing, prediction sequencing, paused-prediction
  options, health/profiler accounting, command results, and local overlay updates in that path.
- Remove the duplicate issue-and-record helpers from Input, HUD, and Minimap.
- Inject explicit read-only control or feedback policy callbacks into selection, control-group,
  command-budget, replay-control, renderer, and combat-audio consumers. Do not pass those consumers
  the command-capable interaction merely because they need related policy answers.
- Remove `state.controlPolicy` assignment and every production fallback/read through `GameState`.
  Keep authoritative snapshot data in `GameState` and browser-local intent in `ClientIntent`.
- Add a zero-tolerance architecture or contract guard that rejects publication or discovery of
  command policy through `GameState`; do not introduce a transitional baseline.
- Preserve ordinary players, spectators, read-only Lab viewers, Lab operators, queued commands,
  owner-relative feedback, visuals, and audio.

## Non-goals

- Do not change match startup/rollback or `Net` subscriber diagnostics; Phases 9 and 10 own those
  jobs.
- Do not introduce a service locator, DI container, general command bus, or compatibility shim.
- Do not broadly split Match, Input, HUD, Minimap, Renderer, or GameState.

## Likely Touch Points

- `client/src/match.js`
- a small command-interaction helper under `client/src/`
- `client/src/input/`, `client/src/hud.js`, and `client/src/minimap.js`
- read-only policy consumers in command budget, control groups, replay controls, renderer feedback,
  entities, and combat audio
- focused files under `tests/client_contracts/`
- `scripts/check-client-architecture.mjs`
- `docs/design/client-ui.md` and its capsule where the module contract changes

## Verification

- Focused contracts proving ordinary and Lab command paths record local planned feedback once.
- Focused contracts proving read-only consumers receive policy answers without a command-issuing
  collaborator.
- A negative architecture fixture proving `GameState.controlPolicy` publication or discovery fails.
- `node scripts/check-client-architecture.mjs`
- `node tests/client_contracts.mjs`
- `node tests/minimap_input_contracts.mjs`
- `node tests/hud_command_card.mjs`
- `git diff --check`

## Manual Test Focus

In a normal match, issue representative move, attack, build, and ability commands from the viewport,
HUD, and Minimap, including one queued command. In Lab, confirm an operator can issue as the selected
owner while a read-only viewer remains passive and previews/audio remain owner-relative.

## Handoff

Mark this phase done in its implementation commit. Report the command-capable boundary, read-only
policy boundary, hidden paths removed, and once-only feedback evidence. Tell the Phase 9 agent not to
reshape command interaction while making construction and App assembly rollback-safe.
