//! Fighting enemy Tanks: each cannon ordered onto a target goes on the enemy Tank in its reach that
//! dies in the fewest shots.
//!
//! Sim facts it works from:
//! - a Tank's cannon reaches 5 tiles moving and 14 after three seconds standing, and every shell
//!   hits.
//! - armor by where the shot comes from relative to the hull: front (within 45 degrees) 1.0x,
//!   side 1.5x, rear (beyond 135 degrees) 1.7x a 60-damage shell. A hull faces the way it last
//!   drove.
//! - an Attack order from elsewhere in the decision may name infantry, a building, or a fresh Tank
//!   while a damaged or side-on one is in reach: killing a Tank a shot sooner takes its cannon out
//!   of the fight a reload sooner.
//!
//! Measured on 300 recorded Tank fights replayed both ways: the enemy loses as much and Jeff loses
//! about 2% less. Two other pieces that won the fights themselves did not survive whole games:
//! holding charged Tanks still against an enemy Tank driving at them (15% fewer Tanks lost in the
//! fights) slowed pushes enough to cost wins and lost two base races on Crossroads, and sending a
//! second Tank up to a firing post on a parked enemy Tank traded about even.

use super::*;

/// A Tank's front armor covers this much either side of its hull's heading; beyond the side arc
/// is the rear.
const FRONT_ARC_DEGREES: f32 = 45.0;
const SIDE_ARC_DEGREES: f32 = 135.0;
/// A cannon shell's damage to a Tank's front; the side takes 1.5x, the rear 1.7x.
const SHELL_DAMAGE: f32 = 60.0;
const SIDE_MULTIPLIER: f32 = 1.5;
const REAR_MULTIPLIER: f32 = 1.7;
/// A new target must die this many shots sooner to be worth turning the turret.
const RETARGET_GAIN_SHOTS: u32 = 1;
/// Enemy Tanks seen this recently still count as the target an order names.
const FIGHT_MEMORY_TICKS: u32 = config::TICK_HZ * 6;

/// An enemy Tank: where it is, whether it is in sight now, its HP, and the way its hull faces if
/// known.
#[derive(Clone, Copy, Debug)]
struct Foe {
    pos: (f32, f32),
    visible: bool,
    hp: u32,
    heading: Option<(f32, f32)>,
}

/// Enemy Tanks: visible ones, and ones seen moments ago.
fn foes(observation: &AiObservation, micro: &TankMicro) -> BTreeMap<u32, Foe> {
    let tick = observation.tick;
    let heading = |seen: &SeenTank| {
        seen.heading
            .and_then(|(dx, dy)| normalized_direction((0.0, 0.0), (dx as f32, dy as f32)))
    };
    let mut out: BTreeMap<u32, Foe> = micro
        .enemy_tanks
        .iter()
        .filter(|(_, seen)| tick.saturating_sub(seen.last_seen) <= FIGHT_MEMORY_TICKS)
        .map(|(id, seen)| {
            (
                *id,
                Foe {
                    pos: world(seen.pos),
                    visible: false,
                    hp: seen.hp,
                    heading: heading(seen),
                },
            )
        })
        .collect();
    for enemy in observation
        .visible_enemies
        .iter()
        .filter(|enemy| enemy.kind == EntityKind::Tank && enemy.hp > 0)
    {
        out.insert(
            enemy.id,
            Foe {
                pos: (enemy.x, enemy.y),
                visible: true,
                hp: enemy.hp,
                heading: micro.enemy_tanks.get(&enemy.id).and_then(heading),
            },
        );
    }
    out
}

fn tiles_between(a: (f32, f32), b: (f32, f32), tile_size: f32) -> f32 {
    dist2(a.0, a.1, b.0, b.1).sqrt() / tile_size
}

/// The angle in degrees between a hull heading and the direction toward `toward` from `from`.
fn off_heading(heading: (f32, f32), from: (f32, f32), toward: (f32, f32)) -> Option<f32> {
    let dir = normalized_direction(from, toward)?;
    let dot = (heading.0 * dir.0 + heading.1 * dir.1).clamp(-1.0, 1.0);
    Some(dot.acos().to_degrees())
}

/// One of our shells' damage on `foe` from `from`, by the armor it hits.
fn shell_damage(foe: &Foe, from: (f32, f32)) -> f32 {
    let multiplier = match foe
        .heading
        .and_then(|heading| off_heading(heading, foe.pos, from))
    {
        Some(angle) if angle <= FRONT_ARC_DEGREES => 1.0,
        Some(angle) if angle <= SIDE_ARC_DEGREES => SIDE_MULTIPLIER,
        Some(_) => REAR_MULTIPLIER,
        // Unknown: assume it faces us.
        None => 1.0,
    };
    SHELL_DAMAGE * multiplier
}

/// Shots `foe` takes to die from `from`.
fn shots_to_kill(foe: &Foe, from: (f32, f32)) -> u32 {
    (foe.hp as f32 / shell_damage(foe, from)).ceil() as u32
}

/// Rewrites the decision's Attack orders on the live Jeff's Tanks: a Tank whose order names
/// infantry, a building or an enemy Tank that takes longer to kill fires instead at the visible
/// enemy Tank in its current reach that dies in the fewest shots, by HP and the armor it shows.
pub(super) fn aim(
    observation: &AiObservation,
    micro: &mut TankMicro,
    tanks: &BTreeMap<u32, &AiEntitySummary>,
    claimed: &BTreeSet<u32>,
    commands: &mut Vec<Command>,
) {
    let tick = observation.tick;
    let ts = observation.map.tile_size as f32;
    let foes = foes(observation, micro);
    let visible: Vec<(u32, Foe)> = foes
        .iter()
        .filter(|(_, foe)| foe.visible)
        .map(|(id, foe)| (*id, *foe))
        .collect();
    if visible.is_empty() {
        return;
    }
    let mut added: Vec<Command> = Vec::new();
    for command in commands.iter_mut() {
        let Command::Attack { units, target, .. } = command else {
            continue;
        };
        let target = *target;
        let mut keep = Vec::with_capacity(units.len());
        for id in units.drain(..) {
            let Some(tank) = tanks.get(&id).filter(|_| !claimed.contains(&id)) else {
                keep.push(id);
                continue;
            };
            let from = (tank.x, tank.y);
            let range = current_range_tiles(micro, id, tick);
            let in_range = |foe: &Foe| tiles_between(from, foe.pos, ts) <= range;
            let current = foes
                .get(&target)
                .filter(|foe| in_range(foe))
                .map(|foe| shots_to_kill(foe, from));
            let best = visible
                .iter()
                .filter(|(_, foe)| in_range(foe))
                .map(|(other, foe)| (*other, shots_to_kill(foe, from)))
                .min_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0)));
            match best {
                Some((best, shots))
                    if best != target
                        && current.is_none_or(|current| shots + RETARGET_GAIN_SHOTS <= current) =>
                {
                    micro.notes.push(format!(
                        "aim tank={id} from {target} to tank {best} ({shots} shots)"
                    ));
                    added.push(Command::Attack {
                        units: vec![id],
                        target: best,
                        queued: false,
                    });
                }
                _ => keep.push(id),
            }
        }
        *units = keep;
    }
    commands.retain_mut(|command| units_mut(command).is_none_or(|units| !units.is_empty()));
    commands.extend(added);
}

/// Our Tanks this far from an enemy Tank in a fight come up to fire on it.
const JOIN_REACH_TILES: f32 = 22.0;
/// A Tank brought up fires from this far: well inside the 14-tile charged reach, so an enemy Tank
/// sliding about on a trap line stays in it.
const FIRE_POST_TILES: f32 = 13.0;
/// Within this a Tank stops and charges where it is rather than driving to a post; a joined Tank
/// farther than `OUT_OF_REACH_TILES` from its enemy Tank takes a new post.
const STAND_TILES: f32 = 13.9;
const OUT_OF_REACH_TILES: f32 = 13.9;
/// An enemy Tank that has kept to one spot this long is fought from posts.
const STILL_TICKS: u32 = config::TICK_HZ * 2;
/// Posts keep this far from our other Tanks and posts.
const POST_SPACING_TILES: f32 = 2.0;
/// A post within this of an enemy Tank not already in the fight is in its reach.
const THREAT_REACH_TILES: f32 = REACH_TILES;
/// A deployed Anti-Tank Gun reaches 20 tiles.
const AT_GUN_REACH_TILES: f32 = 21.0;
/// A join holds until its enemy Tank is this far from where it stood when joined: then it drives.
const JOIN_DRIFT_TILES: f32 = 5.0;
/// A post whose ground route is longer than this many times the straight line, plus
/// `DETOUR_EXTRA_TILES`, is out of the way.
const DETOUR_RATIO: f32 = 1.5;
const DETOUR_EXTRA_TILES: f32 = 3.0;
/// A Tank has reached its post within this.
const POST_ARRIVED_TILES: f32 = 1.0;
/// A post not reached in this long is given up; a join lasts at most this long.
const POST_TIMEOUT_TICKS: u32 = config::TICK_HZ * 12;
const JOIN_MAX_TICKS: u32 = config::TICK_HZ * 45;
/// An enemy Tank out of sight this long no longer holds a join.
const JOIN_SIGHT_TICKS: u32 = config::TICK_HZ * 3;
/// A push Tank joins fights on enemy Tanks this close to the push's centre.
const PUSH_FIGHT_TILES: f32 = 16.0;
/// Angles tried about the straight line from the enemy Tank to ours, for a free post.
const POST_ANGLES_DEGREES: [f32; 7] = [0.0, 15.0, -15.0, 30.0, -30.0, 45.0, -45.0];

/// One of our Tanks brought up to fire on an enemy Tank in a fight.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Join {
    target: u32,
    target_at: (i32, i32),
    post: (i32, i32),
    since: u32,
    arrived: bool,
}

/// Brings every Tank at hand into fights on enemy Tanks.
///
/// A fight: a visible enemy Tank that has stood still a moment and either is in the reach of one of
/// our Tanks, is shooting at one of our units, or is the target of an Attack order on our Tanks.
/// One enemy Tank fires one shell per reload however many of ours are in its reach, so each more
/// Tank of ours firing on it shortens the fight without drawing more fire, unless it comes into
/// the reach of another enemy Tank or gun not yet in the fight: posts keep out of those.
///
/// Each of our Tanks within `JOIN_REACH_TILES` of such a Tank that cannot hit any enemy Tank from
/// where it is joins: within `STAND_TILES` it stops and charges, farther out it drives to a post
/// `FIRE_POST_TILES` from the enemy Tank on its own side and charges there. It stays until the
/// enemy Tank dies, moves, or is lost from sight. No Tank joins a fight our Tanks already win two
/// to one, and a Tank of the push joins only fights at the push. Returns our Tanks joined; their
/// orders are in `extra`, and other orders on them are dropped.
pub(super) fn concentrate(
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    micro: &mut TankMicro,
    tanks: &BTreeMap<u32, &AiEntitySummary>,
    busy: &BTreeSet<u32>,
    commands: &[Command],
    extra: &mut Vec<Command>,
) -> BTreeSet<u32> {
    let mut joined = BTreeSet::new();
    let tick = observation.tick;
    let ts = observation.map.tile_size as f32;
    let owned: BTreeSet<u32> = observation.owned.iter().map(|unit| unit.id).collect();
    let visible_tanks: Vec<&AiEntitySummary> = observation
        .visible_enemies
        .iter()
        .filter(|enemy| enemy.kind == EntityKind::Tank && enemy.hp > 0)
        .collect();
    let ordered_onto: BTreeSet<u32> = commands
        .iter()
        .filter_map(|command| match command {
            Command::Attack { units, target, .. }
                if units.iter().any(|id| tanks.contains_key(id)) =>
            {
                Some(*target)
            }
            _ => None,
        })
        .collect();
    let reach_of: BTreeMap<u32, f32> = tanks
        .keys()
        .map(|id| (*id, current_range_tiles(micro, *id, tick)))
        .collect();
    let can_hit = |tank: &AiEntitySummary, at: (f32, f32)| {
        tiles_between((tank.x, tank.y), at, ts)
            <= reach_of.get(&tank.id).copied().unwrap_or(0.0) + 0.25
    };
    // Enemy Tanks in a fight with us now.
    let engaged: BTreeSet<u32> = visible_tanks
        .iter()
        .filter(|enemy| {
            enemy
                .target_id
                .is_some_and(|target| owned.contains(&target))
                || ordered_onto.contains(&enemy.id)
                || tanks.values().any(|tank| can_hit(tank, (enemy.x, enemy.y)))
        })
        .map(|enemy| enemy.id)
        .collect();
    let standing = |id: u32| {
        micro
            .enemy_tanks
            .get(&id)
            .is_some_and(|seen| tick.saturating_sub(seen.anchored_since) >= STILL_TICKS)
    };
    let fights: Vec<(u32, (f32, f32))> = visible_tanks
        .iter()
        .filter(|enemy| engaged.contains(&enemy.id) && standing(enemy.id))
        .map(|enemy| (enemy.id, (enemy.x, enemy.y)))
        .collect();
    // Reaches a joining Tank must keep out of: enemy Tanks known about but not in a fight, and
    // visible Anti-Tank Guns.
    let threats: Vec<((f32, f32), f32)> = micro
        .enemy_tanks
        .iter()
        .filter(|(id, seen)| {
            !engaged.contains(id) && tick.saturating_sub(seen.last_seen) <= MEMORY_TICKS
        })
        .map(|(_, seen)| (world(seen.pos), THREAT_REACH_TILES))
        .chain(
            micro
                .ghosts
                .iter()
                .map(|ghost| (world(ghost.pos), THREAT_REACH_TILES)),
        )
        .chain(
            observation
                .visible_enemies
                .iter()
                .filter(|enemy| enemy.kind == EntityKind::AntiTankGun && enemy.hp > 0)
                .map(|enemy| ((enemy.x, enemy.y), AT_GUN_REACH_TILES)),
        )
        .collect();

    // Our Tanks fight an enemy Tank only in numbers: the Tanks we have within reach of it at least
    // match the enemy Tanks around it. Outnumbered, the governor backs them out instead.
    let enemy_tanks_known: Vec<(f32, f32)> = micro
        .enemy_tanks
        .values()
        .filter(|seen| tick.saturating_sub(seen.last_seen) <= ODDS_MEMORY_TICKS)
        .map(|seen| world(seen.pos))
        .chain(micro.ghosts.iter().map(|ghost| world(ghost.pos)))
        .collect();
    let in_numbers = |at: (f32, f32)| {
        let ours = tanks
            .iter()
            .filter(|(id, tank)| {
                !busy.contains(id) && tiles_between((tank.x, tank.y), at, ts) <= JOIN_REACH_TILES
            })
            .count();
        let theirs = enemy_tanks_known
            .iter()
            .filter(|pos| tiles_between(**pos, at, ts) <= ODDS_RADIUS_TILES)
            .count();
        ours >= theirs
    };
    // A fight our Tanks already win two to one, those able to hit the enemy Tank against the
    // enemy Tanks around it, needs no more: Tanks parked on an enemy exit kept being driven to new
    // posts for each Tank that rolled out, losing their charge and leaving their own ground.
    let theirs_at = |at: (f32, f32)| {
        enemy_tanks_known
            .iter()
            .filter(|pos| tiles_between(**pos, at, ts) <= ODDS_RADIUS_TILES)
            .count()
            .max(1)
    };
    let won: BTreeSet<u32> = fights
        .iter()
        .filter(|(_, at)| {
            let shooters = tanks.values().filter(|tank| can_hit(tank, *at)).count();
            shooters >= 2 * theirs_at(*at)
        })
        .map(|(id, _)| *id)
        .collect();
    // Joins end when the enemy Tank is gone, drives off or is lost from sight, when the post is not
    // reached in time, when the Tank falls back, or after the longest join.
    let enemy_tanks = &micro.enemy_tanks;
    let mut dropped: Vec<String> = Vec::new();
    micro.joins.retain(|id, join| {
        let Some(tank) = tanks.get(id).filter(|_| !busy.contains(id)) else {
            dropped.push(format!("unjoin tank={id} busy-or-gone"));
            return false;
        };
        let Some(seen) = enemy_tanks.get(&join.target) else {
            dropped.push(format!("unjoin tank={id} target-forgotten"));
            return false;
        };
        let target_holds = tick.saturating_sub(seen.last_seen) <= JOIN_SIGHT_TICKS
            && tiles_between(world(seen.pos), world(join.target_at), ts) <= JOIN_DRIFT_TILES;
        if !target_holds {
            dropped.push(format!("unjoin tank={id} target-lost-or-drove"));
            return false;
        }
        if tick.saturating_sub(join.since) > JOIN_MAX_TICKS {
            dropped.push(format!("unjoin tank={id} max-time"));
            return false;
        }
        if !in_numbers(world(seen.pos)) {
            dropped.push(format!("unjoin tank={id} outnumbered"));
            return false;
        }
        if !join.arrived {
            if tiles_between((tank.x, tank.y), world(join.post), ts) <= POST_ARRIVED_TILES {
                join.arrived = true;
            } else if tick.saturating_sub(join.since) > POST_TIMEOUT_TICKS {
                dropped.push(format!("unjoin tank={id} post-timeout"));
                return false;
            }
        }
        // Arrived but the enemy Tank slid out of reach: a new post below.
        let out_of_reach =
            tiles_between((tank.x, tank.y), world(seen.pos), ts) > OUT_OF_REACH_TILES;
        if join.arrived && out_of_reach {
            dropped.push(format!("unjoin tank={id} out-of-reach"));
            return false;
        }
        true
    });
    micro.notes.extend(dropped);

    // The push fights where the push is. Its Tanks join only fights that close to the push's
    // centre: drawn home to single raiders, one Tank at a time, the push waited for its rearmost
    // Tank on its own doorstep for 5,000 ticks.
    let push_center = {
        let at: Vec<(f32, f32)> = micro
            .push_tanks
            .iter()
            .filter_map(|id| tanks.get(id))
            .map(|tank| (tank.x, tank.y))
            .collect();
        (!at.is_empty()).then(|| {
            let n = at.len() as f32;
            (
                at.iter().map(|p| p.0).sum::<f32>() / n,
                at.iter().map(|p| p.1).sum::<f32>() / n,
            )
        })
    };
    let fight_at_push = |id: u32, at: (f32, f32)| {
        !micro.push_tanks.contains(&id)
            || push_center.is_some_and(|center| tiles_between(center, at, ts) <= PUSH_FIGHT_TILES)
    };

    // New joins: our Tanks that cannot hit any enemy Tank, to the nearest fight.
    let mut posts: Vec<(f32, f32)> = micro.joins.values().map(|join| world(join.post)).collect();
    let mut new_joins: Vec<(u32, Join)> = Vec::new();
    for (id, tank) in tanks {
        if busy.contains(id) || micro.joins.contains_key(id) {
            continue;
        }
        let from = (tank.x, tank.y);
        if visible_tanks
            .iter()
            .any(|enemy| can_hit(tank, (enemy.x, enemy.y)))
        {
            continue;
        }
        let Some((target, at)) = fights
            .iter()
            .filter(|(fight, _)| !won.contains(fight))
            .filter(|(_, at)| fight_at_push(*id, *at))
            .filter(|(_, at)| tiles_between(from, *at, ts) <= JOIN_REACH_TILES)
            .min_by(|a, b| {
                tiles_between(from, a.1, ts)
                    .total_cmp(&tiles_between(from, b.1, ts))
                    .then_with(|| a.0.cmp(&b.0))
            })
            .copied()
        else {
            continue;
        };
        if !in_numbers(at) {
            continue;
        }
        let distance = tiles_between(from, at, ts);
        let post = if distance <= STAND_TILES {
            Some(from)
        } else {
            let others: Vec<(f32, f32)> = tanks
                .values()
                .filter(|other| other.id != *id)
                .map(|other| (other.x, other.y))
                .chain(posts.iter().copied())
                .collect();
            POST_ANGLES_DEGREES
                .iter()
                .filter_map(|degrees| post_toward(at, from, FIRE_POST_TILES, *degrees, ts))
                .map(|post| clamp_to_map(post, observation.map))
                .find(|post| {
                    tiles_between(*post, at, ts) <= STAND_TILES
                        && others
                            .iter()
                            .all(|other| tiles_between(*other, *post, ts) >= POST_SPACING_TILES)
                        && threats
                            .iter()
                            .all(|(threat, reach)| tiles_between(*threat, *post, ts) > *reach)
                        && defense::defensive_position_is_open(
                            observation,
                            map_analysis,
                            post.0,
                            post.1,
                        )
                        && reachable(map_analysis, from, *post, ts)
                })
        };
        let Some(post) = post else {
            continue;
        };
        micro.notes.push(format!(
            "join tank={id} fight on tank {target} from {distance:.1} tiles: post ({:.0},{:.0})",
            post.0 / ts,
            post.1 / ts
        ));
        posts.push(post);
        new_joins.push((
            *id,
            Join {
                target,
                target_at: stored(at),
                post: stored(post),
                since: tick,
                arrived: distance <= STAND_TILES,
            },
        ));
    }
    let fresh: BTreeSet<u32> = new_joins.iter().map(|(id, _)| *id).collect();
    micro.joins.extend(new_joins);

    // Joined Tanks drive to their posts, or stand at them and all fire on the enemy Tank in reach
    // that dies soonest, so the fight loses its guns one by one.
    let foes = foes(observation, micro);
    let mut aims: Vec<String> = Vec::new();
    for (id, join) in &micro.joins {
        let Some(tank) = tanks.get(id) else {
            continue;
        };
        joined.insert(*id);
        let idle = tank.state == AiEntityState::Idle;
        if join.arrived {
            let from = (tank.x, tank.y);
            let reach = reach_of.get(id).copied().unwrap_or(0.0) + 0.25;
            let shots = |enemy: u32| {
                foes.get(&enemy)
                    .filter(|foe| foe.visible && tiles_between(from, foe.pos, ts) <= reach)
                    .map(|foe| shots_to_kill(foe, from))
            };
            let best = foes
                .keys()
                .filter_map(|enemy| shots(*enemy).map(|count| (*enemy, count)))
                .min_by_key(|(enemy, count)| (*count, *enemy != join.target, *enemy));
            let current = tank.target_id.and_then(shots);
            let driving = !idle && micro.own.get(id).is_some_and(|own| own.still_since >= tick);
            match best {
                Some((best, count))
                    if tank.target_id != Some(best)
                        && current.is_none_or(|current| count + RETARGET_GAIN_SHOTS <= current) =>
                {
                    aims.push(format!("focus tank={id} on tank {best} ({count} shots)"));
                    extra.push(Command::Attack {
                        units: vec![*id],
                        target: best,
                        queued: false,
                    });
                }
                _ if driving => extra.push(Command::HoldPosition {
                    units: vec![*id],
                    queued: false,
                }),
                _ => {}
            }
        } else if idle || fresh.contains(id) {
            let post = world(join.post);
            extra.push(Command::Move {
                units: vec![*id],
                x: post.0,
                y: post.1,
                queued: false,
            });
        }
    }
    micro.notes.extend(aims);
    joined
}

/// The point `tiles` from `target` toward `from`, turned `degrees` about the target.
fn post_toward(
    target: (f32, f32),
    from: (f32, f32),
    tiles: f32,
    degrees: f32,
    ts: f32,
) -> Option<(f32, f32)> {
    let dir = normalized_direction(target, from)?;
    let (sin, cos) = degrees.to_radians().sin_cos();
    let turned = (dir.0 * cos - dir.1 * sin, dir.0 * sin + dir.1 * cos);
    Some((
        target.0 + turned.0 * tiles * ts,
        target.1 + turned.1 * tiles * ts,
    ))
}

/// Whether a Tank at `from` can drive to `to` without a long way round: a ground route exists
/// and is no more than `DETOUR_RATIO` times the straight line plus `DETOUR_EXTRA_TILES`.
fn reachable(
    map_analysis: Option<&AiMapAnalysis>,
    from: (f32, f32),
    to: (f32, f32),
    ts: f32,
) -> bool {
    let Some(analysis) = map_analysis else {
        return true;
    };
    let straight = tiles_between(from, to, ts);
    let route = analysis.compact_group_route(from, to, 1);
    if route.len() <= 1 {
        // No route found: the planner returns the destination alone.
        return straight <= 1.5;
    }
    let mut length = 0.0;
    let mut previous = from;
    for point in &route {
        length += tiles_between(previous, *point, ts);
        previous = *point;
    }
    length <= straight * DETOUR_RATIO + DETOUR_EXTRA_TILES
}
