//! What each unit kind is to the matchup micro: which it steers and which fight, their costs,
//! reaches and damage rates against each other, and which avoid or fire on the move.

use super::*;

/// The kinds the micro steers.
pub(super) fn steers(kind: EntityKind) -> bool {
    matches!(
        kind,
        EntityKind::Rifleman
            | EntityKind::Panzerfaust
            | EntityKind::MachineGunner
            | EntityKind::Tank
            | EntityKind::ScoutCar
    )
}

/// Enemy kinds that fight (workers and buildings are left to the other systems).
pub(super) fn fights(kind: EntityKind) -> bool {
    matches!(
        kind,
        EntityKind::Rifleman
            | EntityKind::Panzerfaust
            | EntityKind::MachineGunner
            | EntityKind::Tank
            | EntityKind::ScoutCar
            | EntityKind::AntiTankGun
            | EntityKind::MortarTeam
            | EntityKind::Warrior
            | EntityKind::Golem
            | EntityKind::Ekat
    )
}

pub(super) fn infantry(kind: EntityKind) -> bool {
    matches!(
        kind,
        EntityKind::Rifleman | EntityKind::Panzerfaust | EntityKind::MachineGunner
    )
}

pub(super) fn vehicle(kind: EntityKind) -> bool {
    matches!(
        kind,
        EntityKind::Tank
            | EntityKind::ScoutCar
            | EntityKind::CommandCar
            | EntityKind::RocketLauncher
            | EntityKind::Artillery
    )
}

pub(super) fn armored(kind: EntityKind) -> bool {
    rts_rules::combat::is_armored(kind)
}

/// The steel plus oil a unit costs.
pub(crate) fn cost(kind: EntityKind) -> f32 {
    rts_rules::balance::unit_stats(kind).map_or(0.0, |s| (s.cost_steel + s.cost_oil) as f32)
}

/// Reach in tiles of `attacker` against `target`; a Tank's cannon ramps from 5 to 14 tiles over
/// its first three seconds standing still, and its coax reaches 6 against soft targets.
pub(crate) fn reach(attacker: EntityKind, target: EntityKind, still_ticks: u32) -> f32 {
    match attacker {
        EntityKind::Tank => {
            let ramp = (still_ticks as f32 / TANK_RAMP_TICKS).min(1.0);
            let cannon = TANK_BASE_REACH + (TANK_FULL_REACH - TANK_BASE_REACH) * ramp;
            if armored(target) {
                cannon
            } else {
                cannon.max(6.0)
            }
        }
        EntityKind::ScoutCar => 7.0,
        EntityKind::MachineGunner => 6.1,
        EntityKind::Rifleman | EntityKind::Panzerfaust | EntityKind::AntiTankGun => 5.0,
        EntityKind::MortarTeam => 17.0,
        EntityKind::Ekat => 6.0,
        EntityKind::Warrior | EntityKind::Golem | EntityKind::Worker => 1.0,
        _ => 0.0,
    }
}

/// Damage per second of `attacker` against `target`, roughly: Machine-gun bursts are three rays
/// at 60% damage and some miss; a Panzerfaust's single rocket is spread over its reload.
pub(crate) fn dps(attacker: EntityKind, target: EntityKind, meth: bool) -> f32 {
    let hard = armored(target);
    let rifle = if hard { 1.0 } else { 5.0 } * TICK_HZ / if meth { 24.0 } else { 32.0 };
    match attacker {
        EntityKind::Rifleman => rifle,
        EntityKind::Panzerfaust => rifle + if vehicle(target) { 10.0 } else { 0.0 },
        EntityKind::MachineGunner => {
            if hard {
                3.0
            } else {
                12.0
            }
        }
        EntityKind::ScoutCar => {
            if hard {
                4.5
            } else {
                19.0
            }
        }
        EntityKind::Tank => {
            if hard {
                12.5
            } else {
                25.0
            }
        }
        EntityKind::AntiTankGun => {
            if hard {
                20.8
            } else {
                6.0
            }
        }
        EntityKind::MortarTeam => 6.0,
        EntityKind::Warrior => 10.0,
        EntityKind::Golem => 10.0,
        EntityKind::Ekat => 8.0,
        EntityKind::Worker => {
            if hard {
                0.6
            } else {
                2.5
            }
        }
        _ => 0.0,
    }
}

/// The pairs where the first kind should not fight the second at all.
pub(super) fn avoids(own: EntityKind, enemy: EntityKind) -> bool {
    enemy == EntityKind::Tank
        && matches!(
            own,
            EntityKind::Rifleman | EntityKind::MachineGunner | EntityKind::ScoutCar
        )
}

/// Fires on the move without losing its rate of fire.
pub(super) fn fires_moving(kind: EntityKind, meth: bool, params: &Params) -> bool {
    vehicle(kind) || (params.kite_rifles && meth && matches!(kind, EntityKind::Rifleman))
}
