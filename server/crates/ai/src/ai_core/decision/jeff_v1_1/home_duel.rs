//! `hduel`: Jeff's AI V1.1's Tanks answer an enemy Tank raiding his bases from long range. A
//! standing Tank's cannon reaches 14 tiles after 3 s (5 while moving) and takes an enemy Tank in
//! its reach over any infantry, so a Jeff Tank standing 12-13 tiles off a raider draws the raider's
//! cannon off the Riflemen and buildings it was shelling and shells it back; the Panzerfaust guard
//! (`pflock`) then runs in on a raider whose cannon is on a Jeff Tank. In 16 traced games vs
//! Pressure AI, raiders stood still 79% of the time; a Jeff Tank stood within 14 tiles of one in
//! 50% of raid snapshots, and only 14-25 tiles off (out of reach, doing nothing) in another 43%.
//!
//! Each visible enemy Tank within `RAID_TILES` of a Jeff Depot is answered by the nearest Jeff
//! Tanks within `RESPOND_TILES` of it and `HOME_TILES` of a Depot that are healthy, off Tank raids
//! and clear of enemy infantry: one more than the enemy Tanks within `GROUP_TILES` of it (seen now
//! or in the last `HIDDEN_TANK_TICKS`), only for raids of up to `MAX_GROUP` Tanks, and none unless
//! that many are free. A
//! responder farther than `DUEL_MAX_TILES` drives to `DUEL_TILES` from the raider on its own side
//! and holds there; nearer, it stands on hold position, and once standing it stays while the raider
//! is within `STAY_TILES` or on the move. It is let go when the raider dies, leaves
//! (`LEAVE_TILES`) or stays out of sight for `LOST_TICKS`, below `MIN_HP_SHARE`, or once enemy
//! infantry comes within `INFANTRY_TILES` of it (the Tank micro takes that).

use super::*;
use crate::ai_core::decision::geometry::{clamp_to_map, dist2, squared};
use crate::ai_core::observation::AiEntityState;
use rts_rules::math::GameplayMath as _;
use rts_sim::game::command::SimCommand as Command;

const RAID_TILES: f32 = 18.0;
const LEAVE_TILES: f32 = 24.0;
const RESPOND_TILES: f32 = 26.0;
const HOME_TILES: f32 = 26.0;
const GROUP_TILES: f32 = 10.0;
/// Only raids of this many Tanks are answered (a lone raider, 25% of raid time vs Pressure AI,
/// shot a Jeff Tank in 3% of snapshots: it shells the base unanswered); one that grows past one
/// more is let go.
const MAX_GROUP: usize = 2;
/// Where a responder stops: inside a standing raider's 14 tiles, so its cannon turns on the
/// responder, and within the responder's own sight (9 tiles), so it sees what it shoots; its reach
/// (5 moving, ramping to 14 over 3 s) grows to 9 tiles in 1.3 s. Stopping at 12.5 tiles, traced
/// responders shot the raider only a fifth of the time.
const DUEL_TILES: f32 = 9.0;
const DUEL_MAX_TILES: f32 = 10.0;
/// A standing responder stays while the raider is this close or on the move: every move resets
/// its reach to 5, and chasing a moving raider kept a third of them driving.
const STAY_TILES: f32 = 14.5;
const LOST_TICKS: u32 = config::TICK_HZ * 5;
const MIN_HP_SHARE: f32 = 0.35;
const INFANTRY_TILES: f32 = 7.0;
const HIDDEN_TANK_TICKS: u32 = config::TICK_HZ * 10;
const RESEND_TILES: f32 = 2.5;

/// `hduel`: each responding Tank's duel.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(in crate::ai_core::decision) struct HomeDuelMemory {
    duels: BTreeMap<u32, Duel>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Duel {
    raider: u32,
    /// When and where the raider was last seen.
    seen: u32,
    at: (i32, i32),
    /// Where the responder was last sent.
    sent: Option<(i32, i32)>,
}

impl HomeDuelMemory {
    pub(in crate::ai_core::decision) fn units(&self) -> impl Iterator<Item = u32> + '_ {
        self.duels.keys().copied()
    }
}

pub(in crate::ai_core::decision) fn active(profile_id: &str) -> bool {
    is_jeff_v1_1(profile_id) && !super::off("hduel")
}

fn unit_vector(from: (f32, f32), to: (f32, f32)) -> Option<(f32, f32)> {
    let (dx, dy) = (to.0 - from.0, to.1 - from.1);
    let length = (dx * dx + dy * dy).det_sqrt();
    (length > 1.0).then(|| (dx / length, dy / length))
}

/// Answers raiding enemy Tanks with Jeff's Tanks (see the module docs); runs after the Tank micro
/// and takes its responders' orders over.
pub(in crate::ai_core::decision) fn finish(
    decision: &mut AiDecision,
    observation: &AiObservation,
    memory: &mut AiDecisionMemory,
) {
    if !memory.profile_id().is_some_and(active) {
        memory.jeff_v1_1.home_duel = HomeDuelMemory::default();
        return;
    }
    let ts = observation.map.tile_size as f32;
    let tick = observation.tick;
    let within =
        |a: (f32, f32), b: (f32, f32), tiles: f32| dist2(a.0, a.1, b.0, b.1) <= squared(tiles * ts);
    let at = |unit: &AiEntitySummary| (unit.x, unit.y);
    let depots: Vec<(f32, f32)> = observation
        .owned
        .iter()
        .filter(|unit| unit.kind == EntityKind::ResourceDepot && unit.is_complete && unit.hp > 0)
        .map(|unit| (unit.x, unit.y))
        .collect();
    let home =
        |point: (f32, f32), tiles: f32| depots.iter().any(|depot| within(*depot, point, tiles));
    let enemy_tanks: Vec<&AiEntitySummary> = observation
        .visible_enemies
        .iter()
        .filter(|enemy| enemy.kind == EntityKind::Tank && enemy.hp > 0)
        .collect();
    let own_tanks: Vec<&AiEntitySummary> = observation
        .owned
        .iter()
        .filter(|unit| unit.kind == EntityKind::Tank && unit.is_complete && unit.hp > 0)
        .collect();
    let tank_hp = rts_rules::defs::unit_def(EntityKind::Tank).map_or(292, |def| def.stats.hp);
    let healthy = |unit: &AiEntitySummary| unit.hp as f32 >= MIN_HP_SHARE * tank_hp as f32;
    let infantry_near = |unit: &AiEntitySummary| {
        observation.visible_enemies.iter().any(|enemy| {
            enemy.hp > 0
                && matches!(
                    enemy.kind,
                    EntityKind::Rifleman | EntityKind::MachineGunner | EntityKind::Panzerfaust
                )
                && within(at(enemy), at(unit), INFANTRY_TILES)
        })
    };
    let recent = super::push::recent_tank_spots(memory, tick, HIDDEN_TANK_TICKS);
    let reserved: BTreeSet<u32> = memory.jeff_v1_1.tank_raid.units().collect();
    let mut duel = std::mem::take(&mut memory.jeff_v1_1.home_duel);

    // Let go of finished duels.
    duel.duels.retain(|own, fight| {
        let Some(unit) = own_tanks.iter().find(|unit| unit.id == *own) else {
            return false;
        };
        if !healthy(unit) || infantry_near(unit) {
            return false;
        }
        match enemy_tanks.iter().find(|tank| tank.id == fight.raider) {
            Some(raider) => {
                fight.seen = tick;
                fight.at = (raider.x as i32, raider.y as i32);
                let group = enemy_tanks
                    .iter()
                    .filter(|other| within(at(other), at(raider), GROUP_TILES))
                    .count();
                home(at(raider), LEAVE_TILES) && group <= MAX_GROUP + 1
            }
            None => tick.saturating_sub(fight.seen) <= LOST_TICKS,
        }
    });

    // New responders, the raider nearest a Depot first.
    let to_base = |point: (f32, f32)| {
        depots
            .iter()
            .map(|depot| dist2(depot.0, depot.1, point.0, point.1))
            .fold(f32::INFINITY, f32::min)
    };
    let mut raiders: Vec<&AiEntitySummary> = enemy_tanks
        .iter()
        .copied()
        .filter(|tank| home(at(tank), RAID_TILES))
        .collect();
    raiders.sort_by(|a, b| {
        to_base(at(a))
            .total_cmp(&to_base(at(b)))
            .then(a.id.cmp(&b.id))
    });
    let mut note = String::new();
    for raider in raiders {
        let answering = duel
            .duels
            .values()
            .filter(|fight| fight.raider == raider.id)
            .count();
        let seen_group = enemy_tanks
            .iter()
            .filter(|other| other.id != raider.id && within(at(other), at(raider), GROUP_TILES))
            .count();
        let hidden_group = recent
            .iter()
            .filter(|(id, spot)| {
                *id != raider.id
                    && !enemy_tanks.iter().any(|seen| seen.id == *id)
                    && within(*spot, at(raider), GROUP_TILES)
            })
            .count();
        let group = 1 + seen_group + hidden_group;
        let want = group + 1;
        if answering >= want {
            continue;
        }
        // A bigger raid is left to the Tanks where they stand: the side that stands wins a Tank
        // fight, and four responders driving at four raiders on Wald lost 5 Tanks for 1.
        if group > MAX_GROUP {
            use std::fmt::Write as _;
            let _ = write!(note, " t{}:G{}", raider.id, group);
            continue;
        }
        let mut free: Vec<&AiEntitySummary> = own_tanks
            .iter()
            .copied()
            .filter(|unit| !duel.duels.contains_key(&unit.id) && !reserved.contains(&unit.id))
            .filter(|unit| healthy(unit) && !infantry_near(unit))
            .filter(|unit| {
                within(at(unit), at(raider), RESPOND_TILES) && home(at(unit), HOME_TILES)
            })
            .collect();
        if answering + free.len() < want {
            use std::fmt::Write as _;
            let _ = write!(
                note,
                " t{}:g{}f{}",
                raider.id,
                group,
                answering + free.len()
            );
            continue;
        }
        free.sort_by(|a, b| {
            dist2(a.x, a.y, raider.x, raider.y)
                .total_cmp(&dist2(b.x, b.y, raider.x, raider.y))
                .then(a.id.cmp(&b.id))
        });
        for unit in free.into_iter().take(want - answering) {
            duel.duels.insert(
                unit.id,
                Duel {
                    raider: raider.id,
                    seen: tick,
                    at: (raider.x as i32, raider.y as i32),
                    sent: None,
                },
            );
        }
    }

    // Orders: drive to `DUEL_TILES` from the raider, else stand.
    let held: BTreeSet<u32> = duel.duels.keys().copied().collect();
    if !held.is_empty() {
        decision
            .commands
            .retain_mut(|command| super::matchup::strip(command, &held));
    }
    let mut orders: Vec<Command> = Vec::new();
    for (own, fight) in duel.duels.iter_mut() {
        let Some(unit) = own_tanks.iter().find(|unit| unit.id == *own) else {
            continue;
        };
        let visible = enemy_tanks.iter().find(|tank| tank.id == fight.raider);
        let raider_at = visible.map_or((fight.at.0 as f32, fight.at.1 as f32), |tank| at(tank));
        let standing = unit.state != AiEntityState::Move;
        let stay = standing
            && (within(at(unit), raider_at, STAY_TILES)
                || visible.is_some_and(|tank| tank.state == AiEntityState::Move));
        if visible.is_none() || stay || within(at(unit), raider_at, DUEL_MAX_TILES) {
            if unit.state == AiEntityState::Move {
                orders.push(Command::HoldPosition {
                    units: vec![unit.id],
                    queued: false,
                });
            }
            fight.sent = None;
            continue;
        }
        let away = unit_vector(raider_at, at(unit)).unwrap_or((0.0, 1.0));
        let place = clamp_to_map(
            (
                raider_at.0 + away.0 * DUEL_TILES * ts,
                raider_at.1 + away.1 * DUEL_TILES * ts,
            ),
            observation.map,
        );
        let spot = (place.0 as i32, place.1 as i32);
        let near = (RESEND_TILES * ts) as i32;
        let same = fight
            .sent
            .is_some_and(|sent| (sent.0 - spot.0).abs() <= near && (sent.1 - spot.1).abs() <= near);
        if same && unit.state == AiEntityState::Move {
            continue;
        }
        orders.push(Command::Move {
            units: vec![unit.id],
            x: place.0,
            y: place.1,
            queued: false,
        });
        orders.push(Command::HoldPosition {
            units: vec![unit.id],
            queued: true,
        });
        fight.sent = Some(spot);
    }
    decision.trace.notes.push(format!(
        "hduel duels={}{}",
        duel.duels
            .iter()
            .map(|(own, fight)| format!("{own}>{}", fight.raider))
            .collect::<Vec<_>>()
            .join(","),
        note
    ));
    decision.commands.extend(orders);
    memory.jeff_v1_1.home_duel = duel;
}
