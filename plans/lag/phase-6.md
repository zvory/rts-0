# Phase 6 - Unit Intent Surfaces

## Phase Status

- [ ] Planned.

## Objective

Expand provisional owned-world response beyond movement for unit commands. The player should see
owned units accept attack, gather, setup, teardown, and ability intent on the command cadence while
the server remains authoritative for targets, damage, resources, visibility, cooldown outcomes, and
deaths.

## Scope

Enable local intent response for command families only after command-specific tri-state coverage
exists:

- `attack`: owned unit posture, target/order marker, and movement toward currently visible targets
  when owner-safe data is sufficient; no damage, hidden target inference, or kill prediction.
- `gather`: owned worker gather intent, path/posture, and local target marker; no resource income,
  depletion, dropoff, or slot-stealing prediction.
- `setupAntiTankGuns` and `tearDownAntiTankGuns`: owned weapon setup/teardown posture and facing
  intent; no authoritative range, damage, or completed setup side effects before server confirm.
- `useAbility` and `recastAbility`: owner-only targeting marker, windup/posture, and conservative
  cooldown display when safe; no damage, smoke/fog outcome, teleport legality, projectile hit, or
  recast success before authority.
- `setAutocast`: local command acceptance state only; authoritative autocast behavior remains
  server-only.

## Safety Rules

- Prediction may show what the player ordered their owned unit to try.
- Prediction must not show that a hidden enemy exists or does not exist.
- Prediction must not show server validation as successful until command result metadata or
  owner-visible snapshots prove it.
- Rejected and no-op commands must clear provisional intent by `clientSeq`.
- Rolled-back and clamped-rollback commands should reconcile as corrected authority, not as
  mysterious path snapbacks.
- Outside-window late commands must remain legible through clamped rollback or fallback metadata and
  future lead adjustment.

## Expected Touch Points

- `server/crates/sim-wasm/src/lib.rs`
- `client/src/prediction_controller.js`
- `client/src/state.js`
- `client/src/renderer/feedback_view_model.js`
- `client/src/renderer/feedback.js`
- `client/src/minimap.js`
- `client/src/input/`
- `server/crates/sim/src/game/services/order_planner.rs`
- `docs/design/protocol.md` if command result metadata expands
- new tri-state scenarios per command family

## Verification

- Unit tests for each enabled family:
  - accepted on healthy two-tick cadence
  - late arrival inside rollback window converges
  - arrival behind the active replay cursor corrects and clears local intent
  - outside-window late arrival uses clamped rollback when the family is clamp-safe, or corrects via
    fallback and raises lead when appropriate
  - rejected by ownership
  - rejected by invalid target or missing eligibility
  - no-op remains legible and clears local intent
  - prediction-disabled path remains authoritative-only
- Tri-state scenarios for:
  - visible attack target intent
  - hidden attack target correction with no leak
  - gather intent and no local resource gain
  - setup/teardown posture correction
  - ability targeting marker rejection and confirmation
  - coalesced snapshots replaying mixed pending unit intents
- Run:
  - `node tests/prediction_controller.mjs`
  - focused tri-state scenarios for every touched family
  - `node tests/sim_wasm_smoke.mjs` when WASM assets are present
  - `cargo test --manifest-path server/Cargo.toml -p rts-sim-wasm`
  - `node scripts/check-prediction-guardrails.mjs`

## Manual Testing Focus

Under artificial latency, attack, gather, setup, teardown, and ability commands should visibly feel
accepted for owned units without showing false damage, false income, or hidden enemy information.
Invalid commands should clear their provisional state with a legible rejection rather than leaving
stale local intent.

## Handoff Expectations

The handoff must list exactly which unit command families are enabled, which are clamp-safe,
which are exact-rollback-only or live-fallback-only, which remain authoritative-only, the
no-op/rejection signals used, and the tri-state artifacts that should be inspected if later behavior
regresses.
