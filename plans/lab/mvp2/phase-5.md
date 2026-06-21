# Phase 5 - Hardening, Smoke, and Docs

## Phase Status

- [ ] Planned.

## Objective

Harden the MVP2 lab interaction model, update source-of-truth docs, and verify the end-to-end
operator workflow before closing the plan.

## Work

- Tighten active-tool visual state, cancellation text, disabled states, and result/error status in
  the lab panel without turning this into a broad redesign.
- Add or refresh focused tests around command-card lab behavior, click-to-spawn, selected setup
  tools, catalog filtering, teardown, and normal-match non-regression.
- Run a browser smoke path for the two primary user stories: command-card orders in lab and
  palette click-to-spawn.
- Update `docs/design/client-ui.md` for the stable lab collaborator, command-surface, and lab tool
  intent contracts.
- Update `docs/design/protocol.md` or `docs/design/server-sim.md` only if earlier phases changed
  those contracts.
- Refresh context capsule section pointers if design-doc structure shifts.
- Record remaining gaps for future plans, especially timeline controls, multi-operator semantics,
  persistent scenarios, lab flags, and `/dev/scenario` migration.

## Expected Touch Points

- `client/src/lab_panel.js`
- `client/src/match.js`
- `client/src/client_intent.js`
- `client/styles.css`
- `tests/client_contracts.mjs`
- `tests/hud_command_card.mjs`
- `scripts/check-client-architecture.mjs`
- `scripts/check-faction-catalog-parity.mjs` if palette catalogs changed
- `docs/design/client-ui.md`
- `docs/design/protocol.md` if protocol changed
- `docs/design/server-sim.md` if public `Game` lab APIs changed
- `docs/context/client-ui.md`
- `docs/context/protocol.md` or `docs/context/server-sim.md` only if their section lists changed
- `plans/lab/mvp2/*.md`

## Implementation Checklist

- [ ] Polish active-tool state and cancellation affordances.
- [ ] Verify lab teardown does not leak panel, input, or intent state across rematches.
- [ ] Add or update focused automated coverage for the MVP2 workflows.
- [ ] Run the relevant focused verification commands.
- [ ] Perform a manual browser smoke of lab command-card ordering and palette click-to-spawn.
- [ ] Update design docs and context capsule pointers for changed contracts.
- [ ] Mark completed phase documents done as appropriate.
- [ ] Record remaining non-MVP2 gaps without expanding this plan's scope.

## Verification

- `node tests/client_contracts.mjs`
- `node tests/hud_command_card.mjs`
- `node scripts/check-client-architecture.mjs`
- `git diff --check`

If catalog exports changed:

- `node scripts/check-faction-catalog-parity.mjs`

If protocol changed in any earlier phase:

- `node tests/protocol_parity.mjs`

If a live browser smoke is automated or selected by the test selector:

- `node tests/select-suites.mjs --verify`

## Manual Test Focus

Start a lab, spawn units through the faction palette by clicking the map, select those units, issue
move/stop or another valid command-card order, switch vision, export the scenario, import it again,
and confirm a normal match or spectator path still does not expose lab tools.

## Handoff Expectations

Summarize the final MVP2 interaction contract in plain language, list exact verification and manual
smoke results, and name remaining lab gaps for follow-up planning. If any phase left a temporary
advanced form or fallback in place, state why it remains and what future plan should remove it.
