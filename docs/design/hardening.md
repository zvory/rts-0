## 7. Hardening (input is untrusted)
The server treats every client as potentially hostile. Limits live next to the code:
- **WebSocket frame cap** (`main.rs`): `max_message_size`/`max_frame_size` = 256 KiB. Oversized
  frames are rejected and the connection closed before they reach serde.
- **Command unit cap** (`services/commands.rs` `MAX_UNITS_PER_COMMAND = 256`): unit-list commands are
  deduped and capped before per-unit work, so a repeated/huge id list can't trigger an A* storm.
- **Queued order caps** (`entity/order.rs` `MAX_QUEUED_ORDERS = 8`): each mobile unit stores at most
  eight future intents. Queued command application still runs the unit-list dedupe/cap first, and
  queued promotion drains invalid stale intents instead of retrying them forever.
  Phase 6 kept this cap at eight because no playtest evidence in the repo justified a larger
  command buffer; mixed ability/setup replay coverage now guards the current cap and command-log
  shapes.
- **Building rally cap** (`services/commands.rs` `MAX_RALLY_STAGES = 4`): each production building
  stores at most four move/attack-move rally stages. Non-queued `setRally` replaces the plan; queued
  `setRally` appends until the cap and ignores further stages.
- **Bounds-checked placement** (`services/occupancy.rs` `footprint_tiles`): tile math uses `checked_add` and
  out-of-range build coords are rejected — the tick loop never panics on adversarial input.
- **Body-aware construction placement**: `services::standability::building_site_clear` is the
  final scaffold policy. A building footprint rectangle must be in-bounds, passable, clear of
  existing building rectangles/resource bodies, and clear of every living unit circle. Build
  command intent uses the paired build-intent predicate, which ignores only the chosen builder's
  own body so a worker can be ordered to build over its current position and walk out first.
  `construction_system` repeats the build-intent unit-body policy at arrival before creating the
  scaffold, so every other living unit still blocks the site but the chosen builder can start the
  scaffold and become a ghost active builder.
  The client placement ghost mirrors the intent policy for the first selected worker, but remains
  advisory; the server is authoritative.
- **Idle timeout + heartbeat**: the server drops connections idle for `IDLE_TIMEOUT = 40s`
  (`main.rs`); the client pings every 15s (`main.js`). This evicts half-open/stuck clients so a
  silent player can't wedge a shared room, and frees the room slot.
- **Join ack**: `RoomEvent::Join` carries a `oneshot<bool>`; a connection only marks itself joined
  on an accept, so a rejected mid-match join doesn't wedge the socket.
- **Replay artifact and seek limits**: production `ReplaySession` construction rejects malformed or
  oversized artifacts before building a game: empty/duplicate/too-many players, duration over one
  hour at 30 Hz, more than 200k command-log entries, commands for unknown players, tick 0 commands,
  out-of-order commands, and commands after the artifact duration. Accepted replay seeks are
  rate-limited to one every 500 ms per replay room. Long seeks are allowed because long games are a
  normal replay-analysis case; accepted seeks restore the nearest recorded replay keyframe at or
  before the target tick and fast-forward from there. Replay setup and accepted seeks log
  build/rebuild duration, viewer/player counts, duration, and command counts so long artifacts and
  expensive controls are visible in server logs.
- **Deploy drain**: SIGTERM/Ctrl-C starts a server drain instead of immediately shutting down.
  The lobby flips into a draining state, existing room tasks continue ticking active normal
  matches, and new match starts are rejected while lobby clients see `can_start: false`. The
  process waits until all tracked normal matches finish or `DEPLOY_DRAIN_TIMEOUT` (10 minutes)
  elapses, whichever comes first, then asks all WebSocket connection tasks to close so Axum
  graceful shutdown can complete; Fly's `kill_timeout` is set to the same 10-minute ceiling. Dev
  self-play/replay/scenario rooms are not tracked as deploy blockers because they can intentionally
  run or auto-restart forever.
- **Fog is authoritative**: `snapshot_for` and per-recipient event delivery go through
  `rules::projection`, which gates entity views, `target_id` tracers, and death/attack events on
  visibility. Hidden enemies are never sent except inside explicit one-second lingering death
  vision, where entity views are marked `visionOnly` and remain non-actionable. Visibility is
  terrain-aware: stone blocks sight beyond itself on both the server fog grid and the client
  cosmetic fog overlay.
- **Shot blocking and overpenetration**: ranged attacks first resolve against the first enemy tank
  body or building footprint intersecting the line from attacker to intended target. That blocker
  takes the shot damage and the intended target behind it is unharmed. Shots that hit ordinary
  units still overpenetrate past the primary target, but any tank body or building footprint hit
  by that carry-through damage absorbs the shot and stops further overpenetration. Stone blocks
  target acquisition and primary fire.
- **Tank body and weapon facing**: the snapshot `facing` field is the tank hull/body angle. Tanks
  rotate that body angle at a bounded rate (`TANK_BODY_TURN_RATE_RAD_PER_TICK = 0.035`) on
  movement paths; badly misaligned tanks pivot in place instead of sliding sideways at full speed.
  The current locomotion model is stateless per tick: it does not store velocity or acceleration,
  but it does brake by scaling the tick movement budget for hull misalignment, frontal traffic, and
  oil starvation. Tank hull movement intent uses the shared oriented-vehicle route lookahead:
  intermediate waypoints can be skipped only when the vehicle's swept static body can legally reach
  the next route segment from the current position. Tanks use a 5-tile lookahead on that legal
  route segment; local steering and collision displacement do not become hull intent. Static
  terrain/building legality uses the oriented `50.4px` by `28.8px` hull plus `1.5px` clearance
  rather than the conservative circular radius, so a lengthwise tank still fits through a
  2-tile-wide straight corridor while front/rear and side clearance near blockers match the hull
  shape.
  A tank reverses toward a nearby goal within 3 tiles when that goal is more than 90 degrees behind
  the hull; farther behind goals make it pivot first. Alignment error at or below `0.55` radians
  keeps full drive speed, error at or above `1.25` radians pivots with no translation, and values
  between those thresholds linearly reduce throttle. If a proposed tank rotation is blocked by
  static terrain or a building while the current hull orientation remains legal, the tank probes one
  speed-step forward and backward along its current hull axis and takes the candidate that makes the
  rotated hull legal, preferring the candidate nearer the active route point. Frontal traffic within
  2 tiles can reduce throttle and add a bounded `0.28` radian turn bias toward open space, but does
  not inject a perpendicular sidestep waypoint.
  The snapshot `weaponFacing` field is the independent turret/barrel angle. Tank combat rotates the
  turret toward the target at a bounded rate and fires only once the turret is within tolerance; the
  hull does not need to face the target. Tanks do not clear their movement path when they fire, so
  they can continue driving while the turret tracks and shoots on both `Move` and `AttackMove`
  orders. A tank on plain `Move` only opportunistically fires at enemies already in range; it does
  not chase out-of-range enemies. Shoot-while-moving units retain their current valid target before
  falling back to nearest-target acquisition, so drive-by fire tends to finish one enemy instead of
  spreading damage across every passing unit. Projection omits enemy `weaponFacing` when it would
  reveal a hidden target direction.
- **Rifleman Methamphetamines fire**: upgraded riflemen are permanently charging and keep their
  movement path while firing at enemies in range instead of stopping to shoot. While on a plain
  `Move`, upgraded riflemen only fire opportunistically at enemies already in range and do not
  chase. Moving charge shots use normal rifleman accuracy and do not add a movement miss roll.
- **Scout car movement and weapon facing**: scout cars are light unarmored vehicles with a
  rear-mounted machine gun (higher damage, same range and cooldown as machine gunners). They use the
  same oriented-body/pathing/collision model as tanks, including standoff firing and firing while
  moving, but they use simplified car locomotion instead of tank pivot locomotion. A scout car's
  yaw is capped by movement budget over a 1.5-tile minimum turn radius, so it can steer
  while translating but cannot rotate in place when blocked or badly misaligned. Reverse is a
  bounded maneuver latched to the immediate waypoint: nearby final waypoints and injected recovery
  waypoints can be reached by backing up, but route lookahead alone cannot put the car into reverse.
  Farther behind goals make the scout car drive through a broad forward turn instead of
  backtracking. Scout cars, tanks, and AT teams use the same clearance-aware player-move route
  shape: coarse A* still works on tiles, but vehicles add static-clearance, turn,
  adjacent-blocker, and corner-graze costs so open alternatives are preferred before local movement
  gets close to walls. Tank-style pivot vehicles (tanks and AT teams) expand each diagonal A* tile
  step into an L-shaped pair of orthogonal tile-center waypoints, choosing the lower-clearance-cost
  elbow when both elbows are passable. This makes pivoting vehicles clear corners before retargeting
  the next leg instead of stopping near an inside corner and immediately rotating toward a diagonal
  segment. The clearance
  cost is finite, so intended narrow passages remain traversable; exact interaction paths such as
  chase, gather, and build staging keep tile-guided `Normal` routing. Oriented vehicles follow the
  route corridor rather than exact intermediate waypoint centers: an intermediate waypoint is
  consumed inside
  `VEHICLE_WAYPOINT_ACCEPTANCE_RADIUS_PX` (0.75 tiles), after the vehicle has passed the waypoint
  along the next route segment, or when the next route segment is statically reachable by the
  vehicle's swept oriented body from its current legal body position. Scout-car drive intent uses a
  3-tile lookahead on the current statically legal route segment, so a car that comes alongside the
  route can continue to a drivable point ahead instead of oscillating around a point it cannot
  laterally reach; tanks and AT teams use a 5-tile lookahead with pivot-drive locomotion. The
  lookahead never skips through terrain or building occupancy that fails oriented-body segment
  legality. A final
  move waypoint can settle inside `SCOUT_CAR_FINAL_GOAL_TOLERANCE_PX` (0.375 tiles) only when the
  remaining error is small and mostly lateral to the car's feasible travel direction; ordinary exact
  arrival still snaps to the ordered point when the car can actually reach it. Scout-car movement
  must never accept a rotated or translated oriented body that is statically illegal against terrain
  or building occupancy, and blocked cars preserve the player's movement order so bounded recovery
  behavior can continue from the same command. When a scout car on `Move` or `AttackMove` remains
  stuck far from its `path_goal`, is still in a legal oriented body position, and its recovery
  cooldown has elapsed, movement searches backward along the current hull axis for a legal reverse
  waypoint up to `SCOUT_CAR_REVERSE_RECOVERY_DISTANCE_PX` (2 tiles). The candidate must be finite,
  in bounds, statically standable at the current facing, and connected by a statically standable
  segment. The waypoint is pushed into the existing reverse-ordered path so the car backs away and
  then resumes the original route; `SCOUT_CAR_RECOVERY_COOLDOWN_TICKS` bounds duplicate injection.
  Behind-the-car intermediate waypoints must be physically reached instead of pass-by consumed, so
  reverse recovery cannot disappear on the same tick it is added. Recovery does not add network
  fields, issue player-visible commands, add infantry sidesteps, or make scout cars pivot in place.
  This is still a path-following approximation, not tire or Ackermann steering physics; replace it
  with proper truck/wheeled movement semantics when that model exists.
  Scout cars do not use tank armor or tank damage reduction.
- **Vehicle movement oil burn**: tanks and scout cars consume oil based on distance actually moved,
  using `TANK_OIL_COST_PER_PX` and `SCOUT_CAR_OIL_COST_PER_PX` respectively. Fractional movement
  cost accumulates per vehicle until whole oil units are deducted from the owner's stockpile. Tanks
  also track lifetime movement oil as `oilUsed` for the client selected-entity panel. If the owner
  has zero oil at the start of a movement tick, that vehicle does not advance and waits
  `TANK_OIL_STARVED_PAUSE_TICKS` (one second) before retrying, so sparse oil income does not
  produce constant one-tick stuttering. Turret/combat behavior still runs through the combat system
  while movement is paused.
- **Methamphetamines research**: Training Centres can queue one permanent player upgrade costing
  100 steel / 100 oil and taking 600 ticks. Once completed, all current and future owned riflemen
  use the charging movement/fire model permanently, move at tank speed, and attack 25% faster.
  Legacy `charge` commands remain decodable but have no eligible carriers.
- **Stage-two unit unlock research**: R&D Complex can queue `at_gun_unlock` for 200 steel / 75 oil
  over 600 ticks, unlocking AT Gun training at Gun Works for that player. R&D Complex can queue
  `tank_unlock` for 150 steel / 100 oil over 600 ticks, unlocking Tank training at Vehicle Works
  for that player. Server-side train validation checks both the producer kind and the completed
  player upgrade set, so clients cannot bypass these locks by sending `train` commands directly.
- **Tank armor facing**: tank and AT-team attacks against tank victims use the victim tank's hull
  `facing` and the attacker's position. Front hits (`<=45°` from the hull direction) deal normal
  damage, side hits (`>45°` and `<=135°`) deal `1.25x`, and rear hits (`>135°`) deal `1.75x`.
  Infantry damage, building damage, non-tank victims, and non-anti-tank attackers ignore armor
  facing.
- **Worker direct-hit retreat**: combat stamps `last_damage_pos`/`last_damage_tick` on every
  damaged entity but never mutates orders or paths. `Game::worker_retreat_commands_for(player)`
  projects that private metadata into ordinary AI-owned worker `Move` commands for workers hit on
  the previous tick. The room task passes those commands through `rts-ai`, then enqueues them via
  the normal command path. Constructing workers stay latched, and human players receive no
  automatic retreat.
- **Tolerant arrival**: a unit on a `Move` or `AttackMove` order in `MovePhase::Moving` that has not
  moved more than `STUCK_EPS_PX` per tick for `STUCK_ARRIVAL_TICKS` consecutive ticks (~0.5 s at
  30 Hz) and is within `TOLERANT_ARRIVAL_RADIUS_PX` (2 tiles) of its `path_goal` is immediately
  marked `Arrived` and halted. This dissolves the stuck-blob pattern where multiple units ordered
  to the same tile fight each other for the last position. The two per-unit state fields
  (`stuck_ticks: u16`, `last_progress_pos: (f32, f32)`) live in `MovementState` and are reset
  whenever a fresh order is issued.
- **Static-obstacle repath**: if a unit on a `Move` or `AttackMove` order repeatedly fails to take
  its next path step because terrain/building occupancy blocks the landing tile, movement debounces
  the failure for `STATIC_BLOCKED_REPATH_TICKS` (~1 s at 30 Hz), clears the stale path, and marks the
  unit `AwaitingPath`. The existing path coordinator then recomputes under current occupancy within
  the normal per-tick A* budget. This covers buildings constructed after a long path was assigned
  without periodically repathing every moving unit.
- **Route waypoint skipping**: intermediate movement waypoints are path-following hints, not hard
  stop points, when the unit's own static swept body can legally reach the following route segment
  from its current position. Infantry and workers use the same standability segment check as
  vehicles, with their circular bodies, so they can skip reachable dog-leg points instead of
  oscillating around the waypoint center. Blocked corners still keep the intermediate waypoint until
  the swept segment is legal. Oriented vehicles additionally keep their facing-specific guard so a
  waypoint behind the hull, including scout-car reverse recovery, must be physically reached.
- **Vehicle clearance pathing**: player-issued scout-car, tank, and AT-team `Move` / `AttackMove`
  path requests use the shared clearance-aware tile A* route shape. The route shape adds finite
  static-clearance, turn, adjacent-blocker, and corner-graze costs so open alternatives are
  preferred before local movement gets close to walls. Path cache keys include the route-shaping
  mode so clearance-shaped movement paths do not share cached routes with normal interaction
  pathing. The returned route is still used for reachability, then snaps the reverse-ordered final
  waypoint to the exact command goal. Scout cars simplify straight segments up to their route
  lookahead window; tanks and AT teams keep their L-expanded tile-center route so corner-clearing
  waypoints are not collapsed back into diagonal shortcuts. Interaction paths for attack chasing,
  gathering, and build staging remain tile-guided `Normal` routing so combat, mining, construction
  range checks, and infantry/worker traffic stay controlled by their existing logic.
- **Vehicle diagonal-pinch avoidance**: A* passability for oriented-vehicle bodies (tanks, scout
  cars, AT teams) rejects tiles wedged between two diagonally-opposite blocked corners — i.e. (NW
  blocked AND SE blocked) OR (NE blocked AND SW blocked). The rotating hull cannot legally thread
  such 1-tile gaps at any intermediate heading, so routing through them used to deadlock at the
  static-blocked-repath threshold. Infantry pathing is unaffected; legitimate 2-tile-wide corridors
  always leave at least one diagonal of each pair open and remain traversable.
- **Formation goal legality**: group move goals keep the existing distance-sensitive formation
  behavior, but candidate tiles are accepted only when the specific unit kind can stand there under
  `standability::unit_static_standable`. This prevents large units from being assigned a center tile
  whose body would clip terrain or a building footprint; dynamic unit traffic is still handled by
  steering and collision after movement. AT-team group moves use the same deterministic candidate
  search but first prefer goal tiles with one open tile between assigned AT teams, falling back to
  ordinary unique tiles when terrain or buildings leave no spaced slot.
- **Local steering**: before taking a partial path step for a plain `Move` order, non-vehicle movement
  computes a short-range separation proposal away from nearby firm/braced/heavy mobile units.
  Neighbor ids are sorted and capped so replay behavior stays deterministic, and separation uses the
  same footing profiles as hard collision so braced/heavy units exert stronger local pressure than
  firm units. The steered landing is only accepted if `standability::unit_static_standable` says the
  unit body fits there; otherwise movement falls back to the ordinary path step / wall-slide logic.
  Tanks do not receive perpendicular steering waypoints or sidestep injections: frontal traffic
  instead reduces throttle and biases the bounded hull turn toward reachable open space. Steering
  does not reserve space or replace collision.
- **Production spawn legality**: production completes in two steps. The front queue item advances
  to complete, then the producer searches deterministic rings around its actual footprint for a
  `standability::unit_spawn_standable` point. Spawn candidates must fit the unit body inside world
  bounds without clipping terrain, any building footprint, or any living unit body, including ghost
  workers. If every candidate is blocked, the complete queue item stays in place and retries on
  later ticks; cost and supply remain reserved from enqueue time. When the producer has a rally
  plan set, the search picks the closest standable candidate to the first stage within the first
  ring that has any (so units exit the rally-facing side), and the new unit is immediately given
  the first rally stage as its active move/attack-move order plus later stages as queued orders;
  with no rally plan the legacy first-found candidate is used and the unit spawns idle.
- **Unit collision**: `services::movement::resolve_collisions` runs after production each tick and
  pair-wise pushes overlapping mobile units apart using `services::geometry::unit_body_overlap`.
  Infantry resolve as circles while tanks resolve from their oriented hulls, so a tank front/back or
  side contact separates on the actual hull axis instead of the center-to-center circle direction.
  Workers in `GatherPhase::Harvesting` or `BuildPhase::Constructing` are ghost pass-through units:
  they neither push nor are pushed, which keeps walking units from being deadlocked by miners or
  active builders. All other mobile-unit pairs split overlap by footing resistance, so braced or
  deployed support weapons and tanks hold ground better than soft moving infantry while equal-profile
  units still split pushes evenly. Moving tanks therefore displace idle soft infantry more readily,
  braced weapons hold ground, and tank-vs-tank contacts tend to stop or reverse along the hull axis
  rather than slide sideways past each other. Push targets are accepted only when the same
  standability layer says the resulting body position is legal; blocked pushes are skipped or
  absorbed by the other side. `Game::assert_invariants` then asserts that no two non-ghost mobile
  unit bodies overlap by more than `OVERLAP_TOLERANCE_PX` (residue from pushes that landed against
  impassable terrain or building body clearance). Collision is deterministic overlap cleanup for
  dynamic unit traffic; static correctness comes from standability checks before positions or
  scaffolds are accepted.

---
