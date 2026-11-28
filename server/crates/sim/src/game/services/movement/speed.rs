//! Shared movement rate for commanded travel and unit-relative Vector Field displacement.
use crate::config;
use crate::game::entity::{Entity, EntityKind, EntityStore};
use crate::game::map::Map;
use crate::game::services::spatial::SpatialIndex;
use crate::game::upgrade::UpgradeKind;
use crate::game::PlayerState;
use crate::rules::terrain::{movement_speed_multiplier, TerrainKind};

pub(super) fn movement_speed(
    map: &Map,
    entities: &EntityStore,
    players: &[PlayerState],
    spatial: &SpatialIndex,
    e: &Entity,
    on_wire: bool,
) -> f32 {
    let (kind, owner, x, y) = (e.kind, e.owner, e.pos_x(), e.pos_y());
    let has_meth = players
        .iter()
        .any(|p| p.id == owner && p.has_upgrade(UpgradeKind::Methamphetamines));
    let breakthrough_multiplier = breakthrough_speed_multiplier(
        entities,
        spatial,
        owner,
        x,
        y,
        e.breakthrough_ticks(),
        e.recent_smoke_ticks(),
    );
    let tank_plus = kind == EntityKind::Tank
        && players
            .iter()
            .any(|p| p.id == owner && p.has_upgrade(UpgradeKind::TankPlus));
    let speed_multiplier = if crate::rules::is_rifle_infantry(kind) && has_meth {
        config::METHAMPHETAMINES_SPEED_MULTIPLIER
    } else if breakthrough_multiplier > 1.0 {
        breakthrough_multiplier
    } else if kind == EntityKind::MachineGunner && has_meth {
        config::METHAMPHETAMINES_SPEED_MULTIPLIER
    } else {
        1.0
    };
    let terrain_speed_multiplier = {
        let (tx, ty) = map.tile_of(x, y);
        TerrainKind::from_map_code(map.terrain_at(tx, ty))
            .map(|terrain| movement_speed_multiplier(kind, terrain))
            .unwrap_or(1.0)
    };
    e.movement_speed()
        * speed_multiplier
        * if tank_plus {
            config::TANK_PLUS_SPEED_MULTIPLIER
        } else {
            1.0
        }
        * terrain_speed_multiplier
        * map.slow_movement_multiplier_at(x, y)
        * if rts_rules::barbed_wire_affects(kind) && on_wire {
            0.7
        } else {
            1.0
        }
}

/// Return the strongest current Command Car speed effect for an owned unit.
///
/// A timed Breakthrough status preserves its existing full-speed/smoke behavior. Otherwise an
/// owned, completed Command Car continuously supplies the smaller passive aura while in range.
fn breakthrough_speed_multiplier(
    entities: &EntityStore,
    spatial: &SpatialIndex,
    owner: u32,
    x: f32,
    y: f32,
    breakthrough_ticks: u16,
    recent_smoke_ticks: u16,
) -> f32 {
    if breakthrough_ticks > 0 {
        return if recent_smoke_ticks > 0 {
            config::BREAKTHROUGH_SMOKE_SPEED_MULTIPLIER
        } else {
            config::BREAKTHROUGH_BASE_SPEED_MULTIPLIER
        };
    }

    let radius_px = config::BREAKTHROUGH_RADIUS_TILES * config::TILE_SIZE as f32;
    let radius2 = radius_px * radius_px;
    let has_owned_command_car = spatial
        .ids_in_circle_bbox(x, y, radius_px)
        .any(|candidate| {
            let Some(car) = entities.get(candidate) else {
                return false;
            };
            if car.kind != EntityKind::CommandCar
                || car.owner != owner
                || car.hp == 0
                || car.under_construction()
            {
                return false;
            }
            let dx = car.pos_x() - x;
            let dy = car.pos_y() - y;
            dx * dx + dy * dy <= radius2
        });
    if has_owned_command_car {
        config::COMMAND_CAR_AURA_SPEED_MULTIPLIER
    } else {
        1.0
    }
}
