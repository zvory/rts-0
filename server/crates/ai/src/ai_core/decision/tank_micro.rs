//! The live Jeff's Tank micro.
//!
//! A Tank's cannon reaches 5 tiles while it moves and ramps to 14 over three seconds standing
//! still, and any path movement resets the ramp. A Tank that drives into a parked Tank's reach
//! therefore takes free shots over the ~9 tiles it needs before it can even stop, then for three
//! more seconds while it charges. Against human players who park their Tanks, Jeff lost most of
//! its Tanks exactly that way: pushes marching into a parked line, single reinforcements walking
//! into it, and home defense driving to a park point inside its reach.
//!
//! This module runs after every other system has issued its orders for the decision:
//! - it remembers where enemy Tanks stood and whether they were parked;
//! - it cuts any Tank order whose path enters a parked enemy Tank's reach at the reach edge, so
//!   the Tank stops and charges outside instead of driving in;
//! - a Tank hit hard by fire it cannot answer stops and charges when the shooter is in sight and
//!   the local Tank odds are even, and otherwise backs out in short reverse hops that keep its
//!   front armor toward the threat;
//! - with more Tanks than a parked enemy group, a Scout Car smokes a cloud inside the group's
//!   reach; the Tanks drive into it, stop and charge unseen, and fight at full range once it
//!   lifts (`smoke_assault.rs`).
//!
//! Only the live `jeffs_ai` runs it; the `jeffs_ai_pre_tank_micro` freeze keeps the old behavior.

use super::geometry::{clamp_to_map, dist2, normalized_direction, squared};
use super::*;

mod smoke_assault;
#[cfg(test)]
mod tests;

pub(crate) use smoke_assault::SmokeAssault;

pub(super) fn uses_tank_micro(profile_id: &str) -> bool {
    profile_id == JEFFS_AI_ID
}

/// Updates the live Jeff's Tank memory before any system decides.
pub(super) fn observe_live(profile_id: &str, observation: &AiObservation, micro: &mut TankMicro) {
    if uses_tank_micro(profile_id) {
        observe(observation, micro);
    }
}

/// Runs the live Jeff's Tank governor over a finished decision and records its trace notes.
pub(super) fn finish(
    decision: &mut AiDecision,
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    memory: &mut AiDecisionMemory,
) {
    if uses_tank_micro(decision.profile_id) {
        decision.trace.notes = govern(
            observation,
            map_analysis,
            &mut memory.tank_micro,
            &mut decision.commands,
            &mut decision.intents,
        );
    }
}

const TANK_BASE_RANGE_TILES: f32 = 5.0;
const TANK_FULL_RANGE_TILES: f32 = 14.0;
const TANK_RANGE_RAMP_TICKS: u32 = config::TICK_HZ * 3;
/// A parked enemy Tank's reach as the governor treats it: full range plus a margin for body
/// radii and for the enemy creeping forward.
pub(super) const REACH_TILES: f32 = TANK_FULL_RANGE_TILES + 1.0;
/// A cut order stops this far outside the reach.
const CLIP_STANDOFF_TILES: f32 = 0.5;
/// A Tank this close to its stop point is left standing rather than re-ordered.
const ARRIVED_TILES: f32 = 0.75;
/// A Tank already standing is not moved to a stop point closer than this.
const ARRIVED_IDLE_TILES: f32 = 2.0;
/// Enemy Tanks not seen for this long are forgotten, unless their spot is seen empty sooner.
const MEMORY_TICKS: u32 = config::TICK_HZ * 20;
/// Parked enemy Tanks are remembered this long: a parked line usually stays put.
const PARKED_MEMORY_TICKS: u32 = config::TICK_HZ * 60;
/// An enemy Tank this far from where it was last seen moved.
const STILL_TILES: f32 = 0.2;
/// An estimated unseen shooter is kept this long.
const GHOST_TICKS: u32 = config::TICK_HZ * 15;
/// Where an unseen shooter is assumed to stand, from the Tank it hit.
const GHOST_DISTANCE_TILES: f32 = 13.0;
/// HP lost between two decisions that counts as a hit worth reacting to (a cannon shell deals 60
/// to the front, a Panzerfaust rocket or Anti-Tank Gun more).
const HEAVY_HIT_HP: u32 = 40;
/// Reverse hops stay under the simulation's 3-tile limit for driving backwards, so the hull keeps
/// facing the threat.
const REVERSE_HOP_TILES: f32 = 2.75;
const REACTION_HOLD_TICKS: u32 = config::TICK_HZ * 2;
const REVERSE_TICKS: u32 = config::TICK_HZ * 3;
/// Enemy Tanks this close to a hit Tank count against it.
const ODDS_RADIUS_TILES: f32 = 16.0;
/// Our Tanks this close to a hit Tank count for it.
const FRIEND_RADIUS_TILES: f32 = 10.0;
/// Remembered enemy Tanks seen this recently count in the odds.
const ODDS_MEMORY_TICKS: u32 = config::TICK_HZ * 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct SeenTank {
    pub(super) pos: (i32, i32),
    pub(super) hp: u32,
    /// Standing still when last seen, so it is charged or will be within three seconds.
    pub(super) parked: bool,
    pub(super) still_since: u32,
    pub(super) last_seen: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Ghost {
    pub(super) pos: (i32, i32),
    pub(super) tick: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct OwnTank {
    pos: (i32, i32),
    hp: u32,
    still_since: u32,
    /// Last movement since the previous decision, in world pixels.
    heading: Option<(i32, i32)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OverrideKind {
    /// Stand still and charge: the shooter is in sight and the odds are even.
    Hold,
    /// Back out of an unseen or out-ranging shooter's reach.
    Reverse { to: (i32, i32), threat: (i32, i32) },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Override {
    kind: OverrideKind,
    until: u32,
    issued: bool,
}

/// The live Jeff's Tank micro state, kept across decisions.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct TankMicro {
    pub(super) enemy_tanks: BTreeMap<u32, SeenTank>,
    pub(super) ghosts: Vec<Ghost>,
    own: BTreeMap<u32, OwnTank>,
    /// HP each own Tank lost since the previous decision.
    recent_damage: BTreeMap<u32, u32>,
    overrides: BTreeMap<u32, Override>,
    pub(super) assault: Option<SmokeAssault>,
    pub(super) last_assault_end: Option<u32>,
    /// This decision's trace notes.
    pub(super) notes: Vec<String>,
    /// Where each Scout Car leading a march was last sent.
    scout_leads: BTreeMap<u32, (i32, i32)>,
    /// Where unseen fire proved the enemy sees, until when.
    assumed_vision: Vec<((i32, i32), u32)>,
}

/// A parked enemy Tank's (or an estimated unseen shooter's) reach.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Zone {
    pub(super) center: (f32, f32),
    pub(super) radius: f32,
    pub(super) tank: Option<u32>,
}

pub(super) fn world(pos: (i32, i32)) -> (f32, f32) {
    (pos.0 as f32, pos.1 as f32)
}

pub(super) fn stored(point: (f32, f32)) -> (i32, i32) {
    (point.0.round() as i32, point.1.round() as i32)
}

fn sight_px(kind: EntityKind, tile_size: f32) -> f32 {
    rts_rules::balance::unit_stats(kind)
        .map(|stats| stats.sight_tiles as f32)
        .unwrap_or(8.0)
        * tile_size
}

pub(super) fn in_smoke(observation: &AiObservation, point: (f32, f32)) -> bool {
    let tile_size = observation.map.tile_size as f32;
    observation.smokes.iter().any(|cloud| {
        dist2(cloud.x, cloud.y, point.0, point.1) <= squared(cloud.radius_tiles * tile_size)
    })
}

/// Updates enemy Tank memory, estimated shooters, and own Tank state before any system decides.
pub(super) fn observe(observation: &AiObservation, micro: &mut TankMicro) {
    let tick = observation.tick;
    let tile_size = observation.map.tile_size as f32;
    let mut visible = BTreeSet::new();
    for enemy in observation
        .visible_enemies
        .iter()
        .filter(|enemy| enemy.kind == EntityKind::Tank && enemy.hp > 0)
    {
        visible.insert(enemy.id);
        let pos = stored((enemy.x, enemy.y));
        let moving = enemy.state == AiEntityState::Move;
        match micro.enemy_tanks.get_mut(&enemy.id) {
            Some(seen) => {
                let moved = dist2(world(seen.pos).0, world(seen.pos).1, enemy.x, enemy.y)
                    > squared(STILL_TILES * tile_size);
                if moving || moved {
                    seen.still_since = tick;
                }
                seen.parked = !moving;
                seen.pos = pos;
                seen.hp = enemy.hp;
                seen.last_seen = tick;
            }
            None => {
                micro.enemy_tanks.insert(
                    enemy.id,
                    SeenTank {
                        pos,
                        hp: enemy.hp,
                        parked: !moving,
                        // An idle Tank seen for the first time has been standing a while.
                        still_since: if enemy.state == AiEntityState::Idle {
                            tick.saturating_sub(PARKED_ZONE_TICKS)
                        } else {
                            tick
                        },
                        last_seen: tick,
                    },
                );
            }
        }
    }

    // Our units that are not inside smoke see their surroundings; a remembered Tank whose spot
    // is in that sight but is no longer there has moved or died.
    let watchers: Vec<((f32, f32), f32)> = observation
        .owned
        .iter()
        .filter(|unit| unit.kind.is_unit() && unit.is_complete && unit.hp > 0)
        .filter(|unit| !in_smoke(observation, (unit.x, unit.y)))
        .map(|unit| ((unit.x, unit.y), sight_px(unit.kind, tile_size) - tile_size))
        .collect();
    let spot_seen_empty = |point: (f32, f32)| {
        !in_smoke(observation, point)
            && watchers.iter().any(|(watcher, sight)| {
                *sight > 0.0 && dist2(watcher.0, watcher.1, point.0, point.1) <= squared(*sight)
            })
    };
    micro.enemy_tanks.retain(|id, seen| {
        visible.contains(id)
            || (tick.saturating_sub(seen.last_seen)
                <= if seen.parked {
                    PARKED_MEMORY_TICKS
                } else {
                    MEMORY_TICKS
                }
                && !spot_seen_empty(world(seen.pos)))
    });
    micro.ghosts.retain(|ghost| {
        tick.saturating_sub(ghost.tick) <= GHOST_TICKS && !spot_seen_empty(world(ghost.pos))
    });

    micro.recent_damage.clear();
    let mut alive = BTreeSet::new();
    for tank in observation
        .owned
        .iter()
        .filter(|unit| unit.kind == EntityKind::Tank && unit.is_complete && unit.hp > 0)
    {
        alive.insert(tank.id);
        let pos = stored((tank.x, tank.y));
        match micro.own.get_mut(&tank.id) {
            Some(own) => {
                if own.hp > tank.hp {
                    micro.recent_damage.insert(tank.id, own.hp - tank.hp);
                }
                let moved = dist2(world(own.pos).0, world(own.pos).1, tank.x, tank.y)
                    > squared(0.05 * tile_size);
                if moved {
                    own.heading = Some((pos.0 - own.pos.0, pos.1 - own.pos.1));
                }
                if moved || tank.state == AiEntityState::Move {
                    own.still_since = tick;
                }
                own.pos = pos;
                own.hp = tank.hp;
            }
            None => {
                micro.own.insert(
                    tank.id,
                    OwnTank {
                        pos,
                        hp: tank.hp,
                        still_since: tick,
                        heading: None,
                    },
                );
            }
        }
    }
    micro.own.retain(|id, _| alive.contains(id));
    micro
        .overrides
        .retain(|id, held| alive.contains(id) && held.until > tick);
    micro.assumed_vision.retain(|(_, until)| *until > tick);
}

/// An enemy Tank counts as parked, and its reach as a threat, once it has stood this long.
const PARKED_ZONE_TICKS: u32 = config::TICK_HZ * 3 / 2;
/// Where unseen fire proved the enemy sees, it is assumed to see this far around the hit Tanks.
const ASSUMED_VISION_TILES: f32 = 8.0;
const ASSUMED_VISION_TICKS: u32 = config::TICK_HZ * 20;
/// An estimated shooter is assumed to see this far.
const GHOST_SIGHT_TILES: f32 = 15.0;
/// The governor checks a Tank's path every this many tiles.
const SAMPLE_TILES: f32 = 0.5;

fn entity_sight_tiles(kind: EntityKind) -> f32 {
    rts_rules::balance::unit_stats(kind)
        .map(|stats| stats.sight_tiles as f32)
        .or_else(|| rts_rules::balance::building_stats(kind).map(|stats| stats.sight_tiles as f32))
        .unwrap_or(8.0)
}

/// What the enemy can see: its visible units' and buildings' sight, remembered Tanks' own sight,
/// estimated shooters, and areas where unseen fire proved it sees. A parked Tank only threatens
/// what its side sees.
pub(super) struct Vision {
    sources: Vec<((f32, f32), f32)>,
    /// Visible enemy units other than Tanks, and their sight: the spotters a parked line relies
    /// on beyond its own 9 tiles.
    spotters: Vec<(u32, (f32, f32), f32)>,
}

impl Vision {
    pub(super) fn sees(&self, point: (f32, f32)) -> bool {
        self.sources
            .iter()
            .any(|(center, radius)| dist2(center.0, center.1, point.0, point.1) <= squared(*radius))
    }

    /// The one visible spotter whose sight alone exposes the point, if killing it would blind the
    /// enemy there.
    pub(super) fn sole_spotter(&self, point: (f32, f32)) -> Option<u32> {
        let seen_by = |center: &(f32, f32), radius: f32| {
            dist2(center.0, center.1, point.0, point.1) <= squared(radius)
        };
        let spotters: Vec<u32> = self
            .spotters
            .iter()
            .filter(|(_, center, radius)| seen_by(center, *radius))
            .map(|(id, _, _)| *id)
            .collect();
        let other = self.sources.iter().any(|(center, radius)| {
            seen_by(center, *radius)
                && !self
                    .spotters
                    .iter()
                    .any(|(_, spot, sight)| spot == center && sight == radius)
        });
        (spotters.len() == 1 && !other).then(|| spotters[0])
    }

    #[cfg(test)]
    pub(super) fn everywhere() -> Self {
        Self {
            sources: vec![((0.0, 0.0), 1.0e7)],
            spotters: Vec::new(),
        }
    }

    #[cfg(test)]
    pub(super) fn around(sources: Vec<((f32, f32), f32)>) -> Self {
        Self {
            sources,
            spotters: Vec::new(),
        }
    }
}

pub(super) fn enemy_vision(observation: &AiObservation, micro: &TankMicro) -> Vision {
    let tick = observation.tick;
    let tile_size = observation.map.tile_size as f32;
    let visible: BTreeSet<u32> = observation.visible_enemies.iter().map(|e| e.id).collect();
    let mut sources: Vec<((f32, f32), f32)> = observation
        .visible_enemies
        .iter()
        .filter(|enemy| enemy.hp > 0)
        .map(|enemy| {
            (
                (enemy.x, enemy.y),
                entity_sight_tiles(enemy.kind) * tile_size,
            )
        })
        .collect();
    let spotters = observation
        .visible_enemies
        .iter()
        .filter(|enemy| enemy.hp > 0 && enemy.kind.is_unit() && enemy.kind != EntityKind::Tank)
        .map(|enemy| {
            (
                enemy.id,
                (enemy.x, enemy.y),
                entity_sight_tiles(enemy.kind) * tile_size,
            )
        })
        .collect();
    sources.extend(
        micro
            .enemy_tanks
            .iter()
            .filter(|(id, _)| !visible.contains(id))
            .map(|(_, seen)| {
                (
                    world(seen.pos),
                    entity_sight_tiles(EntityKind::Tank) * tile_size,
                )
            }),
    );
    sources.extend(
        micro
            .ghosts
            .iter()
            .map(|ghost| (world(ghost.pos), GHOST_SIGHT_TILES * tile_size)),
    );
    sources.extend(
        micro
            .assumed_vision
            .iter()
            .filter(|(_, until)| *until > tick)
            .map(|(center, _)| (world(*center), ASSUMED_VISION_TILES * tile_size)),
    );
    Vision { sources, spotters }
}

/// Parked enemy Tanks' reach and estimated unseen shooters' reach.
pub(super) fn zones(observation: &AiObservation, micro: &TankMicro) -> Vec<Zone> {
    let radius = REACH_TILES * observation.map.tile_size as f32;
    micro
        .enemy_tanks
        .iter()
        .filter(|(_, seen)| {
            seen.parked && seen.last_seen.saturating_sub(seen.still_since) >= PARKED_ZONE_TICKS
        })
        .map(|(id, seen)| Zone {
            center: world(seen.pos),
            radius,
            tank: Some(*id),
        })
        .chain(micro.ghosts.iter().map(|ghost| Zone {
            center: world(ghost.pos),
            radius,
            tank: None,
        }))
        .collect()
}

/// The reach a point lies in, if the enemy also sees it.
fn danger_zone(zones: &[Zone], vision: &Vision, point: (f32, f32)) -> Option<usize> {
    let zone = zones.iter().position(|zone| {
        dist2(point.0, point.1, zone.center.0, zone.center.1) <= squared(zone.radius)
    })?;
    vision.sees(point).then_some(zone)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Clip {
    /// The path stays out of every seen reach.
    Clear,
    /// The path enters a seen reach: stop here instead, outside it.
    StopAt {
        point: (f32, f32),
        zone: usize,
        danger_at: (f32, f32),
    },
    /// Already inside a seen reach and the order goes deeper: stand still instead.
    Stand { zone: usize },
}

/// How the governor treats a Tank ordered from `from` toward `to`.
pub(super) fn clip_path(
    zones: &[Zone],
    vision: &Vision,
    from: (f32, f32),
    to: (f32, f32),
    tile_size: f32,
) -> Clip {
    if let Some(index) = danger_zone(zones, vision, from) {
        let zone = zones[index];
        let now = dist2(from.0, from.1, zone.center.0, zone.center.1).sqrt();
        let then = dist2(to.0, to.1, zone.center.0, zone.center.1).sqrt();
        return if then < now - 0.5 * tile_size {
            Clip::Stand { zone: index }
        } else {
            Clip::Clear
        };
    }
    let length = dist2(from.0, from.1, to.0, to.1).sqrt();
    if length < 1.0 {
        return Clip::Clear;
    }
    let at = |t: f32| (from.0 + (to.0 - from.0) * t, from.1 + (to.1 - from.1) * t);
    let steps = (length / (SAMPLE_TILES * tile_size)).ceil().max(1.0) as usize;
    for step in 1..=steps {
        let t = step as f32 / steps as f32;
        if let Some(zone) = danger_zone(zones, vision, at(t)) {
            let back = ((CLIP_STANDOFF_TILES + SAMPLE_TILES) * tile_size / length).min(t);
            return Clip::StopAt {
                point: at((t - back).max(0.0)),
                zone,
                danger_at: at(t),
            };
        }
    }
    Clip::Clear
}

fn units_mut(command: &mut Command) -> Option<&mut Vec<u32>> {
    match command {
        Command::Move { units, .. }
        | Command::FormationMove { units, .. }
        | Command::AttackMove { units, .. }
        | Command::ClearObstacleArea { units, .. }
        | Command::Attack { units, .. }
        | Command::AttackTankTrapCluster { units, .. }
        | Command::Deconstruct { units, .. }
        | Command::SetupAntiTankGuns { units, .. }
        | Command::TearDownAntiTankGuns { units }
        | Command::UseAbility { units, .. }
        | Command::ArtilleryFire { units, .. }
        | Command::RecastAbility { units, .. }
        | Command::SetAutocast { units, .. }
        | Command::Gather { units, .. }
        | Command::Build { units, .. }
        | Command::PointTanks { units, .. }
        | Command::Stop { units }
        | Command::HoldPosition { units, .. } => Some(units),
        _ => None,
    }
}

/// A moving Tank's current cannon range, from how long it has stood still.
fn current_range_tiles(micro: &TankMicro, tank: u32, tick: u32) -> f32 {
    let still = micro
        .own
        .get(&tank)
        .map_or(0, |own| tick.saturating_sub(own.still_since))
        .min(TANK_RANGE_RAMP_TICKS);
    TANK_BASE_RANGE_TILES
        + (TANK_FULL_RANGE_TILES - TANK_BASE_RANGE_TILES) * still as f32
            / TANK_RANGE_RAMP_TICKS as f32
}

fn enemy_start(observation: &AiObservation) -> Option<(f32, f32)> {
    let own_team = observation
        .players
        .iter()
        .find(|player| player.id == observation.player_id)
        .map(|player| player.team_id);
    observation
        .players
        .iter()
        .find(|player| {
            player.id != observation.player_id
                && player.is_alive
                && own_team.is_none_or(|team| player.team_id != team)
        })
        .map(|player| tile_center(player.start_tile, observation.map.tile_size))
}

/// A reverse hop away from `threat`, at most 3 tiles so the Tank backs up, on open ground.
fn reverse_hop(
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    from: (f32, f32),
    threat: (f32, f32),
) -> Option<(f32, f32)> {
    let away = normalized_direction(threat, from)?;
    let tile_size = observation.map.tile_size as f32;
    for degrees in [0.0_f32, 25.0, -25.0, 50.0, -50.0] {
        let (sin, cos) = degrees.to_radians().sin_cos();
        let direction = (away.0 * cos - away.1 * sin, away.0 * sin + away.1 * cos);
        for tiles in [REVERSE_HOP_TILES, REVERSE_HOP_TILES * 0.6] {
            let point = clamp_to_map(
                (
                    from.0 + direction.0 * tiles * tile_size,
                    from.1 + direction.1 * tiles * tile_size,
                ),
                observation.map,
            );
            if dist2(point.0, point.1, from.0, from.1) < squared(tile_size)
                || !defense::defensive_position_is_open(observation, map_analysis, point.0, point.1)
            {
                continue;
            }
            return Some(point);
        }
    }
    None
}

fn is_anti_armor(kind: EntityKind) -> bool {
    matches!(
        kind,
        EntityKind::Tank | EntityKind::AntiTankGun | EntityKind::Panzerfaust
    )
}

/// A local force may drive into a parked group's reach when it outnumbers it and has this much
/// more HP.
const SUPERIOR_HP_RATIO: f32 = 1.1;
/// A group that can shoot back still backs out when outnumbered, or even in numbers but this many
/// times weaker in charge...
const OUTNUMBERED_RATIO: f32 = 1.2;
/// ...and its nearest shooter is at least this far, so a short reverse leaves the reach.
const ESCAPE_FROM_TILES: f32 = 11.0;
/// Tanks this close to a hit Tank share its reaction, so a group never splits under fire.
const GROUP_TILES: f32 = 6.0;

/// Decides how the groups around Tanks that just took heavy hits react: all of a group holds and
/// charges when one of them sees a shooter in reach and the local odds are even, otherwise all of
/// it backs out of the reach.
fn react_to_hits(
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    micro: &mut TankMicro,
    tanks: &BTreeMap<u32, &AiEntitySummary>,
    claimed: &BTreeSet<u32>,
) {
    let tick = observation.tick;
    let tile_size = observation.map.tile_size as f32;
    let vision = enemy_vision(observation, micro);
    let backing_out = |micro: &TankMicro, id: &u32| {
        micro
            .overrides
            .get(id)
            .is_some_and(|held| matches!(held.kind, OverrideKind::Reverse { .. }))
    };
    let free: Vec<u32> = tanks
        .keys()
        .copied()
        .filter(|id| !claimed.contains(id) && !backing_out(micro, id))
        .collect();
    let mut hit: Vec<u32> = micro
        .recent_damage
        .iter()
        .filter(|(id, damage)| **damage >= HEAVY_HIT_HP && free.contains(id))
        .map(|(id, _)| *id)
        .collect();
    let mut decided: BTreeSet<u32> = BTreeSet::new();
    while let Some(seed) = hit.pop() {
        if decided.contains(&seed) {
            continue;
        }
        // The hit Tank and every free Tank linked to it within GROUP_TILES.
        let mut group = vec![seed];
        let mut index = 0;
        while index < group.len() {
            let Some(member) = tanks.get(&group[index]) else {
                index += 1;
                continue;
            };
            let (mx, my) = (member.x, member.y);
            for id in &free {
                if group.contains(id) || decided.contains(id) {
                    continue;
                }
                if let Some(other) = tanks.get(id) {
                    if dist2(other.x, other.y, mx, my) <= squared(GROUP_TILES * tile_size) {
                        group.push(*id);
                    }
                }
            }
            index += 1;
        }
        decided.extend(group.iter().copied());
        let members: Vec<&AiEntitySummary> = group
            .iter()
            .filter_map(|id| tanks.get(id).copied())
            .collect();
        if members.is_empty() {
            continue;
        }
        let center = (
            members.iter().map(|tank| tank.x).sum::<f32>() / members.len() as f32,
            members.iter().map(|tank| tank.y).sum::<f32>() / members.len() as f32,
        );
        let answerable = members.iter().any(|tank| {
            let range = current_range_tiles(micro, tank.id, tick);
            observation.visible_enemies.iter().any(|enemy| {
                enemy.hp > 0
                    && ((enemy.kind == EntityKind::Tank
                        && dist2(enemy.x, enemy.y, tank.x, tank.y)
                            <= squared((TANK_FULL_RANGE_TILES + 0.5) * tile_size))
                        || (is_anti_armor(enemy.kind)
                            && dist2(enemy.x, enemy.y, tank.x, tank.y)
                                <= squared((range.max(6.0) + 0.5) * tile_size)))
            })
        });
        // The enemy saw these Tanks somehow; remember that it sees here.
        if !vision.sees(center) {
            micro
                .assumed_vision
                .push((stored(center), tick + ASSUMED_VISION_TICKS));
        }
        // Charged Tanks outrange moving or settling ones, so each Tank counts half plus half its
        // stationary charge, on both sides.
        let charge = |still_since: u32| {
            0.5 + 0.5
                * (tick.saturating_sub(still_since) as f32 / TANK_RANGE_RAMP_TICKS as f32).min(1.0)
        };
        let friends: f32 = tanks
            .values()
            .filter(|other| {
                dist2(other.x, other.y, center.0, center.1)
                    <= squared(FRIEND_RADIUS_TILES * tile_size)
            })
            .map(|other| {
                micro
                    .own
                    .get(&other.id)
                    .map_or(0.5, |own| charge(own.still_since))
            })
            .sum();
        let near_center = |p: (f32, f32)| {
            dist2(p.0, p.1, center.0, center.1) <= squared(ODDS_RADIUS_TILES * tile_size)
        };
        let foes: f32 = micro
            .enemy_tanks
            .values()
            .filter(|seen| tick.saturating_sub(seen.last_seen) <= ODDS_MEMORY_TICKS)
            .filter(|seen| near_center(world(seen.pos)))
            .map(|seen| {
                if seen.parked {
                    charge(seen.still_since)
                } else {
                    0.5
                }
            })
            .sum::<f32>()
            + micro
                .ghosts
                .iter()
                .filter(|ghost| near_center(world(ghost.pos)))
                .count() as f32;
        let damage: u32 = group
            .iter()
            .filter_map(|id| micro.recent_damage.get(id))
            .sum();
        // Backing up inside a close fight only loses the charge while the shots keep coming, so a
        // group that can shoot back holds unless it is clearly outnumbered with the shooters near
        // the edge of their reach, where a short reverse gets it out.
        let nearest_shooter_tiles = observation
            .visible_enemies
            .iter()
            .filter(|enemy| is_anti_armor(enemy.kind) && enemy.hp > 0)
            .map(|enemy| dist2(enemy.x, enemy.y, center.0, center.1).sqrt() / tile_size)
            .fold(f32::MAX, f32::min);
        let escapable = nearest_shooter_tiles >= ESCAPE_FROM_TILES;
        let friend_count = tanks
            .values()
            .filter(|other| {
                dist2(other.x, other.y, center.0, center.1)
                    <= squared(FRIEND_RADIUS_TILES * tile_size)
            })
            .count();
        let foe_count = micro
            .enemy_tanks
            .values()
            .filter(|seen| tick.saturating_sub(seen.last_seen) <= ODDS_MEMORY_TICKS)
            .filter(|seen| near_center(world(seen.pos)))
            .count()
            + micro
                .ghosts
                .iter()
                .filter(|ghost| near_center(world(ghost.pos)))
                .count();
        // A Tank that just stopped charges in three seconds, so numbers decide; charge only breaks
        // a tie.
        let outnumbered = foe_count > friend_count
            || (foe_count == friend_count && foes > friends * OUTNUMBERED_RATIO);
        if answerable && (!outnumbered || !escapable) {
            micro.notes.push(format!(
                "hit group={group:?} damage={damage} answerable strength={friends:.1} vs {foes:.1} -> hold"
            ));
            for tank in &members {
                micro.overrides.insert(
                    tank.id,
                    Override {
                        kind: OverrideKind::Hold,
                        until: tick + REACTION_HOLD_TICKS,
                        issued: tank.state != AiEntityState::Move,
                    },
                );
            }
            continue;
        }

        // Where the shots came from: a visible anti-armor unit, a remembered enemy Tank, an
        // estimated shooter, or else ahead of the way the group was driving (it drove into the
        // reach) or toward the enemy base.
        let nearest = |points: &mut dyn Iterator<Item = (f32, f32)>, max_tiles: f32| {
            points
                .filter(|p| dist2(p.0, p.1, center.0, center.1) <= squared(max_tiles * tile_size))
                .min_by(|a, b| {
                    dist2(a.0, a.1, center.0, center.1)
                        .total_cmp(&dist2(b.0, b.1, center.0, center.1))
                })
        };
        let mut threat = nearest(
            &mut observation
                .visible_enemies
                .iter()
                .filter(|enemy| is_anti_armor(enemy.kind) && enemy.hp > 0)
                .map(|enemy| (enemy.x, enemy.y)),
            ODDS_RADIUS_TILES,
        )
        .or_else(|| {
            nearest(
                &mut micro
                    .enemy_tanks
                    .values()
                    .filter(|seen| tick.saturating_sub(seen.last_seen) <= ODDS_MEMORY_TICKS)
                    .map(|seen| world(seen.pos)),
                REACH_TILES + 3.0,
            )
        })
        .or_else(|| {
            nearest(
                &mut micro.ghosts.iter().map(|ghost| world(ghost.pos)),
                REACH_TILES + 3.0,
            )
        });
        if threat.is_none() {
            let heading = members
                .iter()
                .filter_map(|tank| micro.own.get(&tank.id).and_then(|own| own.heading))
                .map(|(dx, dy)| (dx as f32, dy as f32))
                .reduce(|a, b| (a.0 + b.0, a.1 + b.1))
                .and_then(|sum| normalized_direction((0.0, 0.0), sum))
                .or_else(|| {
                    enemy_start(observation).and_then(|base| normalized_direction(center, base))
                });
            if let Some(direction) = heading {
                let ghost = clamp_to_map(
                    (
                        center.0 + direction.0 * GHOST_DISTANCE_TILES * tile_size,
                        center.1 + direction.1 * GHOST_DISTANCE_TILES * tile_size,
                    ),
                    observation.map,
                );
                micro.ghosts.push(Ghost {
                    pos: stored(ghost),
                    tick,
                });
                threat = Some(ghost);
            }
        }
        let Some(threat) = threat else {
            continue;
        };
        micro.notes.push(format!(
            "hit group={group:?} damage={damage} answerable={answerable} strength={friends:.1} vs {foes:.1} -> reverse from ({:.0},{:.0})",
            threat.0 / tile_size,
            threat.1 / tile_size
        ));
        for tank in &members {
            if let Some(to) = reverse_hop(observation, map_analysis, (tank.x, tank.y), threat) {
                micro.overrides.insert(
                    tank.id,
                    Override {
                        kind: OverrideKind::Reverse {
                            to: stored(to),
                            threat: stored(threat),
                        },
                        until: tick + REVERSE_TICKS,
                        issued: false,
                    },
                );
            }
        }
    }
}

/// Advances reverse hops that landed: another hop while still inside the threat's reach,
/// otherwise a short hold so other systems do not drive straight back in.
fn advance_overrides(
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    micro: &mut TankMicro,
    tanks: &BTreeMap<u32, &AiEntitySummary>,
) {
    let tick = observation.tick;
    let tile_size = observation.map.tile_size as f32;
    let reach = REACH_TILES * tile_size;
    let ids: Vec<u32> = micro.overrides.keys().copied().collect();
    for id in ids {
        let Some(tank) = tanks.get(&id) else {
            continue;
        };
        let Some(held) = micro.overrides.get(&id).copied() else {
            continue;
        };
        let OverrideKind::Reverse { to, threat } = held.kind else {
            continue;
        };
        let target = world(to);
        let landed = dist2(tank.x, tank.y, target.0, target.1)
            <= squared(ARRIVED_TILES * tile_size)
            || (held.issued && tank.state != AiEntityState::Move);
        if !landed {
            continue;
        }
        let threat_point = world(threat);
        let still_in_reach =
            dist2(tank.x, tank.y, threat_point.0, threat_point.1) <= squared(reach);
        let next = still_in_reach
            .then(|| reverse_hop(observation, map_analysis, (tank.x, tank.y), threat_point))
            .flatten();
        let replacement = match next {
            Some(next) => Override {
                kind: OverrideKind::Reverse {
                    to: stored(next),
                    threat,
                },
                until: tick + REVERSE_TICKS,
                issued: false,
            },
            None => Override {
                kind: OverrideKind::Hold,
                until: tick + REACTION_HOLD_TICKS,
                issued: tank.state != AiEntityState::Move,
            },
        };
        micro.overrides.insert(id, replacement);
    }
}

/// A Scout Car marching with Tanks goes this far ahead of the front Tank, so its 15-tile sight
/// finds a parked line before the Tanks walk into its reach (Tanks see only 9).
const SCOUT_LEAD_TILES: f32 = 4.0;
/// Moving Tanks this close to a Scout Car are the march it leads.
const SCOUT_ESCORT_TILES: f32 = 9.0;
/// The lead order is re-issued only when the lead point moves this far.
const SCOUT_RELEAD_TILES: f32 = 2.0;
/// Only Tanks this far from home are on a march; nearer they are forming up.
const SCOUT_LEAD_FROM_HOME_TILES: f32 = 16.0;
/// The marching Tanks' headings must agree this well (summed unit headings per Tank).
const SCOUT_LEAD_AGREEMENT: f32 = 0.75;

/// Sends each Scout Car marching with two or more moving Tanks ahead of them, stopping it just
/// outside any known reach.
fn lead_scouts(
    observation: &AiObservation,
    micro: &mut TankMicro,
    tanks: &BTreeMap<u32, &AiEntitySummary>,
    zones: &[Zone],
    claimed: &mut BTreeSet<u32>,
    extra: &mut Vec<Command>,
) {
    let tile_size = observation.map.tile_size as f32;
    let home = tile_center(observation.own_start_tile, observation.map.tile_size);
    let scouts: Vec<&AiEntitySummary> = observation
        .owned
        .iter()
        .filter(|unit| unit.kind == EntityKind::ScoutCar && unit.is_complete && unit.hp > 0)
        .filter(|unit| !claimed.contains(&unit.id))
        .collect();
    let mut leading = BTreeMap::new();
    for scout in scouts {
        let marching: Vec<&AiEntitySummary> = tanks
            .values()
            .copied()
            .filter(|tank| tank.state == AiEntityState::Move && !claimed.contains(&tank.id))
            .filter(|tank| {
                dist2(tank.x, tank.y, home.0, home.1)
                    >= squared(SCOUT_LEAD_FROM_HOME_TILES * tile_size)
            })
            .filter(|tank| {
                dist2(tank.x, tank.y, scout.x, scout.y) <= squared(SCOUT_ESCORT_TILES * tile_size)
            })
            .collect();
        if marching.len() < 2 {
            continue;
        }
        // A march: the Tanks head the same way. Tanks shuffling into slots do not.
        let headings: Vec<(f32, f32)> = marching
            .iter()
            .filter_map(|tank| micro.own.get(&tank.id).and_then(|own| own.heading))
            .filter_map(|(dx, dy)| normalized_direction((0.0, 0.0), (dx as f32, dy as f32)))
            .collect();
        if headings.len() < 2 {
            continue;
        }
        let sum = headings
            .iter()
            .fold((0.0, 0.0), |acc, h| (acc.0 + h.0, acc.1 + h.1));
        if (sum.0 * sum.0 + sum.1 * sum.1).sqrt() < SCOUT_LEAD_AGREEMENT * headings.len() as f32 {
            continue;
        }
        let Some(direction) = normalized_direction((0.0, 0.0), sum) else {
            continue;
        };
        let Some(front) = marching.iter().max_by(|a, b| {
            (a.x * direction.0 + a.y * direction.1)
                .total_cmp(&(b.x * direction.0 + b.y * direction.1))
                .then_with(|| a.id.cmp(&b.id))
        }) else {
            continue;
        };
        let mut lead = clamp_to_map(
            (
                front.x + direction.0 * SCOUT_LEAD_TILES * tile_size,
                front.y + direction.1 * SCOUT_LEAD_TILES * tile_size,
            ),
            observation.map,
        );
        for zone in zones {
            if dist2(lead.0, lead.1, zone.center.0, zone.center.1) < squared(zone.radius) {
                if let Some(out) = normalized_direction(zone.center, lead) {
                    let edge = zone.radius + 0.2 * tile_size;
                    lead = (zone.center.0 + out.0 * edge, zone.center.1 + out.1 * edge);
                }
            }
        }
        claimed.insert(scout.id);
        let previous = micro.scout_leads.get(&scout.id).copied().map(world);
        let moved_on = previous.is_none_or(|previous| {
            dist2(previous.0, previous.1, lead.0, lead.1) > squared(SCOUT_RELEAD_TILES * tile_size)
        });
        let idle_away = scout.state != AiEntityState::Move
            && dist2(scout.x, scout.y, lead.0, lead.1) > squared(3.0 * tile_size);
        if moved_on || idle_away {
            extra.push(Command::Move {
                units: vec![scout.id],
                x: lead.0,
                y: lead.1,
                queued: false,
            });
            leading.insert(scout.id, stored(lead));
        } else if let Some(previous) = micro.scout_leads.get(&scout.id) {
            leading.insert(scout.id, *previous);
        }
    }
    micro.scout_leads = leading;
}

/// Rewrites the decision's commands for the live Jeff's Tanks. Runs last.
pub(super) fn govern(
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    micro: &mut TankMicro,
    commands: &mut Vec<Command>,
    intents: &mut [AiIntent],
) -> Vec<String> {
    micro.notes.clear();
    let tick = observation.tick;
    let tile_size = observation.map.tile_size as f32;
    let tanks: BTreeMap<u32, &AiEntitySummary> = observation
        .owned
        .iter()
        .filter(|unit| unit.kind == EntityKind::Tank && unit.is_complete && unit.hp > 0)
        .map(|unit| (unit.id, unit))
        .collect();
    let zones = zones(observation, micro);
    let vision = enemy_vision(observation, micro);
    if !micro.enemy_tanks.is_empty()
        || !micro.ghosts.is_empty()
        || !micro.overrides.is_empty()
        || micro.assault.is_some()
    {
        let parked = micro
            .enemy_tanks
            .values()
            .filter(|seen| seen.parked)
            .count();
        let note = format!(
            "memory enemy_tanks={} parked={} ghosts={} overrides={} assault={}",
            micro.enemy_tanks.len(),
            parked,
            micro.ghosts.len(),
            micro.overrides.len(),
            micro
                .assault
                .as_ref()
                .map_or("none".to_string(), SmokeAssault::label)
        );
        micro.notes.push(note);
    }

    // Where each Tank's movement order would take it, and which reach it would run into.
    let mut blocked_by: BTreeMap<u32, usize> = BTreeMap::new();
    for command in commands.iter() {
        let (units, destination) = match command {
            Command::Move { units, x, y, .. } | Command::AttackMove { units, x, y, .. } => {
                (units, Some((*x, *y)))
            }
            Command::FormationMove { units, points, .. } => (units, points.last().copied()),
            _ => continue,
        };
        let Some(destination) = destination else {
            continue;
        };
        for id in units {
            let Some(tank) = tanks.get(id) else {
                continue;
            };
            match clip_path(&zones, &vision, (tank.x, tank.y), destination, tile_size) {
                Clip::StopAt { zone, .. } | Clip::Stand { zone } => {
                    blocked_by.insert(*id, zone);
                }
                Clip::Clear => {}
            }
        }
    }

    let mut claimed: BTreeSet<u32> = BTreeSet::new();
    let mut extra: Vec<Command> = Vec::new();
    smoke_assault::step(
        observation,
        map_analysis,
        micro,
        &zones,
        &tanks,
        &blocked_by,
        &mut extra,
        &mut claimed,
    );

    advance_overrides(observation, map_analysis, micro, &tanks);
    react_to_hits(observation, map_analysis, micro, &tanks, &claimed);
    let override_ids: Vec<u32> = micro
        .overrides
        .keys()
        .copied()
        .filter(|id| !claimed.contains(id))
        .collect();
    // A held Tank is only pinned: its other orders that keep it standing (a direct attack on a
    // target in range, holding) stand, which keeps the defense's focus fire, while movement is
    // dropped below.
    let mut pinned: BTreeSet<u32> = BTreeSet::new();
    for id in override_ids {
        if !tanks.contains_key(&id) {
            continue;
        }
        let Some(held) = micro.overrides.get_mut(&id) else {
            continue;
        };
        match held.kind {
            OverrideKind::Hold => {
                pinned.insert(id);
            }
            OverrideKind::Reverse { to, .. } => {
                claimed.insert(id);
                if !held.issued {
                    let point = world(to);
                    extra.push(Command::Move {
                        units: vec![id],
                        x: point.0,
                        y: point.1,
                        queued: false,
                    });
                    held.issued = true;
                }
            }
        }
    }

    lead_scouts(observation, micro, &tanks, &zones, &mut claimed, &mut extra);

    // Claimed units take only the orders above.
    let mut governed: BTreeSet<u32> = claimed.clone();
    for command in commands.iter_mut() {
        if let Some(units) = units_mut(command) {
            units.retain(|id| !claimed.contains(id));
        }
    }

    // Cut the rest at the edge of the reach the enemy sees. Reactions may have added vision.
    let vision = enemy_vision(observation, micro);
    // A force that clearly outnumbers the enemy Tanks around a reach may drive into it: the smoke
    // assault planned above is preferred, but without smoke a larger force still wins the trade.
    let superior = |micro: &TankMicro, id: u32, zone: &Zone| -> bool {
        let Some(tank) = tanks.get(&id) else {
            return false;
        };
        let own: Vec<&AiEntitySummary> = tanks
            .values()
            .copied()
            .filter(|other| !claimed.contains(&other.id))
            .filter(|other| {
                dist2(other.x, other.y, tank.x, tank.y) <= squared(FRIEND_RADIUS_TILES * tile_size)
            })
            .collect();
        let own_hp: u32 = own.iter().map(|other| other.hp).sum();
        let near = |p: (f32, f32)| {
            dist2(p.0, p.1, zone.center.0, zone.center.1) <= squared(ODDS_RADIUS_TILES * tile_size)
        };
        // Every remembered Tank counts: a parked line out of sight is still there.
        let foes: Vec<&SeenTank> = micro
            .enemy_tanks
            .values()
            .filter(|seen| near(world(seen.pos)))
            .collect();
        let ghosts = micro
            .ghosts
            .iter()
            .filter(|ghost| near(world(ghost.pos)))
            .count();
        let enemy_count = foes.len() + ghosts;
        let enemy_hp: u32 = foes.iter().map(|seen| seen.hp).sum::<u32>() + ghosts as u32 * 292;
        own.len() > enemy_count && own_hp as f32 >= enemy_hp as f32 * SUPERIOR_HP_RATIO
    };
    let mut clipped: Vec<Command> = Vec::new();
    // Pinned Tanks kept by an order that keeps them standing.
    let mut standing: BTreeSet<u32> = BTreeSet::new();
    for command in commands.iter() {
        if let Command::HoldPosition { units, .. } = command {
            standing.extend(units.iter().copied());
        }
    }
    for command in commands.iter_mut() {
        let target_point = |target: u32| {
            observation
                .visible_enemies
                .iter()
                .find(|enemy| enemy.id == target)
                .map(|enemy| (enemy.x, enemy.y))
        };
        let (units, destination, chases_target) = match command {
            Command::Move { units, x, y, .. } | Command::AttackMove { units, x, y, .. } => {
                (units, Some((*x, *y)), false)
            }
            Command::FormationMove { units, points, .. } => {
                let last = points.last().copied();
                (units, last, false)
            }
            Command::Attack { units, target, .. } => (units, target_point(*target), true),
            _ => continue,
        };
        let Some(destination) = destination else {
            continue;
        };
        let mut keep = Vec::with_capacity(units.len());
        for id in units.drain(..) {
            let Some(tank) = tanks.get(&id) else {
                keep.push(id);
                continue;
            };
            let from = (tank.x, tank.y);
            // A direct attack only moves the Tank when the target is beyond its current range.
            if chases_target
                && dist2(from.0, from.1, destination.0, destination.1)
                    <= squared((current_range_tiles(micro, id, tick) + 0.5) * tile_size)
            {
                keep.push(id);
                standing.insert(id);
                continue;
            }
            if pinned.contains(&id) {
                governed.insert(id);
                continue;
            }
            let clip = clip_path(&zones, &vision, from, destination, tile_size);
            if let Clip::StopAt { zone, .. } | Clip::Stand { zone } = clip {
                if superior(micro, id, &zones[zone]) {
                    micro.notes.push(format!(
                        "allow tank={id} superior into reach_of={:?}",
                        zones[zone].tank
                    ));
                    keep.push(id);
                    continue;
                }
            }
            match clip {
                Clip::Clear => keep.push(id),
                Clip::StopAt {
                    point,
                    zone,
                    danger_at,
                } => {
                    governed.insert(id);
                    // Blind them first: when one visible spotter alone exposes the way in and it
                    // is in range, shoot it instead of waiting outside.
                    let range = current_range_tiles(micro, id, tick);
                    let spotter = vision.sole_spotter(danger_at).and_then(|spotter| {
                        observation
                            .visible_enemies
                            .iter()
                            .find(|enemy| enemy.id == spotter)
                            .filter(|enemy| {
                                dist2(enemy.x, enemy.y, from.0, from.1)
                                    <= squared((range - 0.5) * tile_size)
                            })
                            .map(|enemy| enemy.id)
                    });
                    if let Some(spotter) = spotter {
                        micro
                            .notes
                            .push(format!("blind tank={id} spotter={spotter}"));
                        clipped.push(Command::Attack {
                            units: vec![id],
                            target: spotter,
                            queued: false,
                        });
                        continue;
                    }
                    micro.notes.push(format!(
                        "clip tank={id} stop=({:.0},{:.0}) reach_of={}",
                        point.0 / tile_size,
                        point.1 / tile_size,
                        zones[zone]
                            .tank
                            .map_or("estimated".to_string(), |tank| tank.to_string())
                    ));
                    // Every move costs a standing Tank its charge, so a short step is not worth it.
                    let arrived = if tank.state == AiEntityState::Move {
                        ARRIVED_TILES
                    } else {
                        ARRIVED_IDLE_TILES
                    };
                    if dist2(from.0, from.1, point.0, point.1) <= squared(arrived * tile_size) {
                        if tank.state == AiEntityState::Move {
                            clipped.push(Command::HoldPosition {
                                units: vec![id],
                                queued: false,
                            });
                        }
                    } else {
                        clipped.push(Command::Move {
                            units: vec![id],
                            x: point.0,
                            y: point.1,
                            queued: false,
                        });
                    }
                }
                Clip::Stand { zone } => {
                    governed.insert(id);
                    micro.notes.push(format!(
                        "stand tank={id} inside_reach_of={}",
                        zones[zone]
                            .tank
                            .map_or("estimated".to_string(), |tank| tank.to_string())
                    ));
                    if tank.state == AiEntityState::Move {
                        clipped.push(Command::HoldPosition {
                            units: vec![id],
                            queued: false,
                        });
                    }
                }
            }
        }
        *units = keep;
    }

    // A pinned Tank still moving with nothing keeping it standing is stopped once.
    for id in &pinned {
        if standing.contains(id) {
            continue;
        }
        if tanks
            .get(id)
            .is_some_and(|tank| tank.state == AiEntityState::Move)
        {
            clipped.push(Command::HoldPosition {
                units: vec![*id],
                queued: false,
            });
        }
    }
    commands.retain_mut(|command| units_mut(command).is_none_or(|units| !units.is_empty()));
    commands.extend(clipped);
    commands.extend(extra);

    // The live controller drops repeated stage orders for staged units. Units the governor took
    // over (backing out, smoke assault, Scout Car lead) must get every order it gives them; cut
    // stage orders stay staged, so their repeats are still dropped and a Tank standing at its
    // stop point is not shuffled about.
    let _ = governed;
    for intent in intents.iter_mut() {
        if let AiIntent::Stage { units } = intent {
            units.retain(|id| !claimed.contains(id));
        }
    }
    std::mem::take(&mut micro.notes)
}
