use crate::game::entity::{uses_oriented_vehicle_body, Entity, EntityKind, Order, WeaponSetup};
use crate::game::map::Map;
use crate::game::services::occupancy::Occupancy;
use crate::game::services::standability as static_standability;

/// Whether a unit body may stand at this world position against static blockers.
pub(super) fn unit_static_standable(
    occ: &Occupancy,
    map: &Map,
    kind: EntityKind,
    x: f32,
    y: f32,
    facing: f32,
) -> bool {
    static_standability::unit_static_standable_with_facing(map, occ, kind, x, y, facing)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum FootingProfile {
    Ghost,
    Soft,
    Firm,
    Braced,
    Heavy,
}

pub(super) fn footing_profile(e: &Entity) -> FootingProfile {
    if worker_has_pass_through_work_order(e) {
        return FootingProfile::Ghost;
    }
    if requires_weapon_setup(e.kind)
        && matches!(
            e.weapon_setup(),
            WeaponSetup::SettingUp { .. } | WeaponSetup::Deployed
        )
    {
        return FootingProfile::Braced;
    }
    if uses_oriented_vehicle_body(e.kind) {
        return FootingProfile::Heavy;
    }
    if !requires_weapon_setup(e.kind)
        && !uses_oriented_vehicle_body(e.kind)
        && e.target_id().is_some()
        && e.path_is_empty()
    {
        return FootingProfile::Firm;
    }
    FootingProfile::Soft
}

fn worker_has_pass_through_work_order(e: &Entity) -> bool {
    e.kind == EntityKind::Worker
        && matches!(
            e.order(),
            Order::Gather(_) | Order::Build(_) | Order::Deconstruct(_)
        )
}

pub(super) fn requires_weapon_setup(kind: EntityKind) -> bool {
    matches!(
        kind,
        EntityKind::MachineGunner
            | EntityKind::AntiTankGun
            | EntityKind::MortarTeam
            | EntityKind::Artillery
    )
}

pub(super) fn footing_resistance(profile: FootingProfile) -> f32 {
    match profile {
        FootingProfile::Ghost => 0.0,
        FootingProfile::Soft => 1.0,
        FootingProfile::Firm => 3.0,
        FootingProfile::Braced => 8.0,
        FootingProfile::Heavy => 12.0,
    }
}

fn is_pass_through(profile: FootingProfile) -> bool {
    profile == FootingProfile::Ghost
}

/// Whether this unit is currently a ghost for collision and must not be pushed by
/// collision. Ghost units neither push nor are pushed, so other mobile units can pass
/// through them freely.
pub(crate) fn is_collision_anchored(e: &Entity) -> bool {
    is_pass_through(footing_profile(e))
}
