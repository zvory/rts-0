# Phase 7 - Building, Rally, Queue, and Build Intent

## Phase Status

- [ ] Planned.

## Objective

Make building-facing commands feel accepted on the same command cadence without making the client
authoritative for economy or completion. This phase moves existing train/rally optimism into the
scheduled-command and rollback result model and adds reversible local intent for build, research,
cancel, and queue surfaces.

## Scope

- Rally:
  - keep provisional rally plans, now keyed by effective tick and command result metadata
  - correct to authoritative `rallyPlan` when snapshots arrive or rollback rewrites the result
- Train:
  - keep optimistic queue display
  - tie confirmation, rejection, timeout, rollback, clamped rollback, and late fallback to `clientSeq`
  - do not spawn units or spend resources locally
- Research:
  - show provisional accepted queue/progress intent only after command result metadata or safe
    owner-visible confirmation
  - do not grant upgrades locally
- Cancel:
  - show reversible queue intent where the owner-visible queue makes the target unambiguous
  - server snapshots remain authority for refunds and active item state
- Build:
  - show a local owner-only build intent ghost at the requested footprint on the command cadence
  - never reserve tiles, block pathing, spend resources, unlock tech, add supply, or create an
    authoritative scaffold locally
  - replace the ghost with the authoritative scaffold only after the server confirms it
- Progress:
  - keep already-started production/research extrapolation conservative
  - do not reopen construction progress unless this phase adds a direct owner-only active-building
    signal and the interrupted-construction scenarios pass

## Expected Touch Points

- `client/src/prediction_controller.js`
- `client/src/state.js`
- `client/src/progress_extrapolator.js`
- `client/src/hud.js`
- `client/src/renderer/`
- `client/src/input/placement.js` or related placement modules
- `server/src/lobby/room_task.rs`
- `server/crates/sim/src/game/services/construction.rs`
- `server/crates/sim/src/game/services/production.rs`
- `docs/design/protocol.md`
- `docs/design/client-ui.md`
- `tests/prediction_controller.mjs`
- new and existing train/rally/build/research tri-state scenarios

## Verification

- Unit tests for:
  - train optimism on effective tick and confirmation by snapshot/result metadata
  - train/rally correction after rollback
  - train/rally correction after clamped rollback when the surface is declared clamp-safe, or explicit
    live fallback when it is not
  - train/rally correction when a command arrives behind the active replay cursor
  - rally correction by authoritative `rallyPlan`
  - research intent clears on rejection and never grants upgrade locally
  - cancel intent does not refund locally before authority
  - build ghost appears on cadence and clears on rejection
  - build ghost does not affect pathing, selection, tech, supply, or placement legality
  - prediction disabled clears all provisional building overlays
- Tri-state scenarios for:
  - train under two-tick cadence
  - rally under rollback correction
  - invalid build rejection
  - valid build ghost then authoritative scaffold
  - research rejection and confirmation
  - cancel queue correction
  - construction interruption remains authoritative-only unless explicitly supported
- Run:
  - `node tests/prediction_controller.mjs`
  - focused tri-state building/queue scenarios
  - `node tests/client_contracts.mjs`
  - `node scripts/check-client-architecture.mjs`
  - protocol parity/Rust tests if metadata changes

## Manual Testing Focus

Under latency, building commands should feel accepted through reversible local intent. Check valid
and invalid build, train, research, cancel, and rally commands with Movement prediction on and off,
confirming that resources, supply, spawns, upgrades, and completed buildings only change after
server authority.

## Handoff Expectations

The handoff must list which building surfaces are provisional, which are clamp-safe, which side
effects remain authoritative-only, whether any owner-only metadata was added, and which
construction-related cases remain intentionally unsupported.
