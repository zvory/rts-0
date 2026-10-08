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
