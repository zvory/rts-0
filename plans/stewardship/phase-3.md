# Phase 3 - Client command and session seams

## Status

- [ ] Incomplete.

## Objective

Give live command surfaces one explicit owner for command policy, issuance, and local feedback, then
make match startup failures cleanly recoverable. Preserve current gameplay, Lab issue-as behavior,
prediction, visuals, and subscriber isolation while removing hidden discovery through `GameState`.

## Work themes

### One command interaction seam

- Have `Match` compose a narrow command interaction/context and inject it into Input, HUD, Minimap,
  and the small number of feedback/audio ownership consumers that need the same policy answers.
- Centralize gameplay command issuance and associated local planned-order/feedback recording so
  viewport, command-card, and minimap paths do not each maintain an issue-and-record variant.
- Remove `state.controlPolicy` assignment, fallback reads, and duplicate command/policy helper paths.
  Keep authoritative snapshot data in `GameState` and browser-local intent in `ClientIntent`.
- Preserve ordinary players, spectators, read-only Lab viewers, Lab operators, command budgets,
  prediction sequencing, queued commands, and owner-relative feedback. Add a small architecture or
  contract guard that prevents the hidden state path from returning.

The exact collaborator shape is an implementation choice. Prefer one small explicit object over a
general service locator, DI container, or compatibility layer.

### Rollback-safe session assembly

- Make `Match` construction transactional enough that a failure after allocating listeners, timers,
  audio helpers, DOM, or Pixi resources unwinds everything already created and leaves App able to
  return to a usable lobby or start another match.
- Keep teardown idempotent and safe for partially initialized sessions. Do not broadly split
  `Match`, Input, Renderer, or other large client modules as part of this phase.
- Keep `Net` subscribers isolated, but report subscriber exceptions through a bounded diagnostic or
  logging seam with message type and useful context. Reporting must not recurse into dispatch,
  alter the wire protocol, or prevent later subscribers from running.

### Proportionate contracts

- Cover ordinary and Lab command paths through the shared interaction, including local planned
  feedback being recorded once.
- Add failure-injection coverage for a late Match-construction failure, complete cleanup, and a
  successful subsequent session.
- Verify that a throwing Net subscriber is observable while later subscribers still receive the
  event. Keep tests at module boundaries; do not build an exhaustive lifecycle framework.

## Likely touch points

- `client/src/match.js`, `client/src/app.js`, and `client/src/net.js`
- A small new command-interaction and/or construction-cleanup helper under `client/src/`
- `client/src/input/`, `client/src/hud.js`, and `client/src/minimap.js`
- Relevant renderer feedback-ownership and combat-audio helpers
- Focused files under `tests/client_contracts/`, plus the client architecture checker if it owns the
  regression guard
- `docs/design/client-ui.md` and its capsule only where the documented module contract changes

## Verification

- `node scripts/check-client-architecture.mjs`
- `node tests/client_contracts.mjs`
- `node tests/minimap_input_contracts.mjs`
- `node tests/hud_command_card.mjs`
- `node tests/select-suites.mjs --verify`
- Run any additional focused suite selected for the final touched files; GitHub's `Main test gate`
  remains the authoritative full validation.

## Manual testing focus

- In a normal match, issue representative move/attack/build or ability commands from the viewport,
  HUD, and minimap, including one queued command, and confirm previews do not duplicate or linger.
- In Lab, confirm an operator can issue as the selected owner while a read-only viewer remains
  passive and owner-relative visual/audio feedback is unchanged.
- Return to the lobby and start another match to confirm teardown and rematch behavior remain clean.

## Handoff expectations

Mark this phase done in its implementation commit. Report the final command-interaction boundary,
the hidden/duplicate paths removed, rollback and subscriber-error behavior, focused verification,
and the three manual checks above. Push the phase as an owned PR with auto-merge armed, wait for a
definite merge, and verify the phase head is reachable from `origin/main` before handing off the
next phase.
