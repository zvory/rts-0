# Phase 3 - Integration, Documentation, And Playtest Hardening

## Phase Status

Status: pending.

## Objective

Close the loop between the server queue semantics and client planning UX. This phase should ensure
the full player workflow is documented, tested across mixed/edge cases, and ready for playtesting
without hidden mismatches between client preview and server execution.

## Scope

- Verify the full `move -> setup -> pointFire` flow through owner-only snapshots and command-card
  affordances after phases 1 and 2 have both landed.
- Add or adjust integration/regression coverage for:
  - accepted order-plan markers for queued setup and queued Point Fire,
  - client frozen cones reconciling with server `orderPlan`,
  - stale preview cleanup after server rejection or queue clear,
  - terminal Point Fire preventing later queued stages,
  - mixed selections where only artillery receives Point Fire.
- Update `docs/design/server-sim.md`, `docs/design/client-ui.md`, and `docs/design/protocol.md` so
  they agree on:
  - queued setup as a future emplacement/facing intent,
  - queued Point Fire as legal only after a same-artillery setup predecessor or current legal
    Point Fire state,
  - frozen client cones as local feedback rather than authority,
  - final execution checks at deployment/fire time.
- Add patch-note bullets for the player-facing behavior and any caveats worth watching in playtests.
- Prefer focused verification over broad local suites; rely on the owned PR gate for the full
  `./tests/run-all.sh` check.

## Expected Touch Points

- `docs/design/server-sim.md`
- `docs/design/client-ui.md`
- `docs/design/protocol.md`
- focused Rust and JS tests added in phases 1 and 2
- `tests/tri_state/scenarios/queued_move_order_stages_survive_replay.mjs` or a nearby queued-order
  scenario only if an existing scenario is the right fit
- `tests/client_contracts/*` only for gaps discovered during integration
- optional dev scenario or replay artifact docs if manual playtest setup needs a stable entry point

## Edge Cases To Cover

- A queued Point Fire rejected by final range/ammo/deployment checks does not leave frozen client
  feedback behind indefinitely.
- A queued setup followed by ordinary attack-move still behaves as before for support weapons.
- Anti-Tank Guns still use setup previews but do not gain Point Fire targeting.
- A player can queue a second setup after the first frozen setup, and the preview/order-plan state
  stays understandable.
- Replays and reconnect snapshots display accepted `orderPlan` markers without requiring the
  original local frozen preview state.

## Verification

- Focused Rust tests for any remaining queue/promotion edge cases.
- Focused JS client contract tests for preview reconciliation and cleanup.
- `node tests/protocol_parity.mjs` if protocol docs, compact metadata, or projection vocabulary are
  touched.
- `node scripts/check-client-architecture.mjs` if client modules changed in this phase.
- `node scripts/check-docs-health.mjs`.
- `git diff --check`.

## Manual Test Focus

Play through the core artillery plan in a local match: queue move, queue setup, confirm frozen cones,
queue Point Fire inside the planned cone, wait for movement/deployment, and observe firing. Repeat
with Stop before deployment, with an out-of-range target, and with a mixed artillery plus rifleman
selection.

## Handoff Expectations

Summarize the final player-facing behavior, the exact tests run, and the manual scenarios covered.
Include patch-note bullets and any residual playtest risks, especially cases where client preview
can be invalidated by later movement, queue changes, or economy state before the shell fires.
