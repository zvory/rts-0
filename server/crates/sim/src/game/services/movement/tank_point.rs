use super::Occupancy;
use super::{
    pivot_drive::{rotate_toward, vehicle_body_turn_rate},
    standability,
};
use crate::game::entity::{EntityKind, EntityStore, Order};
use crate::game::map::Map;

pub(super) fn advance(map: &Map, entities: &mut EntityStore, occ: &Occupancy) {
    for id in entities.ids() {
        let Some(tank) = entities.get_mut(id) else {
            continue;
        };
        let Order::Point { facing } = tank.order() else {
            continue;
        };
        if tank.kind != EntityKind::Tank || tank.hp == 0 || !facing.is_finite() {
            continue;
        }
        let next = rotate_toward(tank.facing(), facing, vehicle_body_turn_rate(tank.kind) * tank.tank_turn_multiplier());
        if standability::unit_static_standable(occ, map, tank.kind, tank.pos_x, tank.pos_y, next) {
            tank.set_facing(next);
        }
    }
}
