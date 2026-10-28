//! Jeff's AI V1.1 takes base 3 by safety and bank rather than by Tank count. V1.0 waited for five
//! Tanks, three of them free, with the home reserve still home after two guards left; against
//! Pressure AI it held 0-3 Depots while Pressure ran five bases.
//!
//! - **Escort, not reserve**: the base goes once an escort is free, two Tanks or, short of them, a
//!   squad of `ESCORT_RIFLES` Riflemen and `ESCORT_GUNS` Machine Gunners; the home reserve no longer
//!   holds it back.
//! - **Bank trigger**: Steel at `BANK_STEEL` or more for `BANK_TICKS` takes it too, Tanks or not, and
//!   meanwhile that Steel buys Riflemen and guns (`banked`).
//! - **Safer sites**: a site on the enemy's route to the main costs `ROUTE_PENALTY_SCALE` times as
//!   much head start as V1.0's.
//! - **Quick retry**: a failed site is ruled out and the next one tried after `RETRY_TICKS`.

use super::*;
use crate::ai_core::decision::geometry::dist2;

const ESCORT_TANKS: usize = 2;
const ESCORT_RIFLES: usize = 4;
const ESCORT_GUNS: usize = 2;
pub(in crate::ai_core::decision) const BANK_STEEL: u32 = 600;
const BANK_TICKS: u32 = config::TICK_HZ * 30;
pub(in crate::ai_core::decision) const ROUTE_PENALTY_SCALE: i32 = 2;
pub(in crate::ai_core::decision) const RETRY_TICKS: u32 = config::TICK_HZ * 20;
/// The escort's posts stand this far in front of the site and this far apart across it.
const ESCORT_POST_TILES: f32 = 5.0;
const ESCORT_SPACING_TILES: f32 = 1.75;

/// Notes how long V1.1's Steel has stood at `BANK_STEEL` or more, for base 3, and how long with a
/// Tank out, for spending on more production and infantry. Before the first Tank the bank is the
/// spend plan's savings for the natural, the Factory and the Tank: spent on Barracks and Riflemen
/// then, it would put the first Tank back.
pub(in crate::ai_core::decision) fn note_bank(
    observation: &AiObservation,
    memory: &mut AiDecisionMemory,
    profile_id: &str,
) {
    let state = &mut memory.jeff_v1_1;
    let tank_out = observation
        .owned
        .iter()
        .any(|unit| unit.kind == EntityKind::Tank && unit.is_complete && unit.hp > 0);
    let rich = is_jeff_v1_1(profile_id) && observation.economy.steel >= BANK_STEEL;
    if !rich {
        state.base_bank_since = None;
    } else if state.base_bank_since.is_none() {
        state.base_bank_since = Some(observation.tick);
    }
    if !rich || !tank_out {
        state.banked_since = None;
    } else if state.banked_since.is_none() {
        state.banked_since = Some(observation.tick);
    }
}

/// Whether V1.1 has banked `BANK_STEEL` Steel for `BANK_TICKS`, Tank or not: base 3 is due.
pub(in crate::ai_core::decision) fn base_banked(
    observation: &AiObservation,
    memory: &AiDecisionMemory,
) -> bool {
    memory
        .jeff_v1_1
        .base_bank_since
        .is_some_and(|since| observation.tick.saturating_sub(since) >= BANK_TICKS)
}

/// Whether V1.1 has banked `BANK_STEEL` Steel for `BANK_TICKS`.
pub(in crate::ai_core::decision) fn banked(
    observation: &AiObservation,
    memory: &AiDecisionMemory,
) -> bool {
    memory
        .jeff_v1_1
        .banked_since
        .is_some_and(|since| observation.tick.saturating_sub(since) >= BANK_TICKS)
}

/// Banked Steel builds production to spend it: V1.1 mines about 2,000 Steel a minute by 5:00 on
/// Classic and floated 900-1,000 of it with two Barracks and one or two Factories, while Pressure
/// AI grows to four Vehicle Works on its float.
const BANKED_BARRACKS: usize = 4;
const BANKED_FACTORIES: usize = 4;
/// A Factory more needs this much Oil in the bank too, for the Tanks it is to build.
const BANKED_FACTORY_OIL: u32 = 250;

/// The Barracks V1.1 builds toward. Its oldest Barracks trains only guns, so from V1.0's second
/// Barracks on it keeps one more for the Riflemen: with two, it fielded 4-14 Riflemen against
/// Pressure AI's 10-12 Panzerfausts where V1.0 fielded 20-33. While its Steel is banked, at least
/// `BANKED_BARRACKS`.
pub(in crate::ai_core::decision) fn barracks_target(
    profile_id: &str,
    banked: bool,
    target: usize,
) -> usize {
    if !is_jeff_v1_1(profile_id) {
        return target;
    }
    let target = if target >= 2 && !super::off("extrabarracks") {
        target + 1
    } else {
        target
    };
    if banked && !super::off("bankprod") {
        target.max(BANKED_BARRACKS)
    } else {
        target
    }
}

/// Whether V1.1 builds another Factory on banked Steel and Oil, up to `BANKED_FACTORIES`.
pub(in crate::ai_core::decision) fn extra_factory(
    profile_id: &str,
    banked: bool,
    observation: &AiObservation,
    factories: usize,
) -> bool {
    is_jeff_v1_1(profile_id)
        && !super::off("bankprod")
        && banked
        && observation.economy.oil >= BANKED_FACTORY_OIL
        && factories < BANKED_FACTORIES
}

/// The escort that would take a base at `site`: the two Tanks nearest it that no push and not the
/// home Tank holds or, short of them, the nearest free Riflemen and guns.
pub(in crate::ai_core::decision) fn escort(
    observation: &AiObservation,
    memory: &AiDecisionMemory,
    site: (f32, f32),
) -> Option<BTreeSet<u32>> {
    let reserved = memory.squad_reserved();
    let free = |unit: &&AiEntitySummary| {
        unit.is_complete
            && unit.hp > 0
            && Some(unit.id) != memory.home_defensive_tank
            && !memory.containment.active_tanks.contains(&unit.id)
            && !memory.containment.opening_tanks.contains(&unit.id)
            && !memory.containment.active_riflemen.contains(&unit.id)
            && !memory.expansion_security.riflemen.contains(&unit.id)
            && !memory.opening_rush.is_reserved(unit.id)
            && !reserved.contains(&unit.id)
    };
    let nearest = |kind: EntityKind, count: usize| {
        let mut units: Vec<&AiEntitySummary> = observation
            .owned
            .iter()
            .filter(|unit| unit.kind == kind)
            .filter(free)
            .collect();
        units.sort_by(|a, b| {
            dist2(a.x, a.y, site.0, site.1)
                .total_cmp(&dist2(b.x, b.y, site.0, site.1))
                .then(a.id.cmp(&b.id))
        });
        (units.len() >= count).then(|| {
            units
                .into_iter()
                .take(count)
                .map(|unit| unit.id)
                .collect::<Vec<_>>()
        })
    };
    if let Some(tanks) = nearest(EntityKind::Tank, ESCORT_TANKS) {
        return Some(tanks.into_iter().collect());
    }
    let rifles = nearest(EntityKind::Rifleman, ESCORT_RIFLES)?;
    let guns = nearest(EntityKind::MachineGunner, ESCORT_GUNS)?;
    Some(rifles.into_iter().chain(guns).collect())
}

/// One post per escort member, in a line in front of the site, pulled back toward the site until
/// the ground is open.
pub(in crate::ai_core::decision) fn escort_posts(
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    site: (f32, f32),
    enemy: (f32, f32),
    count: usize,
) -> Vec<(f32, f32)> {
    let ts = observation.map.tile_size as f32;
    let direction = normalized_direction(site, enemy).unwrap_or((0.0, 1.0));
    let across = (-direction.1, direction.0);
    (0..count)
        .map(|index| {
            let side = (index as f32 - (count as f32 - 1.0) * 0.5) * ESCORT_SPACING_TILES;
            let mut ahead = ESCORT_POST_TILES;
            while ahead >= 2.0 {
                let point = clamp_to_map(
                    (
                        site.0 + (direction.0 * ahead + across.0 * side) * ts,
                        site.1 + (direction.1 * ahead + across.1 * side) * ts,
                    ),
                    observation.map,
                );
                if defense::defensive_position_is_open(observation, map_analysis, point.0, point.1)
                {
                    return point;
                }
                ahead -= 1.0;
            }
            site
        })
        .collect()
}
