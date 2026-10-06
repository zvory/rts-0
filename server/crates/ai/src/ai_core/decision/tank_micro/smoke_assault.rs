//! Smoke assaults: with more Tanks than a parked enemy Tank group, a Scout Car smokes a cloud
//! inside the group's reach, the Tanks drive into it, stop and charge unseen, and fight at full
//! range once it lifts.
//!
//! A parked Tank beats one that drives into its reach. Tanks that arrive hidden and already
//! charged remove that edge, and the larger side then wins the stationary trade. Smoke blocks
//! sight and fire both ways, so nobody shoots while it lasts; the clouds sit between our Tanks'
//! start line and the enemy, which also covers the short drive in.

use super::*;
use crate::ai_core::observation::AiSmokeSummary;
use rts_rules::faction::AbilityKind;

/// Where the cloud centre sits from the group's nearest Tank.
const CLOUD_DISTANCE_TILES: f32 = 12.0;
/// Assault slots must be within this range of the group's nearest Tank.
const SLOT_MAX_RANGE_TILES: f32 = 13.4;
/// The Scout Car waits outside the reach to cast, then spots from just inside full range.
const SCOUT_WAIT_TILES: f32 = REACH_TILES + 1.0;
const SCOUT_SPOT_TILES: f32 = 13.8;
const SMOKE_CAST_RANGE_TILES: f32 = 13.5;
/// Enemy Tanks this close to each other form one group.
const GROUP_LINK_TILES: f32 = 7.0;
/// Our Tanks this far from the group's nearest Tank can join an assault.
const JOIN_MIN_TILES: f32 = REACH_TILES - 2.0;
const JOIN_MAX_TILES: f32 = REACH_TILES + 10.0;
/// Remembered group Tanks must have been seen this recently.
const TARGET_MEMORY_TICKS: u32 = config::TICK_HZ * 12;
/// Enemy Tanks this close to the group count against the assault.
const SUPPORT_TILES: f32 = 16.0;
const MIN_TANK_HP: u32 = 90;
/// Our Tank HP must exceed the enemy group's by this factor.
const HP_ADVANTAGE: f32 = 1.1;
const PREPARE_TIMEOUT_TICKS: u32 = config::TICK_HZ * 8;
const CAST_CONFIRM_TICKS: u32 = 24;
/// The Tanks start driving before the first cast, so the cloud lands as they reach the reach edge
/// and its five seconds cover the drive in and the three-second charge.
const CAST_DELAY_TICKS: u32 = 24;
/// Without a cast this long after the Tanks started, the assault is called off.
const CAST_GIVE_UP_TICKS: u32 = CAST_DELAY_TICKS + 36;
const ENTER_TIMEOUT_TICKS: u32 = config::TICK_HZ * 4;
const FIGHT_CLEAR_TICKS: u32 = config::TICK_HZ * 3;
const FIGHT_TIMEOUT_TICKS: u32 = config::TICK_HZ * 25;
const WITHDRAW_TIMEOUT_TICKS: u32 = config::TICK_HZ * 6;
/// No assault runs longer than this, whatever its phase.
const ASSAULT_TIMEOUT_TICKS: u32 = config::TICK_HZ * 60;
pub(super) const COOLDOWN_TICKS: u32 = config::TICK_HZ * 12;
/// Re-cast over the same spot when a lone cloud has this few ticks left and Tanks are inside.
const EXTEND_WHEN_EXPIRES_IN: u16 = 45;
const SLOT_ARRIVED_TILES: f32 = 0.6;
const START_READY_TILES: f32 = 2.5;
/// Tanks this close to their start line still join a push that goes without them lined up.
const STRAGGLER_TILES: f32 = 4.0;
/// After this long, the push goes as soon as the lined-up Tanks outnumber the group.
const PARTIAL_GO_TICKS: u32 = config::TICK_HZ * 2;
/// Slot offsets in tiles (toward us, sideways) for a 2-tile cloud; a Tank is 1.6 x 0.9 tiles.
const BASE_SLOTS: [(f32, f32); 5] = [
    (0.0, 0.0),
    (0.0, 1.3),
    (0.0, -1.3),
    (0.9, 0.65),
    (0.9, -0.65),
];
/// Slot offsets for a 4-tile Smoke Plus cloud.
const PLUS_SLOTS: [(f32, f32); 11] = [
    (0.0, 0.0),
    (0.0, 1.4),
    (0.0, -1.4),
    (0.0, 2.8),
    (0.0, -2.8),
    (1.2, 0.7),
    (1.2, -0.7),
    (1.2, 2.1),
    (1.2, -2.1),
    (-1.2, 0.7),
    (-1.2, -0.7),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    /// Tanks line up outside the reach; the Scout Car comes within cast range.
    Prepare,
    /// The Tanks drive into their slots; the first cloud is cast shortly after they start, the
    /// rest one per decision.
    Enter,
    /// Inside the smoke: stand still and charge.
    Hidden,
    /// The smoke lifted: stand and fire at full range while the Scout Car spots.
    Fight,
    /// Outnumbered or the smoke failed: back out of the reach.
    Withdraw,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Slot {
    point: (i32, i32),
    start: (i32, i32),
    cloud: usize,
}

impl SmokeAssault {
    pub(super) fn label(&self) -> String {
        format!("{:?}:{}", self.phase, self.slots.len())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SmokeAssault {
    phase: Phase,
    phase_since: u32,
    started: u32,
    scout: u32,
    targets: BTreeSet<u32>,
    anchor: (i32, i32),
    /// Unit direction from the anchor toward our side, times 1000.
    toward_us: (i32, i32),
    clouds: Vec<(i32, i32)>,
    cast: usize,
    first_cast_tick: Option<u32>,
    extended: bool,
    slots: BTreeMap<u32, Slot>,
    clear_since: Option<u32>,
}

fn store_direction(direction: (f32, f32)) -> (i32, i32) {
    (
        (direction.0 * 1000.0).round() as i32,
        (direction.1 * 1000.0).round() as i32,
    )
}

fn load_direction(value: (i32, i32)) -> (f32, f32) {
    (value.0 as f32 / 1000.0, value.1 as f32 / 1000.0)
}

fn offset_point(
    origin: (f32, f32),
    u: (f32, f32),
    along_tiles: f32,
    side_tiles: f32,
    ts: f32,
) -> (f32, f32) {
    let perp = (-u.1, u.0);
    (
        origin.0 + (u.0 * along_tiles + perp.0 * side_tiles) * ts,
        origin.1 + (u.1 * along_tiles + perp.1 * side_tiles) * ts,
    )
}

fn smoke_ready(observation: &AiObservation, scout: u32) -> bool {
    observation.ability_states.iter().any(|ability| {
        ability.entity_id == scout
            && ability.kind == AbilityKind::Smoke
            && ability.cooldown_left == 0
            && ability.remaining_uses.is_none_or(|uses| uses > 0)
            && ability
                .available_tick
                .is_none_or(|tick| tick <= observation.tick)
            && ability
                .lockout_until_tick
                .is_none_or(|tick| tick <= observation.tick)
    })
}

/// Both charges stored: nothing is recharging.
fn smoke_charges_full(observation: &AiObservation, scout: u32) -> bool {
    smoke_ready(observation, scout)
        && observation.ability_states.iter().any(|ability| {
            ability.entity_id == scout
                && ability.kind == AbilityKind::Smoke
                && ability.charge_recharge_left.is_none_or(|left| left == 0)
        })
}

fn smoke_unlocked(observation: &AiObservation) -> bool {
    observation
        .owned
        .iter()
        .any(|entity| entity.kind == EntityKind::EngineeringComplex && entity.is_complete)
}

fn cloud_present(observation: &AiObservation, center: (f32, f32)) -> Option<&AiSmokeSummary> {
    let tile_size = observation.map.tile_size as f32;
    observation
        .smokes
        .iter()
        .find(|cloud| dist2(cloud.x, cloud.y, center.0, center.1) <= squared(1.0 * tile_size))
}

/// Parked enemy Tanks seen recently, grouped by single linkage.
fn enemy_groups(observation: &AiObservation, micro: &TankMicro) -> Vec<Vec<u32>> {
    let tick = observation.tick;
    let link = squared(GROUP_LINK_TILES * observation.map.tile_size as f32);
    let parked: Vec<(u32, (f32, f32))> = micro
        .enemy_tanks
        .iter()
        .filter(|(_, seen)| {
            seen.parked && tick.saturating_sub(seen.last_seen) <= TARGET_MEMORY_TICKS
        })
        .map(|(id, seen)| (*id, world(seen.pos)))
        .collect();
    let mut group_of: Vec<usize> = (0..parked.len()).collect();
    fn root(group_of: &mut [usize], mut index: usize) -> usize {
        while group_of[index] != index {
            group_of[index] = group_of[group_of[index]];
            index = group_of[index];
        }
        index
    }
    for a in 0..parked.len() {
        for b in (a + 1)..parked.len() {
            let (pa, pb) = (parked[a].1, parked[b].1);
            if dist2(pa.0, pa.1, pb.0, pb.1) <= link {
                let (ra, rb) = (root(&mut group_of, a), root(&mut group_of, b));
                group_of[ra.max(rb)] = ra.min(rb);
            }
        }
    }
    let mut groups: BTreeMap<usize, Vec<u32>> = BTreeMap::new();
    for (index, (id, _)) in parked.iter().enumerate() {
        let group = root(&mut group_of, index);
        groups.entry(group).or_default().push(*id);
    }
    groups.into_values().collect()
}

/// Enemy Tanks the assault has to beat: the group plus remembered Tanks and estimated shooters
/// near it. Returns (count, total HP).
fn opposition(
    observation: &AiObservation,
    micro: &TankMicro,
    group: &BTreeSet<u32>,
) -> (usize, u32) {
    let tick = observation.tick;
    let ts = observation.map.tile_size as f32;
    let positions: Vec<(f32, f32)> = group
        .iter()
        .filter_map(|id| micro.enemy_tanks.get(id).map(|seen| world(seen.pos)))
        .collect();
    let near_group = |p: (f32, f32)| {
        positions
            .iter()
            .any(|g| dist2(g.0, g.1, p.0, p.1) <= squared(SUPPORT_TILES * ts))
    };
    let mut count = 0;
    let mut hp = 0;
    for (id, seen) in &micro.enemy_tanks {
        let in_group = group.contains(id);
        let recent = tick.saturating_sub(seen.last_seen) <= TARGET_MEMORY_TICKS;
        if in_group || (recent && near_group(world(seen.pos))) {
            count += 1;
            hp += seen.hp;
        }
    }
    let visible_extra = observation
        .visible_enemies
        .iter()
        .filter(|enemy| enemy.kind == EntityKind::Tank && enemy.hp > 0)
        .filter(|enemy| !micro.enemy_tanks.contains_key(&enemy.id))
        .filter(|enemy| near_group((enemy.x, enemy.y)))
        .count();
    let ghosts = micro
        .ghosts
        .iter()
        .filter(|ghost| near_group(world(ghost.pos)))
        .count();
    count += visible_extra + ghosts;
    hp += (visible_extra + ghosts) as u32 * 292;
    (count, hp)
}

fn plan(
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    micro: &TankMicro,
    zones: &[Zone],
    tanks: &BTreeMap<u32, &AiEntitySummary>,
    blocked_by: &BTreeMap<u32, usize>,
) -> Option<SmokeAssault> {
    if !smoke_unlocked(observation) {
        return None;
    }
    let tick = observation.tick;
    let ts = observation.map.tile_size as f32;
    let smoke_radius_tiles = if observation.upgrades.contains(&UpgradeKind::SmokePlus) {
        4.0
    } else {
        2.0
    };
    let slot_pattern: &[(f32, f32)] = if smoke_radius_tiles > 2.0 {
        &PLUS_SLOTS
    } else {
        &BASE_SLOTS
    };

    for group in enemy_groups(observation, micro) {
        let group: BTreeSet<u32> = group.into_iter().collect();
        let group_points: Vec<(u32, (f32, f32))> = group
            .iter()
            .filter_map(|id| micro.enemy_tanks.get(id).map(|seen| (*id, world(seen.pos))))
            .collect();
        let nearest_group_tiles = |p: (f32, f32)| {
            group_points
                .iter()
                .map(|(_, g)| dist2(g.0, g.1, p.0, p.1))
                .fold(f32::MAX, f32::min)
                .sqrt()
                / ts
        };
        let candidates: Vec<&AiEntitySummary> = tanks
            .values()
            .copied()
            .filter(|tank| tank.hp >= MIN_TANK_HP && !micro.overrides.contains_key(&tank.id))
            .filter(|tank| {
                let d = nearest_group_tiles((tank.x, tank.y));
                (JOIN_MIN_TILES..=JOIN_MAX_TILES).contains(&d)
            })
            .collect();
        if candidates.len() < 2 {
            continue;
        }
        let blocked = candidates.iter().any(|tank| {
            blocked_by
                .get(&tank.id)
                .and_then(|zone| zones.get(*zone))
                .and_then(|zone| zone.tank)
                .is_some_and(|enemy| group.contains(&enemy))
        });
        if !blocked {
            continue;
        }
        let (enemy_count, enemy_hp) = opposition(observation, micro, &group);
        if candidates.len() < enemy_count + 1 {
            continue;
        }

        let our_center = {
            let n = candidates.len() as f32;
            (
                candidates.iter().map(|t| t.x).sum::<f32>() / n,
                candidates.iter().map(|t| t.y).sum::<f32>() / n,
            )
        };
        let Some(&(_, anchor)) = group_points.iter().min_by(|a, b| {
            dist2(a.1 .0, a.1 .1, our_center.0, our_center.1)
                .total_cmp(&dist2(b.1 .0, b.1 .1, our_center.0, our_center.1))
                .then_with(|| a.0.cmp(&b.0))
        }) else {
            continue;
        };
        let Some(u) = normalized_direction(anchor, our_center) else {
            continue;
        };
        let cloud_center = offset_point(anchor, u, CLOUD_DISTANCE_TILES, 0.0, ts);
        let scout_wait = offset_point(anchor, u, SCOUT_WAIT_TILES, 0.0, ts);
        let Some(scout) = observation
            .owned
            .iter()
            .filter(|unit| unit.kind == EntityKind::ScoutCar && unit.is_complete && unit.hp > 0)
            .filter(|unit| smoke_ready(observation, unit.id))
            .filter(|unit| !micro.overrides.contains_key(&unit.id))
            .min_by(|a, b| {
                dist2(a.x, a.y, scout_wait.0, scout_wait.1)
                    .total_cmp(&dist2(b.x, b.y, scout_wait.0, scout_wait.1))
                    .then_with(|| a.id.cmp(&b.id))
            })
        else {
            continue;
        };
        if dist2(scout.x, scout.y, scout_wait.0, scout_wait.1) > squared(30.0 * ts) {
            continue;
        }

        // One cloud when it holds enough Tanks, keeping the second charge to recast it before it
        // lifts; two side by side only when the enemy needs more Tanks than one cloud holds.
        let two_clouds =
            enemy_count + 1 > slot_pattern.len() && smoke_charges_full(observation, scout.id);
        let clouds: Vec<(f32, f32)> = if two_clouds {
            let side = smoke_radius_tiles - 0.2;
            vec![
                offset_point(anchor, u, CLOUD_DISTANCE_TILES, side, ts),
                offset_point(anchor, u, CLOUD_DISTANCE_TILES, -side, ts),
            ]
        } else {
            vec![cloud_center]
        };
        let mut open_slots: Vec<(usize, (f32, f32), f32)> = Vec::new();
        for (cloud_index, cloud) in clouds.iter().enumerate() {
            let side_of_cloud = if two_clouds {
                if cloud_index == 0 {
                    smoke_radius_tiles - 0.2
                } else {
                    -(smoke_radius_tiles - 0.2)
                }
            } else {
                0.0
            };
            for (along, side) in slot_pattern {
                let point = offset_point(*cloud, u, *along, *side, ts);
                let point = clamp_to_map(point, observation.map);
                if dist2(point.0, point.1, anchor.0, anchor.1) > squared(SLOT_MAX_RANGE_TILES * ts)
                    || !defense::defensive_position_is_open(
                        observation,
                        map_analysis,
                        point.0,
                        point.1,
                    )
                {
                    continue;
                }
                open_slots.push((cloud_index, point, side_of_cloud + side));
            }
        }
        if open_slots.len() < enemy_count + 1 {
            continue;
        }
        // Nearest Tanks first, each to its nearest free slot.
        let mut order = candidates.clone();
        order.sort_by(|a, b| {
            dist2(a.x, a.y, cloud_center.0, cloud_center.1)
                .total_cmp(&dist2(b.x, b.y, cloud_center.0, cloud_center.1))
                .then_with(|| a.id.cmp(&b.id))
        });
        let mut slots = BTreeMap::new();
        for tank in order {
            let Some(best) = (0..open_slots.len()).min_by(|a, b| {
                let pa = open_slots[*a].1;
                let pb = open_slots[*b].1;
                dist2(tank.x, tank.y, pa.0, pa.1).total_cmp(&dist2(tank.x, tank.y, pb.0, pb.1))
            }) else {
                break;
            };
            let (cloud, point, side) = open_slots.remove(best);
            // A Tank already just outside the reach starts where it stands: walking it to a
            // computed point can cost a long detour around water or buildings.
            let here = nearest_group_tiles((tank.x, tank.y));
            let start = if (REACH_TILES..=REACH_TILES + 6.0).contains(&here) {
                (tank.x, tank.y)
            } else {
                clamp_to_map(
                    offset_point(anchor, u, REACH_TILES + 0.5, side, ts),
                    observation.map,
                )
            };
            let start =
                if defense::defensive_position_is_open(observation, map_analysis, start.0, start.1)
                {
                    start
                } else {
                    (tank.x, tank.y)
                };
            slots.insert(
                tank.id,
                Slot {
                    point: stored(point),
                    start: stored(start),
                    cloud,
                },
            );
        }
        let assault_hp: u32 = slots
            .keys()
            .filter_map(|id| tanks.get(id).map(|tank| tank.hp))
            .sum();
        if slots.len() < enemy_count + 1 || (assault_hp as f32) < enemy_hp as f32 * HP_ADVANTAGE {
            continue;
        }
        return Some(SmokeAssault {
            phase: Phase::Prepare,
            phase_since: tick,
            started: tick,
            scout: scout.id,
            targets: group,
            anchor: stored(anchor),
            toward_us: store_direction(u),
            clouds: clouds.into_iter().map(stored).collect(),
            cast: 0,
            first_cast_tick: None,
            extended: false,
            slots,
            clear_since: None,
        });
    }
    None
}

fn order_move(out: &mut Vec<Command>, unit: &AiEntitySummary, point: (f32, f32), tolerance: f32) {
    if dist2(unit.x, unit.y, point.0, point.1) > squared(tolerance)
        && unit.state != AiEntityState::Move
    {
        out.push(Command::Move {
            units: vec![unit.id],
            x: point.0,
            y: point.1,
            queued: false,
        });
    }
}

fn order_hold(out: &mut Vec<Command>, unit: &AiEntitySummary) {
    if unit.state == AiEntityState::Move {
        out.push(Command::HoldPosition {
            units: vec![unit.id],
            queued: false,
        });
    }
}

fn cast(out: &mut Vec<Command>, scout: u32, point: (f32, f32)) {
    out.push(Command::UseAbility {
        ability: AbilityKind::Smoke,
        units: vec![scout],
        x: Some(point.0),
        y: Some(point.1),
        queued: false,
    });
}

/// Runs one decision of an assault. Returns false when it ended.
fn run(
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    micro: &mut TankMicro,
    assault: &mut SmokeAssault,
    tanks: &BTreeMap<u32, &AiEntitySummary>,
    out: &mut Vec<Command>,
    claimed: &mut BTreeSet<u32>,
) -> bool {
    let tick = observation.tick;
    let ts = observation.map.tile_size as f32;
    let anchor = world(assault.anchor);
    let u = load_direction(assault.toward_us);
    assault.slots.retain(|id, _| tanks.contains_key(id));
    if assault.slots.is_empty() {
        return false;
    }
    claimed.extend(assault.slots.keys().copied());
    let scout = observation
        .owned
        .iter()
        .find(|unit| unit.id == assault.scout && unit.hp > 0);
    if let Some(scout) = scout {
        claimed.insert(scout.id);
    }
    let members: Vec<(&AiEntitySummary, Slot)> = assault
        .slots
        .iter()
        .filter_map(|(id, slot)| tanks.get(id).map(|tank| (*tank, *slot)))
        .collect();
    let elapsed = tick.saturating_sub(assault.phase_since);
    if tick.saturating_sub(assault.started) >= ASSAULT_TIMEOUT_TICKS {
        return false;
    }
    let (enemy_count, _) = opposition(observation, micro, &assault.targets);
    let set_phase = |assault: &mut SmokeAssault, phase: Phase| {
        assault.phase = phase;
        assault.phase_since = tick;
    };

    match assault.phase {
        Phase::Prepare => {
            let start_gap = |tank: &AiEntitySummary, slot: &Slot| {
                let start = world(slot.start);
                dist2(tank.x, tank.y, start.0, start.1).sqrt() / ts
            };
            let lined: BTreeSet<u32> = members
                .iter()
                .filter(|(tank, slot)| start_gap(tank, slot) <= START_READY_TILES)
                .map(|(tank, _)| tank.id)
                .collect();
            let timed_out = elapsed >= PREPARE_TIMEOUT_TICKS;
            let failure_note = || {
                let gaps: Vec<String> = members
                    .iter()
                    .map(|(tank, slot)| format!("{}:{:.1}", tank.id, start_gap(tank, slot)))
                    .collect();
                let scout_note = scout.map(|scout| {
                    let wait = offset_point(anchor, u, SCOUT_WAIT_TILES, 0.0, ts);
                    format!(
                        "{:.1} tiles from its wait point, smoke ready {}",
                        dist2(scout.x, scout.y, wait.0, wait.1).sqrt() / ts,
                        smoke_ready(observation, scout.id)
                    )
                });
                format!(
                    "assault prepare failed members={} enemies={} elapsed={} start_gaps={:?} scout={:?}",
                    members.len(),
                    enemy_count,
                    elapsed,
                    gaps,
                    scout_note
                )
            };
            if enemy_count == 0
                || members.len() < enemy_count + 1
                || (timed_out && lined.len() <= enemy_count)
            {
                micro.notes.push(failure_note());
                return false;
            }
            for (tank, slot) in &members {
                order_move(out, tank, world(slot.start), ts);
            }
            let scout_wait = offset_point(anchor, u, SCOUT_WAIT_TILES, 0.0, ts);
            let Some(scout) = scout else {
                return false;
            };
            order_move(out, scout, scout_wait, ts);
            // Once the lined-up Tanks outnumber the group, stragglers are not waited for: every
            // second of waiting is one the enemy can use to reinforce or reposition.
            let all_lined = lined.len() == members.len();
            let enough_lined = lined.len() > enemy_count && elapsed >= PARTIAL_GO_TICKS;
            let in_cast_range = assault.clouds.iter().all(|cloud| {
                let cloud = world(*cloud);
                dist2(scout.x, scout.y, cloud.0, cloud.1) <= squared(SMOKE_CAST_RANGE_TILES * ts)
            });
            if (all_lined || enough_lined) && in_cast_range && smoke_ready(observation, scout.id) {
                // Stragglers close behind still reach the cloud before it lifts; the rest go
                // back to the regular orders.
                let going: BTreeSet<u32> = members
                    .iter()
                    .filter(|(tank, slot)| start_gap(tank, slot) <= STRAGGLER_TILES)
                    .map(|(tank, _)| tank.id)
                    .collect();
                if going.len() < members.len() {
                    micro.notes.push(format!(
                        "assault goes with {} of {} Tanks",
                        going.len(),
                        members.len()
                    ));
                    assault.slots.retain(|id, _| going.contains(id));
                }
                for (tank, slot) in members.iter().filter(|(tank, _)| going.contains(&tank.id)) {
                    order_move(out, tank, world(slot.point), SLOT_ARRIVED_TILES * ts);
                }
                set_phase(assault, Phase::Enter);
            } else if timed_out {
                micro.notes.push(failure_note());
                return false;
            }
            true
        }
        Phase::Enter | Phase::Hidden => {
            let Some(scout) = scout else {
                set_phase(assault, Phase::Withdraw);
                return true;
            };
            let landed: Vec<bool> = assault
                .clouds
                .iter()
                .map(|cloud| cloud_present(observation, world(*cloud)).is_some())
                .collect();
            let cast_failed = assault.first_cast_tick.map_or(
                assault.phase == Phase::Enter && elapsed >= CAST_GIVE_UP_TICKS,
                |cast_tick| !landed[0] && tick.saturating_sub(cast_tick) >= CAST_CONFIRM_TICKS,
            );
            if cast_failed && assault.phase == Phase::Enter {
                set_phase(assault, Phase::Withdraw);
                return true;
            }
            // The first cloud once the Tanks have driven a moment, then one cast per decision.
            let cast_due = assault.cast > 0 || elapsed >= CAST_DELAY_TICKS;
            if cast_due && assault.cast < assault.clouds.len() && smoke_ready(observation, scout.id)
            {
                cast(out, scout.id, world(assault.clouds[assault.cast]));
                if assault.cast == 0 {
                    assault.first_cast_tick = Some(tick);
                }
                assault.cast += 1;
            }
            // A lone cloud about to lift over Tanks still charging is cast again.
            if assault.clouds.len() == 1 && !assault.extended && smoke_ready(observation, scout.id)
            {
                if let Some(cloud) = cloud_present(observation, world(assault.clouds[0])) {
                    if cloud.expires_in <= EXTEND_WHEN_EXPIRES_IN {
                        cast(out, scout.id, (cloud.x, cloud.y));
                        assault.extended = true;
                    }
                }
            }
            order_move(
                out,
                scout,
                offset_point(anchor, u, SCOUT_WAIT_TILES, 0.0, ts),
                ts,
            );
            let mut all_in = true;
            for (tank, slot) in &members {
                let point = world(slot.point);
                let inside =
                    dist2(tank.x, tank.y, point.0, point.1) <= squared(SLOT_ARRIVED_TILES * ts);
                if inside || assault.phase == Phase::Hidden {
                    order_hold(out, tank);
                } else {
                    all_in = false;
                    order_move(out, tank, point, SLOT_ARRIVED_TILES * ts);
                }
            }
            if assault.phase == Phase::Enter && (all_in || elapsed >= ENTER_TIMEOUT_TICKS) {
                set_phase(assault, Phase::Hidden);
            }
            let covered = members
                .iter()
                .any(|(tank, _)| in_smoke(observation, (tank.x, tank.y)));
            if assault.phase == Phase::Hidden && !covered {
                set_phase(assault, Phase::Fight);
            }
            true
        }
        Phase::Fight => {
            for (tank, _) in &members {
                order_hold(out, tank);
            }
            if let Some(scout) = scout {
                order_move(
                    out,
                    scout,
                    offset_point(anchor, u, SCOUT_SPOT_TILES, 0.0, ts),
                    ts,
                );
            }
            let center = {
                let n = members.len() as f32;
                (
                    members.iter().map(|(t, _)| t.x).sum::<f32>() / n,
                    members.iter().map(|(t, _)| t.y).sum::<f32>() / n,
                )
            };
            let visible_near = observation
                .visible_enemies
                .iter()
                .filter(|enemy| enemy.kind == EntityKind::Tank && enemy.hp > 0)
                .filter(|enemy| {
                    dist2(enemy.x, enemy.y, center.0, center.1) <= squared(SUPPORT_TILES * ts)
                        || dist2(enemy.x, enemy.y, anchor.0, anchor.1)
                            <= squared(SUPPORT_TILES * ts)
                })
                .count();
            if visible_near > members.len() {
                set_phase(assault, Phase::Withdraw);
                return true;
            }
            if visible_near == 0 {
                let since = *assault.clear_since.get_or_insert(tick);
                if tick.saturating_sub(since) >= FIGHT_CLEAR_TICKS {
                    // Won: the group's spots are empty even if nobody can see them.
                    let visible: BTreeSet<u32> =
                        observation.visible_enemies.iter().map(|e| e.id).collect();
                    for target in &assault.targets {
                        if !visible.contains(target) {
                            micro.enemy_tanks.remove(target);
                        }
                    }
                    return false;
                }
            } else {
                assault.clear_since = None;
            }
            elapsed < FIGHT_TIMEOUT_TICKS
        }
        Phase::Withdraw => {
            let reach = squared((REACH_TILES + 0.5) * ts);
            let mut all_out = true;
            for (tank, _) in &members {
                if dist2(tank.x, tank.y, anchor.0, anchor.1) <= reach {
                    all_out = false;
                    if tank.state != AiEntityState::Move {
                        if let Some(hop) =
                            reverse_hop(observation, map_analysis, (tank.x, tank.y), anchor)
                        {
                            out.push(Command::Move {
                                units: vec![tank.id],
                                x: hop.0,
                                y: hop.1,
                                queued: false,
                            });
                        }
                    }
                } else {
                    order_hold(out, tank);
                }
            }
            if let Some(scout) = scout {
                order_move(
                    out,
                    scout,
                    offset_point(anchor, u, SCOUT_WAIT_TILES + 2.0, 0.0, ts),
                    ts,
                );
            }
            !all_out && elapsed < WITHDRAW_TIMEOUT_TICKS
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn step(
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    micro: &mut TankMicro,
    zones: &[Zone],
    tanks: &BTreeMap<u32, &AiEntitySummary>,
    blocked_by: &BTreeMap<u32, usize>,
    out: &mut Vec<Command>,
    claimed: &mut BTreeSet<u32>,
) {
    let tick = observation.tick;
    if micro.assault.is_none()
        && micro
            .last_assault_end
            .is_none_or(|end| tick.saturating_sub(end) >= COOLDOWN_TICKS)
    {
        if let Some(assault) = plan(observation, map_analysis, micro, zones, tanks, blocked_by) {
            for id in assault.slots.keys() {
                micro.overrides.remove(id);
            }
            micro.notes.push(format!(
                "assault start tanks={:?} scout={} targets={:?} clouds={}",
                assault.slots.keys().collect::<Vec<_>>(),
                assault.scout,
                assault.targets,
                assault.clouds.len()
            ));
            micro.assault = Some(assault);
            micro.assaults_started += 1;
        }
    }
    let Some(mut assault) = micro.assault.take() else {
        return;
    };
    let phase = assault.phase;
    if run(
        observation,
        map_analysis,
        micro,
        &mut assault,
        tanks,
        out,
        claimed,
    ) {
        if assault.phase != phase {
            micro
                .notes
                .push(format!("assault {:?} -> {:?}", phase, assault.phase));
        }
        micro.assault = Some(assault);
    } else {
        micro
            .notes
            .push(format!("assault ended in {:?}", assault.phase));
        micro.last_assault_end = Some(tick);
    }
}
