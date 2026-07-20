use crate::config;
use crate::game::entity::{
    supports_manual_emplacement, Entity, EntityKind, EntityStore, Order, WeaponSetup,
};

pub(crate) mod targeting;

use targeting::ArtilleryPointFireTarget;

#[derive(Clone, Copy)]
pub(crate) enum FutureOrderMode {
    Preserve,
    Clear,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ArtilleryFireMode {
    Point,
    Blanket,
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
    if !supports_manual_emplacement(e.kind)
        || e.under_construction()
        || !x.is_finite()
        || !y.is_finite()
    {
        return false;
    }
    let facing = if e.kind == EntityKind::MortarTeam {
        e.facing()
    } else {
        (y - e.pos_y).atan2(x - e.pos_x)
    };
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
    if e.kind == EntityKind::MortarTeam
        && matches!(
            e.weapon_setup(),
            WeaponSetup::SettingUp { .. } | WeaponSetup::Deployed
        )
    {
        // Mortars have a full-circle field of fire. Reissuing their in-place setup while they are
        // already setting up or deployed is a terminal stop, not a needless redeploy cycle.
    } else if matches!(e.weapon_setup(), WeaponSetup::Packed) {
        e.set_emplacement_facing(Some(facing));
        e.set_desired_weapon_facing(facing);
    } else {
        e.set_pending_redeploy_facing(Some(facing));
        e.set_weapon_setup(WeaponSetup::TearingDownToRedeploy {
            ticks: teardown_ticks_for(e.kind),
        });
    }
    e.reset_gather_state();
    let (px, py) = (e.pos_x, e.pos_y);
    e.reset_stuck(px, py);
    true
}

pub(crate) fn execute_promoted_support_weapon_setup(
    entities: &mut EntityStore,
    id: u32,
    x: f32,
    y: f32,
) -> bool {
    let future_orders = if entities
        .get(id)
        .is_some_and(|entity| entity.kind == EntityKind::MortarTeam)
    {
        FutureOrderMode::Clear
    } else {
        FutureOrderMode::Preserve
    };
    execute_anti_tank_gun_setup(entities, id, x, y, future_orders)
}

fn is_artillery_entity(e: &Entity) -> bool {
    e.kind == EntityKind::Artillery
}

pub(crate) fn start_artillery_fire_command_order(
    entities: &mut EntityStore,
    unit: u32,
    target: ArtilleryPointFireTarget,
    mode: ArtilleryFireMode,
    radius_tiles: f32,
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
    start_artillery_fire_from_target(e, target, mode, radius_tiles)
}

pub(crate) fn start_artillery_fire_promoted_order(
    entities: &mut EntityStore,
    unit: u32,
    target: ArtilleryPointFireTarget,
    mode: ArtilleryFireMode,
    radius_tiles: f32,
) -> bool {
    let Some(e) = entities.get_mut(unit) else {
        return false;
    };
    e.clear_active_order();
    e.set_path_goal(None);
    start_artillery_fire_from_target(e, target, mode, radius_tiles)
}

fn start_artillery_fire_from_target(
    e: &mut Entity,
    target: ArtilleryPointFireTarget,
    mode: ArtilleryFireMode,
    radius_tiles: f32,
) -> bool {
    match e.weapon_setup() {
        WeaponSetup::Deployed if target.inside_field_of_fire => {
            e.set_desired_weapon_facing(target.facing);
        }
        WeaponSetup::Deployed => {
            e.set_pending_redeploy_facing(Some(target.facing));
            e.set_weapon_setup(WeaponSetup::TearingDownToRedeploy {
                ticks: setup_ticks_for(e.kind),
            });
        }
        WeaponSetup::TearingDownToRedeploy { .. } => {
            e.set_pending_redeploy_facing(Some(target.facing));
        }
        WeaponSetup::Packed | WeaponSetup::SettingUp { .. } => {
            e.set_emplacement_facing(Some(target.facing));
            e.set_desired_weapon_facing(target.facing);
        }
        WeaponSetup::TearingDown { .. } => {
            return false;
        }
    }
    match mode {
        ArtilleryFireMode::Point => {
            e.reset_artillery_blanket_sequence();
            e.replace_active_order(Order::artillery_point_fire(target.x, target.y));
        }
        ArtilleryFireMode::Blanket => {
            e.reset_artillery_blanket_sequence();
            e.replace_active_order(Order::artillery_blanket_fire(
                target.x,
                target.y,
                radius_tiles,
            ));
        }
    }
    true
}

fn setup_ticks_for(kind: EntityKind) -> u16 {
    match kind {
        EntityKind::Artillery => config::ARTILLERY_SETUP_TICKS,
        _ => config::ANTI_TANK_GUN_SETUP_TICKS,
    }
}

fn teardown_ticks_for(kind: EntityKind) -> u16 {
    match kind {
        EntityKind::MortarTeam => config::MORTAR_TEAM_TEARDOWN_TICKS,
        _ => setup_ticks_for(kind),
    }
}
