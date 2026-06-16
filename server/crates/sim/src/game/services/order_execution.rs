use crate::config;
use crate::game::entity::{Entity, EntityKind, EntityStore, Order, WeaponSetup};
use crate::game::map::Map;
use crate::game::smoke::SmokeCloudStore;

#[derive(Clone, Copy)]
pub(crate) enum FutureOrderMode {
    Preserve,
    Clear,
}

#[derive(Clone, Copy)]
pub(crate) struct ArtilleryPointFireTarget {
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) facing: f32,
    pub(crate) inside_field_of_fire: bool,
}

#[derive(Clone, Copy)]
pub(crate) enum ArtilleryPointFireAcceptance {
    BasicTarget,
    Command,
    Deployed,
}

pub(crate) fn execute_anti_tank_gun_setup(
    entities: &mut EntityStore,
    id: u32,
    x: f32,
    y: f32,
    future_orders: FutureOrderMode,
) -> bool {
    let Some(e) = entities.get(id) else {
        return false;
    };
    if !matches!(e.kind, EntityKind::AntiTankGun | EntityKind::Artillery)
        || e.under_construction()
        || !x.is_finite()
        || !y.is_finite()
    {
        return false;
    }
    let facing = (y - e.pos_y).atan2(x - e.pos_x);
    if !facing.is_finite() {
        return false;
    }
    entities.release_miner(id);
    let Some(e) = entities.get_mut(id) else {
        return false;
    };
    match future_orders {
        FutureOrderMode::Preserve => e.clear_active_order(),
        FutureOrderMode::Clear => e.clear_orders(),
    }
    e.set_path_goal(None);
    if matches!(e.weapon_setup(), WeaponSetup::Packed) {
        e.set_emplacement_facing(Some(facing));
        e.set_desired_weapon_facing(facing);
    } else {
        e.set_pending_redeploy_facing(Some(facing));
        e.set_weapon_setup(WeaponSetup::TearingDownToRedeploy {
            ticks: setup_ticks_for(e.kind),
        });
    }
    e.reset_gather_state();
    let (px, py) = (e.pos_x, e.pos_y);
    e.reset_stuck(px, py);
    true
}

pub(crate) fn begin_artillery_teardown_for_movement(entities: &mut EntityStore, ids: &[u32]) {
    for id in ids {
        let Some(e) = entities.get_mut(*id) else {
            continue;
        };
        if e.kind != EntityKind::Artillery {
            continue;
        }
        e.reset_artillery_accuracy();
        if !matches!(e.weapon_setup(), WeaponSetup::Packed) {
            e.set_weapon_setup(WeaponSetup::TearingDown {
                ticks: config::ARTILLERY_SETUP_TICKS,
            });
        }
    }
}

pub(crate) fn artillery_point_fire_target(
    map: &Map,
    entities: &EntityStore,
    player: u32,
    unit: u32,
    x: f32,
    y: f32,
    acceptance: ArtilleryPointFireAcceptance,
) -> Option<ArtilleryPointFireTarget> {
    let target = artillery_point_fire_basic_target(map, entities, player, unit, x, y)?;
    let e = entities.get(unit)?;
    match acceptance {
        ArtilleryPointFireAcceptance::BasicTarget => Some(target),
        ArtilleryPointFireAcceptance::Command => {
            artillery_can_accept_point_fire_command(e).then_some(target)
        }
        ArtilleryPointFireAcceptance::Deployed => {
            matches!(e.weapon_setup(), WeaponSetup::Deployed).then_some(target)
        }
    }
}

pub(crate) fn start_artillery_point_fire_command_order(
    entities: &mut EntityStore,
    unit: u32,
    target: ArtilleryPointFireTarget,
) -> bool {
    entities.release_miner(unit);
    let Some(e) = entities.get_mut(unit) else {
        return false;
    };
    e.clear_orders();
    e.set_path_goal(None);
    e.reset_gather_state();
    let (px, py) = (e.pos_x, e.pos_y);
    e.reset_stuck(px, py);
    start_artillery_point_fire_from_target(e, target, false)
}

pub(crate) fn start_artillery_point_fire_promoted_order(
    entities: &mut EntityStore,
    unit: u32,
    target: ArtilleryPointFireTarget,
) -> bool {
    let Some(e) = entities.get_mut(unit) else {
        return false;
    };
    e.clear_active_order();
    e.set_path_goal(None);
    start_artillery_point_fire_from_target(e, target, true)
}

fn start_artillery_point_fire_from_target(
    e: &mut Entity,
    target: ArtilleryPointFireTarget,
    require_deployed: bool,
) -> bool {
    if require_deployed && !matches!(e.weapon_setup(), WeaponSetup::Deployed) {
        return false;
    }
    if !target.inside_field_of_fire {
        e.set_pending_redeploy_facing(Some(target.facing));
        e.set_weapon_setup(WeaponSetup::TearingDownToRedeploy {
            ticks: setup_ticks_for(e.kind),
        });
    } else {
        e.set_desired_weapon_facing(target.facing);
    }
    e.replace_active_order(Order::artillery_point_fire(target.x, target.y));
    true
}

fn artillery_point_fire_basic_target(
    map: &Map,
    entities: &EntityStore,
    player: u32,
    unit: u32,
    x: f32,
    y: f32,
) -> Option<ArtilleryPointFireTarget> {
    let (x, y) = SmokeCloudStore::clamp_point_to_map(map, x, y)?;
    let e = entities.get(unit)?;
    if e.owner != player
        || e.kind != EntityKind::Artillery
        || e.hp == 0
        || e.under_construction()
        || !e.path_is_empty()
    {
        return None;
    }
    let dx = x - e.pos_x;
    let dy = y - e.pos_y;
    let distance2 = dx * dx + dy * dy;
    if !distance2.is_finite() {
        return None;
    }
    let min_px = config::ARTILLERY_MIN_RANGE_TILES as f32 * config::TILE_SIZE as f32;
    let max_px = config::ARTILLERY_MAX_RANGE_TILES as f32 * config::TILE_SIZE as f32;
    if distance2 < min_px * min_px || distance2 > max_px * max_px {
        return None;
    }
    let center = artillery_point_fire_field_center(e).filter(|facing| facing.is_finite())?;
    let facing = dy.atan2(dx);
    if !facing.is_finite() {
        return None;
    }
    let inside_field_of_fire =
        angle_delta(center, facing).abs() <= config::ARTILLERY_FIELD_OF_FIRE_RAD * 0.5;
    Some(ArtilleryPointFireTarget {
        x,
        y,
        facing,
        inside_field_of_fire,
    })
}

fn angle_delta(a: f32, b: f32) -> f32 {
    let mut d = (a - b).rem_euclid(std::f32::consts::TAU);
    if d > std::f32::consts::PI {
        d -= std::f32::consts::TAU;
    }
    d
}

fn artillery_can_accept_point_fire_command(e: &Entity) -> bool {
    matches!(e.weapon_setup(), WeaponSetup::Deployed)
        || (matches!(e.order(), Order::ArtilleryPointFire(_))
            && matches!(
                e.weapon_setup(),
                WeaponSetup::TearingDownToRedeploy { .. }
                    | WeaponSetup::Packed
                    | WeaponSetup::SettingUp { .. }
            ))
}

fn artillery_point_fire_field_center(e: &Entity) -> Option<f32> {
    match e.weapon_setup() {
        WeaponSetup::TearingDownToRedeploy { .. } => e.pending_redeploy_facing(),
        WeaponSetup::Packed | WeaponSetup::SettingUp { .. } => e.emplacement_facing(),
        _ => e.emplacement_facing().or_else(|| e.weapon_facing()),
    }
}

fn setup_ticks_for(kind: EntityKind) -> u16 {
    match kind {
        EntityKind::Artillery => config::ARTILLERY_SETUP_TICKS,
        _ => config::ANTI_TANK_GUN_SETUP_TICKS,
    }
}
