# Phase 4 - Movement Service Decomposition

Goal: split `server/src/game/services/movement.rs` and then reassess
`server/src/game/services/move_coordinator.rs` so movement behavior is easier to maintain.

## Target Components

- `services/movement/mod.rs`: `movement_system`, shared constants, and module-level orchestration.
- `services/movement/waypoints.rs`: waypoint arrival, path advancement, final/intermediate
  waypoint handling, and order completion hooks.
- `services/movement/tank_drive.rs`: tank body facing, reverse/crawl/pivot behavior, traffic
  adjustment, and tank fuel movement gating.
- `services/movement/steering.rs`: local steering direction, sidestep injection, and nearby-unit
  steering helpers.
- `services/movement/collision.rs`: broad-phase pair collection, axis/depth resolution, push share
  logic, and collision passes.
- `services/movement/standability.rs`: movement-local static standability wrappers if they remain
  distinct from `services/standability.rs`.
- `services/movement/tests.rs`: behavior tests grouped by waypoint, tank, steering, and collision.

After `movement.rs` is split, evaluate `move_coordinator.rs` for a follow-up split:

- `move_coordinator/requests.rs`: request intake, cap enforcement, and scheduling.
- `move_coordinator/formations.rs`: spread goals, formation offsets, facing, and unique tile search.
- `move_coordinator/staging.rs`: current/build staging goals and material goal refresh.
- `move_coordinator/path_assignment.rs`: path creation and entity path mutation.

Implementation note: `move_coordinator.rs` was left as the follow-up boundary for this phase. The
movement split did not require coordinator internals to become public, and the request/formations/
staging/path-assignment seams above remain the right next decomposition cut.

## Design Notes

Keep `movement_system` as the only system entry point called by `systems.rs`. Extraction should not
make combat, economy, or construction call movement internals directly.

Be especially conservative around tanks. Tank movement currently combines terrain, facing, path
lookahead, traffic, fuel, and collision behavior; preserve deterministic order and floating-point
calculations during mechanical moves.

## Tests

- Run `cargo test` in `server/`.
- Run self-play tests when changing anything beyond mechanical moves.
- Watch for tests that become too broad after extraction; split them by behavior where useful.

## Done

- `movement_system` is short enough to show the tick flow.
- Tank drive, steering, collision, and waypoint behavior are independently testable.
- `move_coordinator.rs` has a clear follow-up boundary or has been split under the same principles.
