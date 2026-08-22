use super::{tree_detours, Occupancy, RoutePolicy, RouteShape};
use crate::game::entity::EntityKind;
use crate::game::map::Map;
use crate::game::pathfinding::Passability;
use crate::rules::terrain::{self, TerrainKind};

const VEHICLE_ADJACENT_BLOCKER_COST: u32 = 2;
const VEHICLE_CORNER_GRAZE_COST: u32 = 18;
const VEHICLE_DIAGONAL_BLOCKER_COST: u32 = 3;

/// Passability oracle that layers terrain, occupancy, body clearance, and route shaping.
pub(super) struct TerrainPassability<'a> {
    pub(super) map: &'a Map,
    pub(super) occupancy: &'a Occupancy<'a>,
    pub(super) kind: EntityKind,
    pub(super) radius_tiles: u32,
    pub(super) route_shape: RouteShape,
    pub(super) policy: RoutePolicy,
    /// When true, reject tiles pinched between two diagonally-opposite blocked corners.
    /// Used for oriented vehicle bodies so A* avoids 1-tile gaps that the rotating hull
    /// cannot legally thread (see docs/design/server-sim.md pathing notes).
    pub(super) avoid_diagonal_pinch: bool,
}

impl TerrainPassability<'_> {
    fn tile_passable(&self, tx: i32, ty: i32) -> bool {
        if !self.map.in_bounds(tx, ty) {
            return false;
        }
        let Some(terrain_kind) =
            TerrainKind::from_map_code(self.map.terrain_at(tx as u32, ty as u32))
        else {
            return false;
        };
        if !terrain::movement_allowed(self.kind, terrain_kind) {
            return false;
        }
        self.occupancy.passable_for_kind(tx, ty, self.kind)
    }

    pub(super) fn vehicle_corner_cost(&self, tx: i32, ty: i32) -> u32 {
        let n = !self.tile_passable(tx, ty - 1);
        let e = !self.tile_passable(tx + 1, ty);
        let s = !self.tile_passable(tx, ty + 1);
        let w = !self.tile_passable(tx - 1, ty);
        let nw = !self.tile_passable(tx - 1, ty - 1);
        let ne = !self.tile_passable(tx + 1, ty - 1);
        let se = !self.tile_passable(tx + 1, ty + 1);
        let sw = !self.tile_passable(tx - 1, ty + 1);

        let adjacent_blockers = [n, e, s, w].into_iter().filter(|blocked| *blocked).count() as u32;
        let diagonal_blockers = [nw, ne, se, sw]
            .into_iter()
            .filter(|blocked| *blocked)
            .count() as u32;
        let grazes_corner = (w || e) && (s || n);

        adjacent_blockers * VEHICLE_ADJACENT_BLOCKER_COST
            + diagonal_blockers * VEHICLE_DIAGONAL_BLOCKER_COST
            + if grazes_corner {
                VEHICLE_CORNER_GRAZE_COST
            } else {
                0
            }
    }
}

impl Passability for TerrainPassability<'_> {
    fn dimensions(&self) -> (u32, u32) {
        (self.map.width, self.map.height)
    }

    fn passable(&self, tx: i32, ty: i32) -> bool {
        let r = self.radius_tiles as i32;
        for dy in -r..=r {
            for dx in -r..=r {
                if !self.tile_passable(tx + dx, ty + dy) {
                    return false;
                }
            }
        }
        if self.avoid_diagonal_pinch {
            let nw = !self.tile_passable(tx - 1, ty - 1);
            let ne = !self.tile_passable(tx + 1, ty - 1);
            let sw = !self.tile_passable(tx - 1, ty + 1);
            let se = !self.tile_passable(tx + 1, ty + 1);
            if (nw && se) || (ne && sw) {
                return false;
            }
        }
        true
    }

    fn movement_cost(&self, tx: i32, ty: i32, base_step_cost: u32) -> u32 {
        tree_detours::movement_cost(self, tx, ty, base_step_cost)
    }

    fn edge_cost(
        &self,
        from_tx: i32,
        from_ty: i32,
        dx: i32,
        dy: i32,
        base_step_cost: u32,
    ) -> Option<u32> {
        let to_tx = from_tx + dx;
        let to_ty = from_ty + dy;
        if !self.passable(to_tx, to_ty)
            || (dx != 0
                && dy != 0
                && (!self.passable(from_tx + dx, from_ty) || !self.passable(from_tx, from_ty + dy)))
        {
            return None;
        }
        if self.policy != RoutePolicy::FastestTerrainTime {
            return Some(base_step_cost.saturating_add(self.movement_cost(
                to_tx,
                to_ty,
                base_step_cost,
            )));
        }
        Some(
            super::route_cost::RouteCostModel::new(self.map)
                .edge_cost((from_tx, from_ty), (to_tx, to_ty), base_step_cost)?
                .saturating_add(tree_detours::weighted_tree_avoidance_cost(
                    self.occupancy.tree_path_avoidance_cost(to_tx, to_ty),
                )),
        )
    }

    fn heuristic_step_costs(&self) -> (u32, u32) {
        if self.policy == RoutePolicy::FastestTerrainTime {
            (
                terrain::MIN_TERRAIN_CARDINAL_ROUTE_COST,
                terrain::MIN_TERRAIN_DIAGONAL_ROUTE_COST,
            )
        } else {
            (10, 14)
        }
    }
}
