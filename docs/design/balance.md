## 5. Balance definitions & constants
Kind-specific server balance lives in `server/crates/rules/src/defs.rs`; faction availability,
buildables, trainables, upgrade ids, and ability carriers live in
`server/crates/rules/src/faction.rs`; terrain movement/cover/concealment hooks live in
`server/crates/rules/src/terrain.rs` and currently return the all-open-ground defaults.
`config.rs` is the thin constants module for timings, tile size, starting resources, supply caps,
mining amounts, and other scalar simulation constants; its `unit_stats(kind)` and
`building_stats(kind)` helpers read the defs table.
`client/src/config.js` mirrors the subset the UI/render/fog needs (costs, supply, sight, sizes,
colors, and command-card descriptors). Keep both in sync; run
`node scripts/check-faction-catalog-parity.mjs` to mechanically compare the Rust-authoritative
default faction catalog to the client descriptors.
The server wiki's `/wiki/stats` page is generated from the same Rust definitions and faction
catalogs. For changes that affect visible stats, faction availability, upgrades, or ability
metadata, run `node scripts/check-wiki.mjs`; it includes the wiki route/table checks and the client
catalog parity check.

`server/src/config.rs` and `server/crates/sim/src/config.rs` are compatibility shims for
Rust-owned balance exports while call sites are migrated. They should not accumulate server-shell
or sim-only implementation constants. Those values belong beside the module that owns the behavior.

### Final source-of-truth map and guardrails

Use `server/crates/rules/src/defs.rs` for unit/building stat records, costs, supply, sight, ranges,
footprints, body dimensions, and build/train timing. Use `server/crates/rules/src/faction.rs` for
faction catalogs: buildables, trainables, research rows, ability carriers, command-card descriptors,
and ability/upgrade metadata exported by Rust. Use `server/crates/rules/src/balance.rs` for shared
scalar constants such as tick rate, resource-node amounts, support-weapon timings/ranges, upgrade
durations, body dimensions, and ability effect scalars. Sim-only behavior constants belong beside
the sim module that owns the behavior rather than in the compatibility config shims.

`client/src/config.js` mirrors only the subset needed by UI, rendering, fog previews, and command
cards. Rust-owned mirrored values include gameplay-visible costs, supply, sight, footprints, body
dimensions, client-visible timing/range/duration constants, faction legality, upgrade metadata,
ability descriptors exported by the Rust faction catalog, ability effect fields exported in the
rules dump, and resource starting amounts. Client-owned values include render colors, camera
defaults, fog overlay alpha, command-card layout hints, local presentation labels/icons that are not
exported by Rust, and resource render labels/sizes.

Run `node scripts/check-faction-catalog-parity.mjs` after changing Rust-owned values that are
mirrored into `client/src/config.js`. The check runs the Rust `rts-rules` faction catalog dump,
including the `clientConfig` parity payload, and compares the client mirror for catalogs, stat
fields, body dimensions, resource amounts, upgrade metadata, ability compact/order-stage codes, and
Rust-owned ability descriptors/effect fields. Run `node scripts/check-wiki.mjs` as well when a
change affects visible stats, faction availability, upgrades, or ability metadata that appears on
the generated `/wiki/stats` page.

| Constant | Before Phase 5 | After Phase 5 | Mirror impact |
|----------|----------------|---------------|---------------|
| `MORTAR_FIRE_TOLERANCE_RAD` | Sim-only mortar aim tolerance exported from `server/crates/sim/src/config.rs` beside mirrored balance constants | Sim-local `server/crates/sim/src/game/mortar.rs` `FIRE_TOLERANCE_RAD`, owned by mortar firing behavior | None; it is not mirrored into `client/src/config.js` and does not change wire shape or balance values |

### Client mirror boundary inventory

Phase 1 records the current source-of-truth map before later phases add broader mechanical checks.
This is an inventory only; it does not change balance, gameplay, or client rendering.

| Value/path | Rust owner | JS mirror path | Category | Current checker | Proposed future checker | Client-only exclusion reason | Compact version impact |
|------------|------------|----------------|----------|-----------------|-------------------------|------------------------------|------------------------|
| `TICK_HZ`, `TICK_MS`, `TILE_SIZE`, simulation timing scalars | `server/crates/rules/src/balance.rs`, re-exported by `server/src/config.rs` and `server/crates/sim/src/config.rs` | `client/src/config.js` `TICK_HZ`, `SNAPSHOT_MS`, interpolation delay constants | balance scalar | `scripts/check-faction-catalog-parity.mjs` checks `TICK_HZ` and client-visible duration constants against the Rust rules dump | Extend the structured dump if another timing scalar becomes client-visible | Interpolation delay is client-only smoothing; `TICK_HZ` is mirrored | No compact impact unless snapshot cadence or compact fields change |
| Unit and building costs, supply, sight, footprint/radius, train/build times, and command-card stat rows | `server/crates/rules/src/defs.rs` and `server/crates/rules/src/balance.rs`; faction legality in `server/crates/rules/src/faction.rs` | `client/src/config.js` `STATS`, `WORKER_BUILDABLE`, `FACTION_CATALOGS` | balance scalar / faction catalog fact | `scripts/check-faction-catalog-parity.mjs` checks catalog legality, costs, supply, sight, ranges, build ticks, building footprints, requirements, train lists, research lists, and non-body unit render radius; `node scripts/check-wiki.mjs` covers generated wiki stats when run | Future work can move client-only labels/icons into Rust catalogs if they should become authoritative | Client-only labels/icons in `STATS` are presentation unless the Rust catalog exports them; `STATS.size` for body-driven vehicles is presentation because the Rust-owned body dimensions are checked separately | No compact impact |
| Vehicle/body dimensions | `server/crates/rules/src/balance.rs` `*_BODY_*` constants | `client/src/config.js` `TANK_BODY`, `ANTI_TANK_GUN_BODY`, `ARTILLERY_BODY`, `SCOUT_CAR_BODY`, `COMMAND_CAR_BODY` | balance scalar | `scripts/check-faction-catalog-parity.mjs` checks every client body length, width, and clearance against the Rust rules dump | Keep adding new body records to the dump/check when body-driven units are added | None; client uses these for art, selection, and advisory placement previews, including Tank Trap preview rejection for vehicle-body units, while Rust collision is authoritative | No compact impact |
| Ability descriptors, carrier lists, target mode, range, cooldown, cost, queueability, autocast, command-card label/icon/hotkey/title | `server/crates/rules/src/faction.rs` plus scalar constants in `server/crates/rules/src/balance.rs` | `client/src/config.js` `ABILITIES` and imported `ABILITY` ids | faction catalog fact / balance scalar | `scripts/check-faction-catalog-parity.mjs` checks exported command-card descriptors, carriers, target mode, range, cooldown, cost, queueability, autocast, compact codes, and Rust-owned effect fields present on descriptors such as radius, delay, duration, pull multipliers, speed, and damage; protocol parity checks ability compact codes | Future effect fields should be added to the Rust dump and descriptor assertion when they become client-visible | Not UI-only when the field is exported by Rust faction catalogs or balance constants; purely local affordance copy belongs in the documented exclusion list | Code changes may affect compact ability/order-stage codes; descriptor-only changes do not |
| Upgrade descriptors, research building, prerequisites, cost, and research time | `server/crates/rules/src/faction.rs` plus `server/crates/rules/src/balance.rs` upgrade constants | `client/src/config.js` `UPGRADES` and imported `UPGRADE` ids | faction catalog fact / balance scalar | `scripts/check-faction-catalog-parity.mjs` checks research building, list membership, upgrade costs, research ticks, and prerequisite upgrade ids | Labels/icons/descriptions can be moved into Rust catalogs later if they should become authoritative | Labels/icons/descriptions are client-only today unless moved into the Rust catalog | No compact impact unless upgrade ids/codes change |
| Resource node starting amounts and economy resource names | `server/crates/rules/src/balance.rs` `STEEL_PATCH_AMOUNT`, `OIL_GEYSER_AMOUNT`; fixed Steel/Oil economy fields in sim/protocol | `client/src/config.js` `RESOURCE_AMOUNTS`, `KIND.STEEL`, `KIND.OIL`, HUD/resource render helpers | balance scalar / wire DTO | `scripts/check-faction-catalog-parity.mjs` checks node starting amounts; protocol parity checks resource kind codes | Add future client-visible resource amounts to the rules dump/check | Resource render labels and sizes are client presentation; amounts affect minimap/tooltips/render assumptions | Resource kind/code changes affect protocol/compact; amount changes do not |
| `PLAYER_PALETTE` | `server/src/lobby/mod.rs` | `client/src/config.js` `PLAYER_PALETTE` | server-owned presentation data mirrored by client | `tests/protocol_parity.mjs` source-scrapes the lobby palette | Structured lobby/config dump | Not client-only because server assigns lobby/start colors and sends them on the wire | No compact impact |
| Terrain, health, selection, placement, and drag colors | None in Rust; rendering-only choices | `client/src/config.js` `COLORS` except resource identity colors that should stay consistent with Steel/Oil presentation | UI-only presentation data | None | Exclusion list in future config parity check | Client owns visual palette; it does not affect simulation, wire DTO shape, or authoritative fog | No compact impact |
| Fog overlay alpha | Authoritative fog visibility is in sim snapshots; alpha is not a Rust value | `client/src/config.js` `FOG_EXPLORED_ALPHA`, `FOG_UNEXPLORED_ALPHA` | UI-only presentation data | None | Exclusion list in future config parity check | Client owns opacity; Rust owns which tiles/entities are visible | No compact impact |
| Camera defaults | None in Rust | `client/src/config.js` `CAMERA` | UI-only presentation data | None | Exclusion list in future config parity check | Client-only input/render affordance | No compact impact |
| Anti-tank gun field-of-fire preview | `server/crates/rules/src/balance.rs` `ANTI_TANK_GUN_FIELD_OF_FIRE_RAD` is authoritative at 45 degrees total | `client/src/config.js` `ANTI_TANK_GUN_FIELD_OF_FIRE_RAD` | balance scalar / advisory UI mirror | `scripts/check-faction-catalog-parity.mjs` checks the client preview against the Rust field-of-fire constant | Keep the preview Rust-owned because it represents the authoritative deployed firing arc | Not client-only: the client preview represents an authoritative firing arc | No compact impact |

### Phase 4 parity exclusions

The structured rules dump intentionally excludes client-owned presentation values that do not have
Rust catalog or balance ownership: global terrain and selection colors, fog overlay alpha, camera
defaults, command-card layout hints, and resource render labels/sizes. `STATS` labels and icons
remain client-owned until they are exported by a Rust catalog. `STATS.size` for units with a
Rust-owned body record is also presentation-only; the parity check enforces those units through
their `*_BODY_*` length, width, and clearance values instead.

### 5.0 Faction economy contract

The faction rollout keeps Steel, Oil, and Supply as the global economy contract. Faction catalogs
decide which global units, buildings, upgrades, and abilities are legal for a player and define
starting Steel/Oil values plus starting entity loadouts, but they still use fixed `steel`, `oil`,
`supplyUsed`, and `supplyCap` fields. Unknown non-empty faction ids do not fall back to the
Kriegsia catalog in lower-level economy queries: catalog-gated build/train/research/gather,
production-anchor, and supply checks return empty or false. Start-map resource nodes remain Steel
and Oil nodes. Score values, replay analysis values, command-card costs, affordability checks,
refunds, and supply reservation are intentionally Steel/Oil/Supply-shaped.
Catalog existence is not lifecycle admission. `server/crates/rules/src/faction.rs` may contain
playable catalogs, explicit fixture catalogs, and future catalog data, but
`server/src/lobby/faction_validation.rs` decides which ids can enter normal lobby, AI, replay,
branch, quickstart, self-play, dev, match-history, and post-match paths. Fixture-only and
reserved/future ids must not inherit Kriegsia economy behavior or appear in product selectors just
because their catalog rows are dumpable.

Approved direct Steel/Oil/Supply modules for this plan are:

- `server/crates/rules/src/defs.rs`, `server/crates/rules/src/economy.rs`, and
  `server/crates/rules/src/balance.rs` for authoritative costs, node amounts, and supply values.
- `server/crates/sim/src/game/player_state.rs`, `services/commands.rs`,
  `services/construction.rs`, `services/economy.rs`, `services/supply.rs`, `scoring.rs`,
  `analysis.rs`, `snapshot.rs`, `replay.rs`, and `setup.rs` for fixed-field simulation,
  score/replay analysis, and start/loadout compatibility shims. New lifecycle/replay starts should
  prefer per-player `PlayerStartingLoadout` records over global starting Steel/Oil overrides.
- `server/crates/protocol/src/lib.rs`, `server/src/protocol.rs`,
  `server/crates/sim/src/protocol.rs`, and `client/src/protocol.js` for the mirrored wire and
  compact transport fields.
- `client/src/config.js`, `client/src/hud.js`, `client/src/hud_command_card.js`,
  `client/src/observer_analysis_overlay.js`, `client/src/scoreboard.js`, and resource rendering
  helpers for current HUD, command-card, replay-analysis, score, and map-resource display.

Generic resources are deferred. If a future faction needs a non-Steel/Oil resource, that work must
be a separate migration across player state, snapshots, compact transport, replay artifacts,
observer analysis, scoring, HUD rows, command-card costs, protocol parity, and prediction/WASM
compatibility.

### 5.1 Target theme and MVP combat loop

The target gameplay direction is a simplified World War II-inspired battlefield with
fictional, faction-agnostic sides. This is not a historical simulation. The theme should
support readable gameplay, clear unit roles, and strong terrain identity without national
or regime-specific iconography.

MVP scope:
- No air forces.
- Late-game Artillery is implemented as the Superior Firepower capstone; Mortar Teams remain the
  early delayed-area fire tool.
- No mines, morale, logistics, suppression-depth model, or detailed tank armor model yet. Tanks
  do have a simple hull-facing armor rule for anti-tank damage.

Core unit roles:
- **Rifleman** is the baseline combat unit: cheap, flexible, useful for capturing and
  holding ground, and the primary answer to enemy infantry in forests.
- **Machine gun** is the defensive escalation unit: it takes one second
  (`MACHINE_GUNNER_SETUP_TICKS`) to set up after stopping, then fires at a very high rate.
  Once deployed it must spend the same one-second interval tearing down before it can move.
  Machine-gun nests
  are the main base-defense tool and should dominate open-ground infantry combat in the
  second stage of the game.
- **Scout Car** is the Mobile Warfare path-entry unit from Vehicle Works: fast, unarmored,
  high-vision, and useful for raiding, scouting, and smoke-enabled attacks before heavier armor
  arrives.
- **Tank** is the machine-gun breaker and open-ground power unit: immune to rifle and
  machine-gun small-arms fire, strong against static defenses and exposed infantry, but
  vulnerable to other tanks and anti-tank infantry.
- **Anti-tank gun team** is the ambush counter to tanks: it can fight while packed at short
  range with reduced damage, or manually set up into a longer-ranged fixed field of fire.
  Deployed guns are dangerous from the side or rear, but weak or inefficient against regular
  infantry and cannot fire outside their emplacement arc.
- **Mortar Team** is the Superior Firepower path-entry pressure unit from Gun Works: its setup time
  is provisionally set to zero ticks for balance improvement, it cannot shoot while moving, and it
  lands delayed area shells that punish static positions and clumped units.
- **Artillery** is the Superior Firepower late capstone from Gun Works: it uses a tank-sized
  gameplay footprint but reads as an exposed field piece, must deploy into a narrow firing arc,
  cannot shoot inside its minimum range, and spends steel on each long-range point-fire shell.

Terrain rules:
- **Open ground** favors machine guns and tanks.
- **Forests** are passable by infantry and impassable to tanks.
- Infantry in forests gets defensive and concealment bonuses.
- Forests are intentionally "infantry country": the main way to clear infantry from a
  forest is to send in your own infantry.
- Tanks and machine guns can contain forests by covering exits, clearings, and forest
  edges, but they should not reliably clear forest infantry from outside.

Intended progression:
- Early game: riflemen fight for map control.
- Midgame: machine guns lock down open lanes and bases.
- Armor phase: tanks break machine-gun-heavy defensive lines in open terrain.
- Counter-armor phase: anti-tank infantry, forest ambushes, and other tanks punish
  unsupported tanks.
- Forest fights remain infantry-led so tanks and machine guns never become universal
  answers.

### 5.2 Current implementation constants

The current implementation uses the themed unit/building names below. Combat is handled by the
shared attack model plus the support-weapon setup/teardown state, tank turret aim gates, and
tank hull-facing damage modifiers for anti-tank hits against tank victims. Tanks keep their active
movement path while firing on either `Move` or `AttackMove` orders; riflemen upgraded with
Methamphetamines are permanently charging, keep advancing while firing with normal accuracy, and
move at tank speed; other mobile combat units
still hold position once a target is in weapon range. Scout cars also fire while moving using an
independent rear machine-gun facing. They are unarmored light vehicles and do not receive
armored damage reduction, but anti-tank guns do not roll their infantry miss chance against them.
Plain `Move` tanks and scout cars only fire at enemies already in
weapon range, while `AttackMove` tanks and scout cars can chase acquired targets. When they chase an
acquired target from outside weapon range, they path to a standoff point inside firing range instead
of the target center. Tank auto-targeting first checks in-range Anti-Tank Guns, Tanks, Tank Traps,
and Mortar Teams, in that order, before generic acquisition; this priority can replace a retained
lower-priority moving-fire target but does not chase out-of-range priority targets or override
explicit player attack orders. Forest-specific rules are future work. The unit, building, and
resource-node tables below are the human-readable form of the authoritative `rules::defs` records.

Default auto-acquisition ranks already-legal targets by weapon fit before distance. Small-arms
default weapons prefer soft targets (`ArmorClass::Small`) over armored or hard targets, but they
still fire at armor, buildings, or vehicle obstacles when no better legal target exists; infantry-like
units still do not auto-acquire Tank Traps without a direct attack order. Anti-armor default weapons
prefer anti-armor threats and armored/hard targets over ordinary soft targets. Tanks keep a narrower
immediate-threat override for targets already in relevant range: Anti-Tank Guns are first, then
other anti-armor threats, armored obstacles, support weapons, and only then ordinary soft targets.
Vehicle-body units treat enemy Tank Traps as high-priority breach targets only when the trap is on
the unit's current short route window or helps close a vehicle-body gap across that route; nearby
irrelevant traps remain attackable fallbacks but no longer outrank ordinary combat targets.
Moving-fire retention is sticky but not absolute: Tanks, Scout Cars, and charged Riflemen keep a
current legal target across equal-rank comparisons so they do not flicker between similar enemies,
but higher-rank default-weapon threats still steal focus. This ranking scope is limited to default
attacks; future grenades, satchels, or demolition attacks need separate attack profiles and explicit
activation/autocast policy instead of being folded into default targeting.

- `TICK_HZ = 30`, `SNAPSHOT_EVERY_N_TICKS = 1`.
- `MACHINE_GUNNER_SETUP_TICKS = 30` (~1s setup or teardown for support weapons).
- Mortar Teams use `MORTAR_TEAM_SETUP_TICKS = 0` (no setup or teardown), `MORTAR_RANGE_TILES = 12`,
  `MORTAR_SHELL_DELAY_TICKS = 68` (~2.27s travel), `MORTAR_OUTER_RADIUS_TILES = 1.5`,
  `MORTAR_INNER_RADIUS_TILES = 0.5`,
  `MORTAR_OUTER_DAMAGE = 40`, `MORTAR_INNER_DAMAGE = 100`, and `MORTAR_AUTOFIRE_ERROR_TILES = 0.35`.
  The inner radius is fully armor-piercing against armored targets; the outer radius keeps
  semi-armor-piercing damage against armored targets. Manual Fire uses hotkey `X`; autocast
  uses normal idle/attack-move acquisition after Mortar Autocast research completes. Mortar impacts
  apply the same damage to friendly and enemy units/buildings; autocast skips predicted impact
  points that would hit any owned unit or building at its current position, while manual fire remains
  unrestricted.
- anti-tank guns use `ANTI_TANK_GUN_PACKED_RANGE_TILES = 5`, `ANTI_TANK_GUN_DEPLOYED_RANGE_TILES = 14`,
  `ANTI_TANK_GUN_PACKED_DAMAGE_MULTIPLIER = 0.75`, and
  `ANTI_TANK_GUN_FIELD_OF_FIRE_RAD = 45 degrees total`.
- Artillery uses `ARTILLERY_MIN_RANGE_TILES = 15`, `ARTILLERY_MAX_RANGE_TILES = 60`,
  `ARTILLERY_FIELD_OF_FIRE_RAD = 20 degrees total`, `ARTILLERY_RELOAD_TICKS = 90` (~3s),
  `ARTILLERY_SETUP_TICKS = 90` (~3s), `ARTILLERY_SHELL_DELAY_TICKS = 150` (~5s), and
  `ARTILLERY_AMMO_COST_STEEL = 10`.
  Repeated fire from the same deployed gun tightens from `ARTILLERY_INITIAL_ERROR_TILES = 10.0`
  to `ARTILLERY_MIN_ERROR_TILES = 2.0` over 5 shots; moving resets that accuracy ramp.
  Its body length, width, clearance, and selection radius match the Tank; its exposed carriage,
  long barrel, large wheels, and deployed spades carry the visual distinction instead of a larger
  footprint. Impacts deal
  150 armor-piercing damage within 1 tile and non-armor-piercing falloff down to 10 damage at
  3 tiles, including friendly fire.
- `TANK_OIL_COST_PER_PX = 20 / (96 * TILE_SIZE)`: tank movement still uses the original
  96-tile calibration, so driving the wider 126-tile map costs proportionally more oil than
  before.
- `SCOUT_CAR_OIL_COST_PER_PX = 5 / (96 * TILE_SIZE)`: scout cars burn oil for movement at
  half the previous tank movement rate. Command Cars use this same movement-oil cost. Tanks, scout
  cars, and command cars cannot advance while their owner has zero oil.
- Human selection and command bandwidth is supply-based: `BASE_COMMAND_SUPPLY_CAP = 24` command
  supply plus `COMMAND_CAR_SUPPLY_CAP_BONUS = 20` and the Command Car's own command weight for each
  selected/commanded Command Car. Units use their mirrored supply as command weight, so current Tanks
  consume 8 command supply and three Tanks fill the base budget; Command Cars still appear as weighted
  selections but their own weight is offset before their bonus is added.
- **Methamphetamines** (Training Centre research): costs 100 steel / 100 oil and takes 600 ticks
  (~20s). Once complete, all current and future riflemen for that player are permanently charging:
  1.25x movement speed (matching tank speed at 2.0 px/tick), fire while moving without an extra
  miss chance, and 25% faster attacks (16 tick cooldown becomes 12).
- **Anti-Tank Gun Crews** (R&D Complex research, protocol id `anti_tank_gun_unlock`): costs 200 steel / 75 oil
  and takes 600 ticks (~20s). Once complete, that player can train Anti-Tank Guns from Gun Works.
- **Unlock Artillery** (R&D Complex research, protocol id `artillery_unlock`): costs 300 steel /
  200 oil and takes 900 ticks (~30s). It requires completed Anti-Tank Gun Crews research. Once complete,
  that player can train Artillery from Gun Works.
- **Tank Production** (R&D Complex research, protocol id `tank_unlock`): costs 150 steel /
  100 oil and takes 600 ticks (~20s). Once complete, that player can train Tanks from Vehicle
  Works. Scout Cars remain immediately trainable from Vehicle Works.
- **Command Car** (R&D Complex research, protocol id `command_car_unlock`): costs 150 steel /
  150 oil and takes 900 ticks (~30s). It requires completed Tank Production research. Once
  complete, that player can train Command Cars from Vehicle Works.
- **Mortar Autocast** (R&D Complex research, protocol id `mortar_autocast`): costs 150 steel /
  150 oil and takes 600 ticks (~20s). Mortar Team autocast is unavailable before completion. Once
  complete, all current and future Mortar Teams for that player start with autocast enabled; players
  can still turn autocast off per selected Mortar Team.
- Ability metadata is Rust-authoritative in `server/crates/rules/src/faction.rs`. The faction
  catalog records carriers, target mode, ranges, cooldowns, charges, Steel/Oil cost, queueability,
  autocast support, and command-card affordances; `client/src/config.js` is mechanically checked
  against that registry for client-visible ability descriptors. Server execution maps those
  registry rows to a small set of sim-local effect hooks: self status, owned area status, delayed
  world effect, dash return, line projectile, Magic Anchor placement, and the one-off artillery
  point-fire path.
- **Ekat** is the first playable one-hero faction unit. The `ekat` catalog starts with
  one Ekat and one Zamok, no workers, no buildable menu, no research, and no other
  controllable combat units. Ekat has 300 HP, 1 HP/s regeneration while alive, 2.0 px/tick
  speed, 9-tile sight, no default attack, and no Steel/Oil/Supply cost. Her
  Dash ability targets up to 5 tiles, has no resource cost, has an 8s cooldown, requires a
  statically standable landing point, and leaves a four-second return marker that can be recast
  after one tick if the marker destination remains standable. Her Line Shot ability targets up to
  6 tiles, has no resource cost, has a 10s cooldown, and launches an 8 px/tick out-and-back
  projectile that deals 40 damage to enemy targetable entities intersecting each 0.6-tile-wide
  swept leg once per leg; if her Magic Anchor is active, the same activation also launches a second
  projectile from the anchor toward the cursor. Her Magic Anchor ability targets up to 5 tiles, has
  no resource cost, places one replacement-style non-blocking, non-attackable 10-second pull field
  with a 3-tile radius, slows units moving away from the anchor to as low as 0.45x speed near the
  center, boosts units moving toward the anchor up to 1.35x speed near the center, and drags idle
  units toward the anchor with less displacement for braced or heavy units.
- **Scout Car Smoke** (hotkey `D`): Scout cars have a targeted smoke-grenade ability immediately;
  no completed Gun Works is required. Each scout car spawns with 2 smoke uses; once those uses are
  depleted, that car cannot use Smoke again. Smoke has no steel or oil cost. Target range: 9 tiles
  from the caster. Launch delay: up to 100 ms at max range, scaling down for closer targets. Cloud
  radius: 2 tiles. Cloud duration: 5 seconds. Cooldown: 20 seconds per caster.
  Expected role: an offensive tool for closing on long-range defenses; push a scout car forward,
  place smoke between the advance and the anti-tank gun / machine-gun nest, then move mobile units through
  the resulting dead zone. Active smoke is neutral world state: it blocks authoritative fog and
  combat LOS, prevents units inside smoke from contributing vision, hides enemies inside smoke, and
  does not participate in pathing, collision, scoring, supply, or targeting as an entity. Units
  inside a cloud still receive that cloud in their own snapshot so the obscuring effect remains
  visible to the player occupying it.
  Cooldown duration (20s) exceeds cloud duration (5s), so each scout car has at most one active
  cloud at a time.
- **Command Car Breakthrough!** (hotkey `E`): Command Cars have a self-targeted instant area speed
  boost. It affects owned units within 9 tiles of the Command Car, lasts 180 ticks (~6s), has a
  750-tick (~25s) per-caster cooldown, has no resource cost, can be queued, and can be cast while
  the Command Car is moving. Affected units move at 1.4x speed, or 1.8x speed while inside smoke or
  during the 60-tick (~2s) recent-smoke grace window after leaving smoke. Multiple Breakthrough
  effects do not stack; a shorter refresh cannot reduce an active buff. Enemies see the status only
  when the affected unit is otherwise visible through authoritative fog. Fake Army and allied-unit
  support are deferred.
- Map: `TILE_SIZE = 32` px. The live map is the hardcoded handcrafted asset at
  `server/assets/maps/default-handcrafted.json` (126×126 today), served for tooling at
  `/maps/default-handcrafted.json`. The current asset is the original 96×96 handcrafted map
  padded with 15 passable grass tiles on every edge.
  Its JSON uses row strings (`.` grass, `#` rock, `~` water), named `sites`, and authored
  player-count-specific spawn `layouts`.
- Start: `STARTING_STEEL = 75`, `STARTING_OIL = 0`, `STARTING_WORKERS = 4`,
  one City Centre at the player's start tile, 12 steel patches with 1,000 steel each + 3 oil
  patches with 3,333 oil each nearby.
- Supply: City Centre, Zamok, and Depot each give `+8`; hard cap `200`.
- Attached mining: workers walk to a patch, latch onto it, and mine in place.
  Every `HARVEST_TICKS = 40` the load (`STEEL_LOAD = 2` / `OIL_LOAD = 2`) is deposited
  directly into the player's economy only if the resource node is within
  `MINING_CC_RANGE_TILES = 9.0` tiles of a completed City Centre owned by that player.
  Starting resources are placed within `CC_RESOURCE_MAX_DIST_TILES = 7.0`, giving City Centres a
  two-tile mining buffer around the authored/base resource cluster. If no completed City Centre is
  close enough, workers ignore new gather orders for that patch and active miners scatter roughly
  one tile away from the patch.
  When a patch empties the worker goes idle (no automatic retarget).
- One worker per patch: each node has a single harvest slot (`Entity::miner`). A patch is
  occupied only after the worker reaches `GatherPhase::Harvesting`; right-clicking a patch
  does not reserve it. Extra workers that arrive while the slot is taken go idle. The slot
  is advisory and self-heals — it's only honored while the recorded worker is alive and
  actively harvesting that node, so death / re-order / retarget free it automatically.
- Starting layout: each active main or natural site gets 12 steel patches and 3 oil patches.
  Map schema v2 stores named main/natural `sites` plus explicit spawn `layouts`. Each layout
  declares a `playerCount` and a list of slots, where each slot pairs one main with one or more
  naturals (`natural` legacy field or `naturals` array). At match start the seed selects one
  layout for the active player count, then shuffles that layout's slots so lobby seat order does
  not pin a human/AI to the same corner. The authored main/naturals grouping inside each slot
  stays intact, which lets maps define different fair naturals for adjacent, cross, safe-base, or
  other spawn constellations. Sites not selected by the chosen layout are unused. The Safer
  Expansions map grants each selected player an in-base natural plus the matching Default-map
  contested natural, giving each player three active bases including their main.

Unit stats (hp, dmg, range[tiles], cooldown[ticks], speed[px/tick], sight[tiles], cost, supply, buildTicks):

| kind            | hp  | dmg | range | cd | speed | sight | steel | oil | sup | buildTicks |
|-----------------|-----|-----|-------|----|-------|-------|-----|-----|-----|-----------|
| worker          | 40  | 4   | 1     | 24 | 2.0   | 7     | 50  | 0   | 1   | 396 (~13.2s) |
| rifleman        | 45  | 5   | 4     | 16 | 1.6   | 8     | 50  | 0   | 1   | 300 (~10s) |
| machine_gunner  | 55  | 4   | 6     | 6  | 1.28  | 8     | 75  | 10  | 2   | 400 (~13s) |
| mortar_team     | 75  | 40 outer / 100 inner AOE | 12 | 60 | 1.6 | 7 | 100 | 50 | 3 | 460 (~15s); trained at Gun Works (`steelworks` kind) |
| anti_tank_gun         | 45  | 60 deployed / 45 packed | 14 deployed / 5 packed | 72 | 1.6 | 6     | 75  | 25  | 3   | 440 (~15s); requires Gun Works (`steelworks` kind) and Anti-Tank Gun Crews (`anti_tank_gun_unlock`) researched in R&D Complex |
| artillery       | 150 | 150 AP inner / 150-10 outer AOE | 15-60 point fire | 90 | 1.3 | 4 | 300 | 100 | 5 | 750 (~25s); requires Gun Works (`steelworks` kind), Anti-Tank Gun Crews (`anti_tank_gun_unlock`), and Unlock Artillery (`artillery_unlock`) researched in R&D Complex; tank-sized footprint |
| scout_car       | 100 | 6   | 5     | 6  | 2.35  | 10    | 125 | 50  | 3   | 480 (~16s) |
| tank            | 292 | 60  | 5     | 72 | 2.0   | 6     | 425 | 150 | 8   | 750 (~25s); requires Vehicle Works (`factory` kind) and Tank Production (`tank_unlock`) researched in R&D Complex |
| command_car     | 225 | 0   | 0     | 0  | 2.35  | 10    | 150 | 75  | 4   | 450 (~15s); requires Vehicle Works (`factory` kind) and Command Car (`command_car_unlock`) researched in R&D Complex; no weapon; Scout Car-style movement with a smaller jeep-sized body |
| ekat       | 300 | 0   | 0     | 0  | 2.0   | 9     | 0   | 0   | 0   | 0; Ekat faction hero; no default attack; regenerates 1 HP/s |

Building stats (hp, sight, cost, footprint tiles wxh, buildTicks, extra):

| kind                       | player-facing name | hp  | sight | cost | foot | buildTicks | notes |
|----------------------------|--------------------|-----|-------|-----|------|-----------|-------|
| city_centre                | City Centre        | 600 | 9     | 200 | 3x3  | 550       | trains worker; +8 supply; players start with one free |
| zamok                      | Zamok              | 600 | 9     | 0   | 3x3  | 0         | Ekat start building; +8 supply; no trains/research in first playable slice |
| depot                      | Supply Depot       | 110 | 4     | 100 | 2x2  | 300       | +8 supply |
| barracks                   | Barracks           | 165 | 6     | 150 | 3x2  | 200       | trains rifleman and machine_gunner; requires a City Centre |
| training_centre            | Training Centre    | 300 | 6     | 100 steel + 50 oil | 3x2  | 560       | shared prerequisite before either advanced path; unlocks machine_gunner training at barracks and researches Methamphetamines; requires a City Centre and Barracks |
| research_complex           | R&D Complex        | 165 | 6     | 100 steel + 100 oil | 3x3  | 450       | research-only building for Anti-Tank Gun Crews, Unlock Artillery, Tank Production, Command Car, and Mortar Autocast; requires a City Centre and Training Centre |
| factory                    | Vehicle Works      | 360 | 6     | 125 steel + 125 oil | 3x3  | 749       | Mobile Warfare path building; trains scout_car immediately, trains tank after Tank Production research, and trains command_car after Command Car research; requires a City Centre and Training Centre |
| steelworks                 | Gun Works          | 300 | 6     | 150 steel + 100 oil | 3x3  | 599       | Superior Firepower path building; trains mortar_team immediately and trains Anti-Tank Guns/Artillery after R&D Complex research; requires a City Centre and Training Centre |
| tank_trap                  | Tank Trap          | 150 | 0     | 15 steel + 0 oil | 1x1  | 300       | engineer-built vehicle obstacle; sparse orthogonal pairs close the single tile between them for vehicle movement only; armored, no trains, no supply, no weapon, no sight/fog reveal, not an elimination building; requires a completed Training Centre |

Win: a player is **eliminated** when they own zero elimination-counting buildings; units and
Tank Traps alone do not keep them alive. Last player standing wins; a 1-player match never ends
(sandbox/exploration mode). In a
3-4 player match, a connected human who is eliminated receives a one-time `gameOver` score
snapshot immediately while the remaining players keep playing; final match resolution sends
`gameOver` only to players who have not already received one.

---
