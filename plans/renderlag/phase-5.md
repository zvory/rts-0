# Phase 5 - HUD And Observer Dirty Guards

## Phase Status

- [ ] Pending.

## Objective

Remove avoidable DOM and observer-overlay work from the animation frame path. These paths are not the
largest measured costs today, but they rebuild DOM or overlay content while inputs are unchanged and
can add GC pressure during selected-unit, replay, and spectator views.

## Work

- Add dirty signatures, version checks, or bounded update cadence for HUD subpanels that currently
  run every frame without input changes:
  - selected entity panel and command-supply grid;
  - command card if any remaining branch lacks a signature guard;
  - resource rows and control groups only where existing guards are incomplete;
  - alert or status surfaces only if measured as hot.
- Add dirty or cadence guards for observer analysis army-value rendering. It should not rebuild body
  DOM every frame when camera bounds, selected tab, analysis payload, and entity view inputs are
  unchanged.
- Preserve controls and accessibility semantics. Dirty guards must not make button states stale,
  hide newly available commands, delay important alerts, or break keyboard/mouse interactions.
- Keep teardown behavior explicit for any new timers, observers, or cached DOM references.
- Add focused DOM/contract tests where practical and include a manual selected-unit stress check in
  the handoff.

## Expected Touch Points

- `client/src/hud.js`
- `client/src/hud_selection_panel.js`
- `client/src/hud_command_card.js`
- `client/src/observer_analysis_overlay.js`
- `client/src/match.js` only if observer/HUD update scheduling changes
- `tests/client_contracts.mjs`
- command-card or HUD-specific tests selected by touched files
- `docs/design/client-ui.md` if public HUD or observer behavior changes

## Implementation Checklist

- [ ] Identify current DOM rebuild paths with a selected-unit and observer-analysis timing probe.
- [ ] Add dirty signatures or cadence guards for selected HUD paths.
- [ ] Add observer-analysis guards without making replay/spectator data stale.
- [ ] Preserve HUD controls, selected-unit detail, command availability, and teardown.
- [ ] Add focused tests for dirty guards and stale-state avoidance.
- [ ] Run before/after browser perf harness workloads plus a selected-unit manual or scripted probe.
- [ ] Run verification and record exact results.
- [ ] Mark this phase as done in this file.

## Verification

- `node tests/client_contracts.mjs`
- relevant HUD or command-card tests selected by `node tests/select-suites.mjs --from=origin/main`
- `node scripts/check-client-architecture.mjs`
- `node scripts/client-perf-harness.mjs --workload matt-alex-replay --seconds 10`
- `node scripts/client-perf-harness.mjs --workload vehicle-wall-stress --seconds 10`
- `git diff --check`

If client design docs change, also run:

```bash
node scripts/check-docs-health.mjs
```

## Manual Test Focus

Select one unit, multiple units, damaged units, producers, workers, and empty ground in a local match.
Confirm the selected panel, command card, resources, control groups, alerts, observer analysis tabs,
collapse state, and replay/spectator overlays update promptly and do not flicker or go stale.

## Handoff Expectations

List each guarded DOM/overlay path, the signature or cadence used, and the measured DOM/timing
effect. State any remaining per-frame DOM work that is intentional and why it should not block
Phase 6.
