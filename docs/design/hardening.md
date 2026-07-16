## 7. Hardening (input is untrusted)
The server treats every client as potentially hostile. Scout Planes are exposed through normal fog-safe projection and omitted when hidden from that player. Their private orbit state is sent only to the owner or full-world projections. Authoritative aerial fog stamping grants owner/team vision through terrain and building blockers while still respecting smoke. Each Command Car may own one active sortie, and duplicate planes with the same owner/source-car pair are removed before vision is stamped; planes launched by different Command Cars each contribute vision. Limits live next to the code:
- **Net-report diagnostic cap** (`server/crates/protocol/src/lib.rs`): client-supplied command lifecycle exemplars are capped during deserialization to the logged top-N contract.
- **WebSocket and lab scenario import caps** (`main.rs`, `lab_scenarios.rs`): WebSocket text-frame
  limits accommodate valid checkpoint-backed scenario round trips, while lab scenario import JSON
  has a separate explicit cap. Oversized frames are rejected before serde, and oversized scenario
  imports are rejected before checkpoint restore.
- **Client stress-test report caps** (`stress_tests.rs`): the public client-only benchmark POST is
  capped at 2 MiB before JSON extraction and validates its fixed schema/workload, scalar lengths,
  duration/status, bounded profiler tables, and 750 KiB SVG limit. Server-issued unguessable run ids
  are the only retrieval keys; the server stores no raw client IP and exposes no run-list endpoint.
  Scriptable SVG elements, handler attributes, and links are rejected before the same-origin
  attachment endpoint can serve a submitted flame graph. Database writes require the separate
  `RTS_RECORD_STRESS_TESTS` gate. See
  [`client-stress-tests.md`](client-stress-tests.md).
- **Command unit cap and budget** (`services/commands.rs`, with mirrored budget scalars in
  `command_budget.rs`): ordinary unit-list commands inspect at most
  `MAX_UNITS_PER_COMMAND = 256` submitted ids, dedupe that bounded window, and reject
  over-budget human commands before planning. Lab `issueCommandAs` requests that explicitly set
  `ignoreCommandLimits` bypass the command-supply budget and inspect at most
  `LAB_MAX_UNITS_PER_COMMAND = 4096` submitted ids, still bounded by the WebSocket frame cap. The
  human command budget is 24 supply plus
  `COMMAND_CAR_SUPPLY_CAP_BONUS = 20` and the Command Car's own command weight for each submitted
  owned Command Car, with mirrored unit supply as command weight and a fallback weight of 1.
  AI-owned players are exempt from the command-budget gameplay limit because live AI still enqueues
  ordinary `SimCommand`s through the same `Game::enqueue` seam as humans. Rejection drops the whole
  malformed human command and emits a private "Command supply exceeded" notice; the server does not
  silently trim the unit list.
- **Queued order caps** (`entity/order.rs` `MAX_QUEUED_ORDERS = 8`): each mobile unit stores at most
  eight future intents. Queued command application still runs the unit-list dedupe/cap first, and
  queued promotion drains invalid stale intents instead of retrying them forever.
  Phase 6 kept this cap at eight because no playtest evidence in the repo justified a larger
  command buffer; mixed ability/setup replay coverage now guards the current cap and command-log
  shapes.
- **Production queue caps** (`entity/state.rs` `MAX_PRODUCTION_QUEUE = 8`): each production
  building stores at most eight explicit unit entries and eight research entries. Manual entries
  may be unpaid, so this authority-side limit prevents free command spam from growing durable
  entity/checkpoint state without an economic bound. Standing repeat lists remain catalog-bounded
  and do not create an item until payment succeeds.
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
- **Lab scenario PR submission boundary**: draft PR creation is disabled unless
  `RTS_SCENARIO_PR_ENABLED` is truthy and server-side GitHub credentials/repo config are present.
  The browser sees only `GET /api/lab-scenarios/submission` capability metadata. Actual
  `submitScenario` requests originate in a lab room, export the current authoritative `Game`, apply
  validated authoring metadata, and write only `server/assets/lab-scenarios/<slug>.json` plus
  `server/assets/lab-scenarios/manifest.json` through a background job. The client never supplies
  credentials, repository paths, branch names, commit text, or scenario snapshots as authority.
  Catalog manifests are capped, scenario filenames must match their safe ids, authoring previews are
  capped by entity count and formatted JSON bytes, and PR requests must contain exactly one scenario
  JSON plus the manifest. Duplicate catalog ids/filenames, path traversal, unsafe branch prefixes,
  branch collisions, missing credentials, rate limits, and GitHub failures return structured errors;
  each lab room can start at most one PR submission job. Operators enable the service with
  `RTS_SCENARIO_PR_ENABLED=1`, `RTS_SCENARIO_PR_GITHUB_TOKEN`, `RTS_SCENARIO_PR_REPO`, optional
  `RTS_SCENARIO_PR_BASE_BRANCH`, and optional `RTS_SCENARIO_PR_BRANCH_PREFIX`. Live submission
  currently shells out to `git` and GitHub CLI (`gh`) on the server host; missing tools fail the job
  instead of falling back to browser credentials.
- **Deploy drain**: SIGTERM/Ctrl-C starts a server drain instead of immediately shutting down.
  The lobby flips into a draining state, existing room tasks continue ticking active normal
  matches, and new match starts are rejected while lobby clients see `can_start: false`. Fly's
  `kill_timeout` is 300 seconds; the server uses a 295 second application budget split into
  260 seconds of natural match drain, 10 seconds to ask active rooms to finalize for shutdown,
  20 seconds to wait for tracked match-history/replay writes, and 5 seconds of final
  WebSocket/Axum slack. If the natural phase expires, each active authoritative room receives a
  `FinalizeForShutdown` event before connection shutdown. Eligible normal live matches capture the
  current scores and replay artifact, queue a match-history row with `outcome = aborted` and no
  winner, and only then drop their active-match drain tracking. Non-eligible authoritative rooms
  ack without writing public history rows. Dev self-play/replay/scenario rooms are not tracked as
  deploy blockers because they can intentionally run or auto-restart forever. Operators should
  treat forced-finalization timeouts, match-history write wait timeouts, and `failed to record
  match` logs as validation blockers for any interrupted live match, then confirm Recent Matches
  shows `Aborted` and can launch the captured replay.
- **Deploy asset hermeticity**: release Docker builds generate browser-loadable prediction WASM
  assets with `scripts/build-sim-wasm.sh` inside the builder image, then fail if
  `client/vendor/sim-wasm/rts_sim_wasm.js` or `rts_sim_wasm_bg.wasm` is missing or empty. These
  generated files stay ignored in git, so deploys must not depend on untracked files in a local
  checkout. Missing static asset requests under paths such as `/vendor`, `/src`, `/assets`, or
  root files with extensions return 404 instead of the SPA `index.html`, making packaging mistakes
  visible to the client and probes.
- **Fog is authoritative**: `snapshot_for` and per-recipient event delivery go through
  `rules::projection`, which gates entity views, `target_id` tracers, and death/attack events on
  visibility. Normal active-player snapshots use the union of current fog from living teammates,
  while command validation and economy stay owner-local. Allied entity inspection exposes read-only
  details, but resources, supply, upgrades, rally/order plans, ability controls, debug paths, and
  command authority remain exact-owner-only. Event fanout and remembered-building refreshes use
  team-current visibility; enemy recipients get combat, death, build, support-fire, smoke, and
  under-attack events only when their team can currently see the relevant origin/target or when a
  documented damage reveal applies. Defeated/disconnected teammates stop contributing live sight,
  and neutral resources never grant vision. Hidden enemies are never sent except inside explicit
  five-second lingering death vision, which is stamped as ordinary temporary team sight and can
  therefore feed snapshots, command validation, remembered intel, and combat acquisition while it
  lasts. Visibility is terrain-aware: stone blocks sight beyond itself on both the server fog grid
  and the client cosmetic fog overlay.
- **Team-safe hostile command targeting**: explicit `Attack` commands, queued attack promotion, and
  target acquisition reject allied owners through the authoritative team relationship snapshot,
  not raw owner inequality. A malicious client can still send arbitrary entity ids, but allied,
  neutral, dead, hidden, smoke-hidden, stale, or non-targetable ids remain no-ops and do not become
  hostile attack orders or retained combat targets. Strict raw-owner checks are still required for
  command authority and economy operations.
- **Team-safe damage attribution**: direct-fire damage, shot interception, overpenetration, damage
  metadata, worker-retreat triggers, under-attack notices, and kill credit use the authoritative
  team relationship snapshot. Same-team entities are not legal direct-fire or overpenetration
  victims, and same-team damage does not update `last_damage_*` metadata or award kill credit.
  If unattributed damage deals the killing blow, stale prior damage attribution is cleared so an
  older enemy hit cannot receive credit for a friendly-fire kill. Mortar and artillery splash remain
  intentional friendly-fire surfaces: they can damage owned and allied entities in the blast radius,
  but same-team splash stays unattributed.
- **Shot blocking and overpenetration**: ranged attacks first resolve against the first enemy tank
  body or non-Tank-Trap building footprint intersecting the line from attacker to intended target.
  That blocker takes the shot damage and the intended target behind it is unharmed. Tank Traps are
  targetable buildings and still block vehicle movement, but they do not intercept shots; attacks
  aimed at a unit behind a Tank Trap continue to that unit. Shots that hit ordinary enemy units
  still overpenetrate past the primary target, but any enemy tank body or enemy non-Tank-Trap
  building footprint hit by that carry-through damage absorbs the shot and stops further
  overpenetration. Allied entities behind the primary target are ignored by overpenetration. Stone
  blocks target acquisition and primary fire.
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
  orders. A tank on active `Move` or `AttackMove` only opportunistically fires at enemies already in
  range; it does not chase out-of-range enemies or replace the commanded path with a standoff route.
  Direct `Attack` orders and post-arrival aggressive behavior can still use vehicle standoff
  pursuit. Shoot-while-moving units retain their current valid target before falling back to
  nearest-target acquisition, so drive-by fire tends to finish one enemy instead of spreading damage
  across every passing unit. Projection omits enemy `weaponFacing` when it would reveal a hidden
  target direction.
- **Rifleman Methamphetamines fire**: upgraded riflemen gain permanent moving fire and keep their
  movement path while firing at enemies in range instead of stopping to shoot. While on a plain
  `Move` or active `AttackMove`, upgraded riflemen only fire opportunistically at enemies already in
  range and do not chase. Moving Methamphetamines shots use normal rifleman accuracy and do not add
  a movement miss roll.
- **Scout car movement and weapon facing**: scout cars are light unarmored vehicles with a
  rear-mounted machine gun (higher damage, same range and cooldown as machine gunners). They use the
  same oriented-body/pathing/collision model as tanks, including vehicle standoff on direct pursuit
  and firing while moving, but they use simplified car locomotion instead of tank pivot locomotion. A
  scout car's
  yaw is capped by movement budget over a 1.5-tile minimum turn radius, so it can steer
  while translating but cannot rotate in place when blocked or badly misaligned. Reverse is a
  bounded maneuver latched to the immediate waypoint: nearby final waypoints and injected recovery
  waypoints can be reached by backing up, but route lookahead alone cannot put the car into reverse.
  Farther behind goals make the scout car drive through a broad forward turn instead of
  backtracking. Scout cars, tanks, and Anti-Tank Guns use the same clearance-aware player-move route
  shape: coarse A* still works on tiles, but vehicles add static-clearance, turn,
  adjacent-blocker, and corner-graze costs so open alternatives are preferred before local movement
  gets close to walls. Tank-style pivot vehicles (tanks and Anti-Tank Guns) expand each diagonal A* tile
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
  laterally reach; tanks and Anti-Tank Guns use a 5-tile lookahead with pivot-drive locomotion. The
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
  use the moving-fire rifleman model permanently, move at tank speed, and attack 25% faster.
  Legacy `charge` commands remain decodable but have no eligible carriers, cooldown, or runtime
  status.
- **Advanced research locks**: R&D Complex can queue `anti_tank_gun_unlock` for 100 steel / 50 oil
  over 300 ticks, unlocking Anti-Tank Gun training at Gun Works for that player;
  `artillery_unlock` for 200 steel / 100 oil over 750 ticks after completed
  `anti_tank_gun_unlock`, unlocking Artillery training at Gun Works;
  `ballistic_tables` for 150 steel / 100 oil over 600 ticks after completed
  `artillery_unlock`, tightening repeated Artillery point-fire shots;
  `tank_unlock` for 150 steel / 100 oil over 600 ticks, unlocking Tank and Command Car training at
  Vehicle Works; and `mortar_autocast` for 150 steel / 150 oil
  over 600 ticks, enabling Mortar Team autocast for current and future owned Mortar Teams; and
  `smoke_plus` for 150 steel / 150 oil over 600 ticks, doubling future Scout Car Smoke radius and
  duration.
  Server-side research validation checks the research building and prerequisite upgrades, while
  train validation checks both the producer kind and completed player upgrade set, so clients cannot
  bypass these locks by sending `research` or `train` commands directly.
- **Tank armor facing**: tank and Anti-Tank Gun attacks against tank victims use the victim tank's hull
  `facing` and the attacker's position. Front hits (`<=45°` from the hull direction) deal normal
  damage, side hits (`>45°` and `<=135°`) deal `1.25x`, and rear hits (`>135°`) deal `1.75x`.
  Infantry damage, building damage, non-tank victims, and non-anti-tank attackers ignore armor
  facing.
- **Worker direct-hit retreat**: combat stamps `last_damage_pos`/`last_damage_tick` only for enemy
  damage and never mutates orders or paths. `Game::worker_retreat_commands_for(player)` projects
  that private metadata into ordinary AI-owned worker `Move` commands for workers hit by enemy
  damage on the previous tick. The room task passes those commands through `rts-ai`, then enqueues
  them via the normal command path. Constructing workers stay latched, allied splash does not
  trigger retreat, and human players receive no automatic retreat.
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
  from its current position. Infantry use the same standability segment check as vehicles, with
  their circular bodies, so they can skip reachable dog-leg points instead of oscillating around the
  waypoint center. Workers do not use route waypoint skipping and keep following intermediate
  points, which avoids clipping building edges during construction/resource traffic. Blocked corners
  still keep the intermediate waypoint until the swept segment is legal. Oriented vehicles
  additionally keep their facing-specific guard so a waypoint behind the hull, including scout-car
  reverse recovery, must be physically reached.
- **Vehicle clearance pathing**: player-issued scout-car, tank, and Anti-Tank Gun `Move` / `AttackMove`
  path requests use the shared clearance-aware tile A* route shape. The route shape adds finite
  static-clearance, turn, adjacent-blocker, and corner-graze costs so open alternatives are
  preferred before local movement gets close to walls. Path cache keys include the route-shaping
  mode so clearance-shaped movement paths do not share cached routes with normal interaction
  pathing. The returned route is still used for reachability, then snaps the reverse-ordered final
  waypoint to the exact command goal. Scout cars simplify straight segments up to their route
  lookahead window; tanks and Anti-Tank Guns keep their L-expanded tile-center route so corner-clearing
  waypoints are not collapsed back into diagonal shortcuts. Interaction paths for attack chasing,
  gathering, and build staging remain tile-guided `Normal` routing so combat, mining, construction
  range checks, and infantry/worker traffic stay controlled by their existing logic.
- **Vehicle diagonal-pinch avoidance**: A* passability for oriented-vehicle bodies (tanks, scout
  cars, Anti-Tank Guns) rejects tiles wedged between two diagonally-opposite blocked corners — i.e. (NW
  blocked AND SE blocked) OR (NE blocked AND SW blocked). The rotating hull cannot legally thread
  such 1-tile gaps at any intermediate heading, so routing through them used to deadlock at the
  static-blocked-repath threshold. Infantry pathing is unaffected; legitimate 2-tile-wide corridors
  always leave at least one diagonal of each pair open and remain traversable.
- **Formation goal legality**: group move goals keep the existing distance-sensitive formation
  behavior, but candidate tiles are accepted only when the specific unit kind can stand there under
  `standability::unit_static_standable`. This prevents large units from being assigned a center tile
  whose body would clip terrain or a building footprint; dynamic unit traffic is still handled by
  steering and collision after movement. Vehicle-body group moves use the same deterministic candidate
  search but first prefer goal tiles with one open tile between any vehicle-body unit and any other
  assigned unit, while infantry can still pack into adjacent infantry slots. The search falls back to
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
