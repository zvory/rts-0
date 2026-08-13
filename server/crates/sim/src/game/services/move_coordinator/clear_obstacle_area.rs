use crate::config;
use crate::game::entity::{EntityStore, MovePhase, Order};
use crate::game::map::Map;

use super::{formation, formation_units, MoveCoordinator};

impl MoveCoordinator<'_> {
    /// Attack-move toward standable ground close enough to inspect the authoritative objective.
    pub fn order_group_clear_obstacle_area(
        &mut self,
        entities: &mut EntityStore,
        player: u32,
        ids: &[u32],
        anchor: u32,
        center: (f32, f32),
    ) {
        let units = formation_units(entities, player, ids);
        if units.is_empty() {
            return;
        }
        let approach = approach_center(self.map, &units, center);
        self.order_group_move(entities, player, ids, approach, true);
        for unit in units {
            let Some(goal) = entities
                .get(unit.id)
                .and_then(|entity| entity.move_intent())
            else {
                continue;
            };
            let Some(entity) = entities.get_mut(unit.id) else {
                continue;
            };
            entity.replace_active_order(Order::clear_obstacle_area_to(
                goal.0, goal.1, anchor, center.0, center.1,
            ));
            entity.set_path_goal(Some(goal));
            entity.mark_move_phase(MovePhase::AwaitingPath);
        }
    }
}

fn approach_center(
    map: &Map,
    units: &[formation::FormationUnit],
    center: (f32, f32),
) -> (f32, f32) {
    let centroid = units.iter().fold((0.0, 0.0), |sum, unit| {
        (sum.0 + unit.pos.0, sum.1 + unit.pos.1)
    });
    let centroid = (
        centroid.0 / units.len() as f32,
        centroid.1 / units.len() as f32,
    );
    let dx = centroid.0 - center.0;
    let dy = centroid.1 - center.1;
    let distance = dx.hypot(dy);
    let (nx, ny) = if distance.is_finite() && distance > 0.001 {
        (dx / distance, dy / distance)
    } else {
        (-1.0, 0.0)
    };
    // The shortest-sighted attack-move units can see the complete objective radius from here.
    let approach_distance =
        (config::TANK_TRAP_CLUSTER_ATTACK_RADIUS_TILES - 1.0).max(0.0) * config::TILE_SIZE as f32;
    (
        (center.0 + nx * approach_distance).clamp(0.0, map.world_width_px() - 0.01),
        (center.1 + ny * approach_distance).clamp(0.0, map.world_height_px() - 0.01),
    )
}
