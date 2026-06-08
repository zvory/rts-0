## 5. Balance definitions & constants
Kind-specific server balance lives in `server/src/rules/defs.rs`; terrain movement/cover/
concealment hooks live in `server/src/rules/terrain.rs` and currently return the all-open-ground
defaults. `config.rs` is the thin constants module for timings, tile size, starting resources,
supply caps, mining amounts, and other scalar simulation constants; its `unit_stats(kind)` and
`building_stats(kind)` helpers read the defs table.
`client/src/config.js` mirrors the subset the UI/render/fog needs (costs, supply, sight, sizes,
colors). Keep both in sync; the comment in each file points at the other.

### 5.1 Target theme and MVP combat loop

The target gameplay direction is a simplified World War II-inspired battlefield with
fictional, faction-agnostic sides. This is not a historical simulation. The theme should
support readable gameplay, clear unit roles, and strong terrain identity without national
or regime-specific iconography.

MVP scope:
- No air forces.
- No artillery or mortars yet.
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
- **Tank** is the machine-gun breaker and open-ground power unit: immune to rifle and
  machine-gun small-arms fire, strong against static defenses and exposed infantry, but
  vulnerable to other tanks and anti-tank infantry.
- **Anti-tank gun team** is the ambush counter to tanks: it can fight while packed at short
  range with reduced damage, or manually set up into a longer-ranged fixed field of fire.
  Deployed guns are dangerous from the side or rear, but weak or inefficient against regular
  infantry and cannot fire outside their emplacement arc.

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
movement path while firing on either `Move` or `AttackMove` orders; charged riflemen keep advancing
while firing and roll a 50% miss chance for those moving charge shots; other mobile combat units
still hold position once a target is in weapon range. Scout cars also fire while moving using an
independent rear machine-gun facing. They are unarmored light vehicles and do not receive
armored damage reduction, but AT guns do not roll their infantry miss chance against them.
Plain `Move` tanks and scout cars only fire at enemies already in
weapon range, while `AttackMove` tanks and scout cars can chase acquired targets. When they chase an acquired
target from outside weapon range, they path to a standoff point inside firing range instead of the
target center. Forest-specific rules are future work. The unit, building, and resource-node tables
below are the human-readable form of the
authoritative `rules::defs` records.

- `TICK_HZ = 30`, `SNAPSHOT_EVERY_N_TICKS = 1`.
- `MACHINE_GUNNER_SETUP_TICKS = 30` (~1s setup or teardown for support weapons).
- AT guns use `AT_GUN_PACKED_RANGE_TILES = 5`, `AT_GUN_DEPLOYED_RANGE_TILES = 12`,
  `AT_GUN_PACKED_DAMAGE_MULTIPLIER = 0.75`, and
  `AT_GUN_FIELD_OF_FIRE_RAD = PI / 4` (45 degrees total).
- `TANK_OIL_COST_PER_PX = 20 / (96 * TILE_SIZE)`: tank movement still uses the original
  96-tile calibration, so driving the wider 126-tile map costs proportionally more oil than
  before.
- `SCOUT_CAR_OIL_COST_PER_PX = 5 / (96 * TILE_SIZE)`: scout cars burn oil for movement at
  half the previous tank movement rate. Tanks and scout cars cannot advance while their owner has
  zero oil.
- Ability shell metadata: Rifleman Charge has a 5s reuse delay and remains unlocked by a completed
  Training Centre.
- **Scout Car Smoke** (hotkey `D`): Scout cars gain a targeted smoke-grenade ability once the
  owner has a completed Steelworks. Each scout car spawns with 2 smoke uses; once those uses are
  depleted, that car cannot use Smoke again. Smoke has no steel or oil cost. Target range: 9 tiles
  from the caster. Cloud radius: 2 tiles. Cloud duration: 5 seconds. Cooldown: 20 seconds per caster.
  Expected role: an offensive tool for closing on long-range defenses; push a scout car forward,
  place smoke between the advance and the AT gun / machine-gun nest, then move mobile units through
  the resulting dead zone. Active smoke is neutral world state: it blocks authoritative fog and
  combat LOS, prevents units inside smoke from contributing vision, hides enemies inside smoke, and
  does not participate in pathing, collision, scoring, supply, or targeting as an entity. Units
  inside a cloud still receive that cloud in their own snapshot so the obscuring effect remains
  visible to the player occupying it.
  Cooldown duration (20s) exceeds cloud duration (5s), so each scout car has at most one active
  cloud at a time.
- Map: `TILE_SIZE = 32` px. The live map is the hardcoded handcrafted asset at
  `server/assets/maps/default-handcrafted.json` (126×126 today), served for tooling at
  `/maps/default-handcrafted.json`. The current asset is the original 96×96 handcrafted map
  padded with 15 passable grass tiles on every edge.
  Its JSON uses row strings (`.` grass, `#` rock, `~` water) plus ordered `baseSites`.
- Start: `STARTING_STEEL = 75`, `STARTING_OIL = 0`, `STARTING_WORKERS = 4`,
  one City Centre at the player's start tile, 18 steel patches + 3 oil patches nearby.
- Supply: City Centre gives `+10`, Depot gives `+8`, hard cap `200`.
- Attached mining: workers walk to a patch, latch onto it, and mine in place.
  Every `HARVEST_TICKS = 40` the load (`STEEL_LOAD = 2` / `OIL_LOAD = 2`) is deposited
  directly into the player's economy only if the resource node is within
  `MINING_CC_RANGE_TILES = 7.0` tiles of a completed City Centre owned by that player.
  The range matches `CC_RESOURCE_MAX_DIST_TILES`, so each starting City Centre can mine
  every patch in its main-base cluster. If no completed City Centre is close enough, workers ignore
  new gather orders for that patch and active miners scatter roughly one tile away from the patch.
  When a patch empties the worker goes idle (no automatic retarget).
- One worker per patch: each node has a single harvest slot (`Entity::miner`). A patch is
  occupied only after the worker reaches `GatherPhase::Harvesting`; right-clicking a patch
  does not reserve it. Extra workers that arrive while the slot is taken go idle. The slot
  is advisory and self-heals — it's only honored while the recorded worker is alive and
  actively harvesting that node, so death / re-order / retarget free it automatically.
- Starting layout: each base site gets 18 steel patches and 3 oil patches. `baseSites` are stored
  as interleaved pairs: `[start0, expansion0, start1, expansion1, ...]`. The pairs are shuffled
  by the match seed, and the first N shuffled starts become the active player starts. For one-,
  three-, and four-player games, each selected start keeps its authored paired neutral expansion.
  For two-player games, the two neutral expansion sites are selected from the authored expansion
  pool by scoring each assignment in the players' local start-to-enemy frames; this favors matching
  forward/lateral offsets and natural distance, avoiding one player receiving a shared middle
  natural while the other receives a side natural. Sites not selected as an active start or active
  expansion are unused, giving exactly 2N active bases on the map. Shuffling stops the lobby seat
  order from pinning the human/AI to the same corner every match.

Unit stats (hp, dmg, range[tiles], cooldown[ticks], speed[px/tick], sight[tiles], cost, supply, buildTicks):

| kind            | hp  | dmg | range | cd | speed | sight | steel | oil | sup | buildTicks |
|-----------------|-----|-----|-------|----|-------|-------|-----|-----|-----|-----------|
| worker          | 40  | 4   | 1     | 24 | 1.6   | 7     | 50  | 0   | 1   | 360 (~12s) |
| rifleman        | 45  | 5   | 4     | 16 | 1.6   | 8     | 50  | 0   | 1   | 300 (~10s) |
| machine_gunner  | 55  | 4   | 6     | 6  | 1.28  | 8     | 75  | 10  | 2   | 400 (~13s) |
| at_team         | 45  | 60 deployed / 45 packed | 12 deployed / 5 packed | 72 | 1.152 | 6     | 75  | 25  | 3   | 440 (~15s); requires Steelworks |
| scout_car       | 150 | 6   | 5     | 6  | 2.35  | 10    | 125 | 50  | 3   | 480 (~16s) |
| tank            | 292 | 60  | 5     | 72 | 2.0   | 6     | 300 | 150 | 6   | 750 (~25s); requires Steelworks |

Building stats (hp, sight, cost, footprint tiles wxh, buildTicks, extra):

| kind                       | hp  | sight | cost | foot | buildTicks | notes |
|----------------------------|-----|-------|-----|------|-----------|-------|
| city_centre          | 600 | 9     | 200 | 3x3  | 400       | trains worker; +10 supply; players start with one free |
| depot                      | 220 | 4     | 100 | 2x2  | 300       | +8 supply |
| barracks                   | 320 | 6     | 150 | 3x2  | 200       | trains rifleman, machine_gunner, at_team; requires a City Centre |
| training_centre   | 300 | 6     | 100 steel + 50 oil | 3x2  | 280       | unlocks machine_gunner training at barracks; requires a City Centre and Barracks |
| factory                    | 360 | 6     | 200 steel + 100 oil | 3x3  | 330       | trains scout_car, tank; requires a City Centre and Training Centre |
| steelworks                 | 300 | 6     | 125 steel + 125 oil | 2x2  | 310       | unlocks at_team training at barracks and tank training; requires a City Centre and Training Centre |

Win: a player is **eliminated** when they own zero buildings (units alone do not keep them
alive). Last player standing wins; a 1-player match never ends (sandbox/exploration mode). In a
3-4 player match, a connected human who is eliminated receives a one-time `gameOver` score
snapshot immediately while the remaining players keep playing; final match resolution sends
`gameOver` only to players who have not already received one.

---
