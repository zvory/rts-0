# Phase 5 - Make Client Command Interaction Explicit

Status: Incomplete.

## Objective

Remove two small but costly client ownership leaks without redesigning the client. Centralize the
duplicated issue-and-record path used by Input, HUD, and Minimap, and remove the mutable Lab control
policy from `GameState` in favor of one narrow read-only policy projection composed by `Match`.
Preserve ordinary play, Lab operator and read-only behavior, spectators/replays, prediction,
command feedback, combat audio, and rendered ownership cues.

## Work

- Add one small command-interaction collaborator that owns the existing sequence of issuing a
  gameplay command through `commandIssuer` and recording the result through `ClientIntent`.
  Preserve command options and the selected-entity snapshot used for planned-order feedback.
- Construct that collaborator in `Match` and inject the same object into Input, HUD, and Minimap.
  Remove their duplicate `_issueCommand`/`issueGameplayCommand` wrappers once every call site uses
  the shared path.
- Replace `state.controlPolicy` with one narrow, read-only policy projection owned by `Match`. The
  projection may expose the ownership and command-surface queries already needed by selection,
  control groups, command budget, HUD/command cards, minimap, renderer feedback/entities, and
  combat audio, but it must not expose mutable Lab controls or the whole `Match`.
- Inject that single projection where ownership semantics are needed. Remove only command-policy
  publication and discovery from `GameState`; preserve every other existing GameState
  responsibility. Inject the same projection explicitly into room-time controls so Lab operator
  access remains enforceable without `state.controlPolicy`. Do not add a set of unrelated callbacks
  to every consumer.
- Keep mutable Lab operator settings out of the read-only projection. Have `App` inject a separate
  narrow LabPanel collaborator for `setIgnoreCommandLimits` and `ignoreCommandLimitsEnabled`, rather
  than letting LabPanel rediscover the mutable policy through `match.state` or receive the whole
  command-capable interaction. Also inject the shared read-only projection into LabPanel for its
  existing target-player `commandUpgrades` query; do not fold that read-only query into the mutable
  settings collaborator.
- Preserve the Lab policy as the authority behind the projection and preserve the ordinary policy
  fallback. Lab issue-as ownership, mixed-owner blocking, ignore-limit behavior, read-only command
  suppression, and the viewer's real `playerId` semantics must remain unchanged.
- Add focused contract coverage for a command issued from each of Input, HUD, and Minimap, proving
  each is sent once and records the same planned-command result once. Cover normal prediction and
  Lab issue-as paths plus passive spectator/replay/read-only blocking. Include a queued/Shift path
  proving command options and queued semantics pass through unchanged.
- Add focused ownership-projection coverage for selection/control groups, command cards, command
  budget, renderer feedback, minimap feedback, and combat-audio categorization. Prefer extending
  the existing client contract files over building a new integration framework. Cover LabPanel's
  target-player completed-research view and room-time operator access through their explicit
  projection dependencies.
- Update `docs/design/client-ui.md` and the client context capsule to describe the shared command
  interaction, read-only policy projection, and the absence of control policy on `GameState`.

## Non-goals

- Do not change wire messages, command shapes, command-supply limits, command-budget decisions, or
  server authority.
- Do not change prediction timing, optimistic overlays, planned-order reconciliation, command
  feedback pixels, or audio classification.
- Do not move Lab transport/panel ownership into client runtime modules or let those modules import
  Lab implementation details.
- Do not introduce a general service container, event bus, policy framework, or a callback per
  policy question.
- Do not refactor unrelated `GameState`, renderer, HUD, input, or minimap behavior.

## Likely Touch Points

- one small client model or app-shell command-interaction module
- `client/src/match.js`
- `client/src/app.js` and `client/src/lab_panel.js` for the narrow mutable LabPanel collaborator
- `client/src/replay_controls.js` for explicit read-only operator-access policy
- `client/src/input/index.js`
- `client/src/hud.js`
- `client/src/minimap.js`
- `client/src/state.js` and the focused ownership consumers currently reading
  `state.controlPolicy`
- existing tests under `tests/client_contracts/`, `tests/minimap_input_contracts.mjs`, and
  `tests/hud_command_card.mjs`
- `tests/client_contracts/lab_contracts.mjs`
- `docs/design/client-ui.md`
- `docs/context/client-ui.md`

## Verification

- Focused client contracts proving Input, HUD, and Minimap share issue-and-record semantics across
  normal, queued, Lab, and passive viewer modes.
- Focused ownership tests proving the read-only projection preserves selection, command-surface,
  feedback, audio, visual ownership decisions, and Lab room-time operator access without
  `GameState.controlPolicy`.
- `node tests/client_contracts.mjs`
- `node tests/minimap_input_contracts.mjs`
- `node tests/hud_command_card.mjs`
- `node scripts/check-client-architecture.mjs`
- `node tests/select-suites.mjs --verify`
- `git diff --check`

## Manual Test Focus

In an ordinary live match, issue one world command, one HUD command, and one minimap command and
confirm prediction, planned-order feedback, sound, and visuals behave as before; make one command
queued with Shift. In Lab, confirm an operator can issue as one selected owner while mixed-owner and
read-only selections remain blocked; toggle ignore-command-limits through the Lab panel, confirm a
non-operator cannot use Lab room-time controls, and spot-check a normal spectator or replay viewer
remains passive.

## Handoff

Mark this phase done in its implementation commit. Report the shared command-interaction owner, the
read-only policy projection surface, every removed `GameState.controlPolicy` dependency, and the
normal/Lab/passive-viewer evidence. Tell the Phase 6 agent that Net subscriber diagnostics are a
separate transport-only change and must not reopen the command or policy seams.
