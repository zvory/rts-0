use std::collections::{HashSet, VecDeque};
use std::sync::Arc;

use crate::config;
use crate::game::entity::{
    movement_body_class, static_blocker_class, Entity, EntityKind, EntityStore, MovementBodyClass,
    StaticBlockerClass,
};
use crate::game::map::Map;
use crate::game::pathfinding::Passability;
use crate::game::teams::TeamRelations;

const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
const TANK_TRAP_ROUTE_WINDOW_TILES: f32 = 6.0;
const TANK_TRAP_ON_ROUTE_RADIUS_TILES: f32 = 0.65;
const TANK_TRAP_PINCH_ROUTE_RADIUS_TILES: f32 = 0.45;

/// A snapshot of which tiles are blocked by buildings this tick, layered over terrain. Units
/// never block (soft overlap is allowed), so only static structures appear here.
#[derive(Clone)]
pub(crate) struct Occupancy<'a> {
    map: &'a Map,
    data: Arc<OccupancyData>,
}

struct OccupancyData {
    all_ground_blocked: Vec<bool>,
    vehicle_body_blocked: Vec<bool>,
    tank_trap_owner_by_tile: Vec<Option<u32>>,
    all_ground_clearance_tiles: Vec<u16>,
    vehicle_body_clearance_tiles: Vec<u16>,
    all_ground_static_fingerprint: u64,
    #[cfg(test)]
    vehicle_body_static_fingerprint: u64,
}

impl<'a> Occupancy<'a> {
    pub(crate) fn build(map: &'a Map, entities: &EntityStore) -> Self {
        Occupancy {
            map,
            data: Arc::new(OccupancyData::build(map, entities)),
        }
    }
}

impl OccupancyData {
    fn build(map: &Map, entities: &EntityStore) -> Self {
        let size = map.size;
        let mut all_ground_blocked = vec![false; (size * size) as usize];
        let mut vehicle_body_blocked = vec![false; (size * size) as usize];
        let mut tank_trap_owner_by_tile = vec![None; (size * size) as usize];
        let mut tank_trap_tiles = HashSet::new();
        for e in entities.iter() {
            if !e.is_building() {
                continue;
            }
            for (tx, ty) in building_footprint(map, e) {
                if tx < size && ty < size {
                    let idx = (ty * size + tx) as usize;
                    match static_blocker_class(e.kind) {
                        StaticBlockerClass::AllGround => all_ground_blocked[idx] = true,
                        StaticBlockerClass::VehicleBodyOnly => vehicle_body_blocked[idx] = true,
                        StaticBlockerClass::None => {}
                    }
                    if e.kind == EntityKind::TankTrap {
                        tank_trap_tiles.insert((tx, ty));
                        tank_trap_owner_by_tile[idx] = Some(e.owner);
                    }
                }
            }
        }
        close_tank_trap_vehicle_gaps(size, &tank_trap_tiles, &mut vehicle_body_blocked);
        let mut all_ground_static_blocked = vec![false; (size * size) as usize];
        let mut vehicle_body_static_blocked = vec![false; (size * size) as usize];
        for ty in 0..size {
            for tx in 0..size {
                let idx = (ty * size + tx) as usize;
                let terrain_blocked = !map.is_passable(tx as i32, ty as i32);
                all_ground_static_blocked[idx] = all_ground_blocked[idx] || terrain_blocked;
                vehicle_body_static_blocked[idx] =
                    all_ground_blocked[idx] || vehicle_body_blocked[idx] || terrain_blocked;
            }
        }
        let all_ground_clearance_tiles = build_clearance_field(map, &all_ground_static_blocked);
        let vehicle_body_clearance_tiles = build_clearance_field(map, &vehicle_body_static_blocked);
        let all_ground_static_fingerprint =
            static_blocked_fingerprint(size, &all_ground_static_blocked);
        #[cfg(test)]
        let vehicle_body_static_fingerprint =
            static_blocked_fingerprint(size, &vehicle_body_static_blocked);

        OccupancyData {
            all_ground_blocked,
            vehicle_body_blocked,
            tank_trap_owner_by_tile,
            all_ground_clearance_tiles,
            vehicle_body_clearance_tiles,
            all_ground_static_fingerprint,
            #[cfg(test)]
            vehicle_body_static_fingerprint,
        }
    }
}

impl Occupancy<'_> {
    /// Tile clearance from the nearest static blocker, in whole tiles. Blocked and out-of-bounds
    /// tiles report zero. Map edges count as static bounds, so edge-adjacent tiles have low
    /// clearance even on otherwise empty maps.
    pub(crate) fn clearance_at_tile(&self, tx: i32, ty: i32) -> u16 {
        self.clearance_at_tile_for_movement_body(tx, ty, MovementBodyClass::InfantryLike)
    }

    pub(crate) fn clearance_at_tile_for_movement_body(
        &self,
        tx: i32,
        ty: i32,
        movement_body_class: MovementBodyClass,
    ) -> u16 {
        if !self.map.in_bounds(tx, ty) {
            return 0;
        }
        let idx = (ty as u32 * self.map.size + tx as u32) as usize;
        match movement_body_class {
            MovementBodyClass::InfantryLike => self.data.all_ground_clearance_tiles[idx],
            MovementBodyClass::VehicleBody => self.data.vehicle_body_clearance_tiles[idx],
        }
    }

    /// Clearance at the tile containing a world-pixel point.
    #[allow(dead_code)]
    pub(crate) fn clearance_near_world_point(&self, x: f32, y: f32) -> u16 {
        if !x.is_finite() || !y.is_finite() || x < 0.0 || y < 0.0 {
            return 0;
        }
        let world_size = self.map.world_size_px();
        if x >= world_size || y >= world_size {
            return 0;
        }
        let ts = config::TILE_SIZE as f32;
        self.clearance_at_tile((x / ts).floor() as i32, (y / ts).floor() as i32)
    }

    /// Minimum static clearance sampled along a world-pixel segment.
    #[allow(dead_code)]
    pub(crate) fn min_clearance_along_segment(&self, from: (f32, f32), to: (f32, f32)) -> u16 {
        if !from.0.is_finite() || !from.1.is_finite() || !to.0.is_finite() || !to.1.is_finite() {
            return 0;
        }

        let dx = to.0 - from.0;
        let dy = to.1 - from.1;
        let distance = (dx * dx + dy * dy).sqrt();
        if !distance.is_finite() {
            return 0;
        }
        let step_px = config::TILE_SIZE as f32 / 4.0;
        let steps = (distance / step_px).ceil().max(1.0) as u32;
        let mut min_clearance = u16::MAX;

        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            let x = from.0 + dx * t;
            let y = from.1 + dy * t;
            min_clearance = min_clearance.min(self.clearance_near_world_point(x, y));
            if min_clearance == 0 {
                break;
            }
        }

        min_clearance
    }

    /// Fingerprint of the static blocker layer used to keep path-cache entries scoped to the
    /// terrain/building clearance field that produced them.
    #[cfg(test)]
    pub(crate) fn static_fingerprint(&self) -> u64 {
        self.static_fingerprint_for_movement_body(MovementBodyClass::InfantryLike)
    }

    #[cfg(test)]
    pub(crate) fn static_fingerprint_for_kind(&self, kind: EntityKind) -> u64 {
        self.static_fingerprint_for_movement_body(movement_body_class(kind))
    }

    pub(super) fn static_fingerprint_for_kind_and_relation(
        &self,
        kind: EntityKind,
        relation: &StaticPathingRelation,
    ) -> u64 {
        let movement_body_class = movement_body_class(kind);
        if movement_body_class == MovementBodyClass::InfantryLike {
            return self.data.all_ground_static_fingerprint;
        }

        let mut hash = FNV_OFFSET_BASIS;
        hash = fnv_mix(hash, self.map.size as u64);
        for ty in 0..self.map.size as i32 {
            for tx in 0..self.map.size as i32 {
                if self.static_blocked_for_pathing(tx, ty, movement_body_class, relation) {
                    let idx = (ty as u32 * self.map.size + tx as u32) as usize;
                    hash = fnv_mix(hash, idx as u64 + 1);
                }
            }
        }
        hash
    }

    #[cfg(test)]
    pub(crate) fn static_fingerprint_for_movement_body(
        &self,
        movement_body_class: MovementBodyClass,
    ) -> u64 {
        match movement_body_class {
            MovementBodyClass::InfantryLike => self.data.all_ground_static_fingerprint,
            MovementBodyClass::VehicleBody => self.data.vehicle_body_static_fingerprint,
        }
    }

    pub(crate) fn building_blocked_at_tile(&self, tx: i32, ty: i32) -> bool {
        let size = self.map.size as i32;
        if tx < 0 || ty < 0 || tx >= size || ty >= size {
            return false;
        }
        let idx = (ty * self.map.size as i32 + tx) as usize;
        self.data.all_ground_blocked[idx] || self.data.vehicle_body_blocked[idx]
    }

    pub(super) fn tank_trap_obstructs_vehicle_route(
        &self,
        attacker: &Entity,
        tank_trap: &Entity,
        relation: &StaticPathingRelation,
    ) -> bool {
        if movement_body_class(attacker.kind) != MovementBodyClass::VehicleBody
            || tank_trap.kind != EntityKind::TankTrap
            || relation.blocks_tank_trap_owned_by(tank_trap.owner)
        {
            return false;
        }

        let Some(route_target) = attacker
            .next_waypoint()
            .or_else(|| attacker.path_goal())
            .or_else(|| attacker.move_intent())
        else {
            return false;
        };
        let from = (attacker.pos_x, attacker.pos_y);
        if !world_point_finite(from) || !world_point_finite(route_target) {
            return false;
        }

        let segment = bounded_route_segment(from, route_target);
        let trap_pos = (tank_trap.pos_x, tank_trap.pos_y);
        if !world_point_finite(trap_pos) {
            return false;
        }

        let tile = self.map.tile_of(tank_trap.pos_x, tank_trap.pos_y);
        let on_route_radius = config::TILE_SIZE as f32 * TANK_TRAP_ON_ROUTE_RADIUS_TILES;
        if distance_sq_to_segment(trap_pos, segment.0, segment.1)
            <= on_route_radius * on_route_radius
        {
            return true;
        }

        self.tank_trap_pinches_route(tile, segment)
    }

    pub(crate) fn passable_for_kind(&self, tx: i32, ty: i32, kind: EntityKind) -> bool {
        self.passable_for_movement_body(tx, ty, movement_body_class(kind))
    }

    pub(super) fn passable_for_kind_and_relation(
        &self,
        tx: i32,
        ty: i32,
        kind: EntityKind,
        relation: &StaticPathingRelation,
    ) -> bool {
        self.passable_for_movement_body_and_relation(tx, ty, movement_body_class(kind), relation)
    }

    pub(super) fn clearance_at_tile_for_kind_and_relation(
        &self,
        tx: i32,
        ty: i32,
        kind: EntityKind,
        relation: &StaticPathingRelation,
    ) -> u16 {
        let movement_body_class = movement_body_class(kind);
        if movement_body_class == MovementBodyClass::InfantryLike {
            return self.clearance_at_tile_for_movement_body(tx, ty, movement_body_class);
        }
        if self.static_blocked_for_pathing(tx, ty, movement_body_class, relation) {
            return 0;
        }

        let size = self.map.size as i32;
        let edge_clearance = (tx + 1).min(ty + 1).min(size - tx).min(size - ty);
        let mut best = edge_clearance.max(0) as u16;
        for radius in 1i32..=3 {
            let mut found = false;
            for dy in -radius..=radius {
                for dx in -radius..=radius {
                    if dx.abs().max(dy.abs()) != radius {
                        continue;
                    }
                    if self.static_blocked_for_pathing(
                        tx + dx,
                        ty + dy,
                        movement_body_class,
                        relation,
                    ) {
                        found = true;
                        break;
                    }
                }
                if found {
                    break;
                }
            }
            if found {
                best = best.min(radius as u16);
                break;
            }
        }
        best
    }

    pub(crate) fn passable_for_movement_body(
        &self,
        tx: i32,
        ty: i32,
        movement_body_class: MovementBodyClass,
    ) -> bool {
        let size = self.map.size as i32;
        if tx < 0 || ty < 0 || tx >= size || ty >= size {
            return false;
        }
        let idx = (ty * self.map.size as i32 + tx) as usize;
        if self.data.all_ground_blocked[idx] {
            return false;
        }
        match movement_body_class {
            MovementBodyClass::InfantryLike => true,
            MovementBodyClass::VehicleBody => !self.data.vehicle_body_blocked[idx],
        }
    }

    fn passable_for_movement_body_and_relation(
        &self,
        tx: i32,
        ty: i32,
        movement_body_class: MovementBodyClass,
        relation: &StaticPathingRelation,
    ) -> bool {
        let size = self.map.size as i32;
        if tx < 0 || ty < 0 || tx >= size || ty >= size {
            return false;
        }
        let idx = (ty * self.map.size as i32 + tx) as usize;
        if self.data.all_ground_blocked[idx] {
            return false;
        }
        match movement_body_class {
            MovementBodyClass::InfantryLike => true,
            MovementBodyClass::VehicleBody => {
                !self.tank_trap_blocks_vehicle_pathing(tx, ty, relation)
            }
        }
    }

    fn static_blocked_for_pathing(
        &self,
        tx: i32,
        ty: i32,
        movement_body_class: MovementBodyClass,
        relation: &StaticPathingRelation,
    ) -> bool {
        if !self.map.in_bounds(tx, ty) {
            return true;
        }
        let idx = (ty as u32 * self.map.size + tx as u32) as usize;
        if !self.map.is_passable(tx, ty) || self.data.all_ground_blocked[idx] {
            return true;
        }
        movement_body_class == MovementBodyClass::VehicleBody
            && self.tank_trap_blocks_vehicle_pathing(tx, ty, relation)
    }

    fn tank_trap_blocks_vehicle_pathing(
        &self,
        tx: i32,
        ty: i32,
        relation: &StaticPathingRelation,
    ) -> bool {
        if self
            .tank_trap_owner_at(tx, ty)
            .is_some_and(|owner| relation.blocks_tank_trap_owned_by(owner))
        {
            return true;
        }
        for ((ax, ay), (bx, by)) in [((tx - 1, ty), (tx + 1, ty)), ((tx, ty - 1), (tx, ty + 1))] {
            let Some(owner_a) = self.tank_trap_owner_at(ax, ay) else {
                continue;
            };
            let Some(owner_b) = self.tank_trap_owner_at(bx, by) else {
                continue;
            };
            if relation.blocks_tank_trap_owned_by(owner_a)
                && relation.blocks_tank_trap_owned_by(owner_b)
            {
                return true;
            }
        }
        false
    }

    fn tank_trap_owner_at(&self, tx: i32, ty: i32) -> Option<u32> {
        if !self.map.in_bounds(tx, ty) {
            return None;
        }
        let idx = (ty as u32 * self.map.size + tx as u32) as usize;
        self.data.tank_trap_owner_by_tile[idx]
    }

    fn tank_trap_pinches_route(
        &self,
        tank_trap_tile: (u32, u32),
        segment: ((f32, f32), (f32, f32)),
    ) -> bool {
        let (tx, ty) = (tank_trap_tile.0 as i32, tank_trap_tile.1 as i32);
        for (other_tile, midpoint_tile) in [
            ((tx - 2, ty), (tx - 1, ty)),
            ((tx + 2, ty), (tx + 1, ty)),
            ((tx, ty - 2), (tx, ty - 1)),
            ((tx, ty + 2), (tx, ty + 1)),
        ] {
            if self
                .tank_trap_owner_at(other_tile.0, other_tile.1)
                .is_none()
            {
                continue;
            }
            if !self.map.in_bounds(midpoint_tile.0, midpoint_tile.1) {
                continue;
            }
            let midpoint = self
                .map
                .tile_center(midpoint_tile.0 as u32, midpoint_tile.1 as u32);
            let pinch_radius = config::TILE_SIZE as f32 * TANK_TRAP_PINCH_ROUTE_RADIUS_TILES;
            if distance_sq_to_segment(midpoint, segment.0, segment.1) <= pinch_radius * pinch_radius
            {
                return true;
            }
        }
        false
    }
}

fn world_point_finite(point: (f32, f32)) -> bool {
    point.0.is_finite() && point.1.is_finite()
}

fn bounded_route_segment(from: (f32, f32), to: (f32, f32)) -> ((f32, f32), (f32, f32)) {
    let dx = to.0 - from.0;
    let dy = to.1 - from.1;
    let distance = (dx * dx + dy * dy).sqrt();
    if !distance.is_finite() || distance <= f32::EPSILON {
        return (from, from);
    }

    let max_distance = config::TILE_SIZE as f32 * TANK_TRAP_ROUTE_WINDOW_TILES;
    if distance <= max_distance {
        return (from, to);
    }

    let scale = max_distance / distance;
    (from, (from.0 + dx * scale, from.1 + dy * scale))
}

fn distance_sq_to_segment(point: (f32, f32), from: (f32, f32), to: (f32, f32)) -> f32 {
    let vx = to.0 - from.0;
    let vy = to.1 - from.1;
    let wx = point.0 - from.0;
    let wy = point.1 - from.1;
    let len_sq = vx * vx + vy * vy;
    if len_sq <= f32::EPSILON {
        return wx * wx + wy * wy;
    }
    let t = ((wx * vx + wy * vy) / len_sq).clamp(0.0, 1.0);
    let closest_x = from.0 + t * vx;
    let closest_y = from.1 + t * vy;
    let dx = point.0 - closest_x;
    let dy = point.1 - closest_y;
    dx * dx + dy * dy
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(super) struct StaticPathingRelation {
    blocking_tank_trap_owners: Vec<u32>,
}

impl StaticPathingRelation {
    pub(super) fn for_player(player: u32, teams: &TeamRelations) -> Self {
        let mut owners = teams.same_team_player_ids(player);
        if player != 0 {
            owners.push(player);
        }
        owners.sort_unstable();
        owners.dedup();
        Self {
            blocking_tank_trap_owners: owners,
        }
    }

    #[cfg(test)]
    pub(super) fn single_owner(player: u32) -> Self {
        let owners = if player == 0 {
            Vec::new()
        } else {
            vec![player]
        };
        Self {
            blocking_tank_trap_owners: owners,
        }
    }

    fn blocks_tank_trap_owned_by(&self, owner: u32) -> bool {
        owner != 0 && self.blocking_tank_trap_owners.binary_search(&owner).is_ok()
    }
}

fn close_tank_trap_vehicle_gaps(
    size: u32,
    tank_trap_tiles: &HashSet<(u32, u32)>,
    vehicle_body_blocked: &mut [bool],
) {
    for &(tx, ty) in tank_trap_tiles {
        for (target, midpoint) in [
            ((tx.saturating_add(2), ty), (tx.saturating_add(1), ty)),
            ((tx, ty.saturating_add(2)), (tx, ty.saturating_add(1))),
        ] {
            if target.0 >= size || target.1 >= size || midpoint.0 >= size || midpoint.1 >= size {
                continue;
            }
            if tank_trap_tiles.contains(&target) {
                let idx = (midpoint.1 * size + midpoint.0) as usize;
                vehicle_body_blocked[idx] = true;
            }
        }
    }
}

impl Passability for Occupancy<'_> {
    /// All-ground static blockers only. Movement code should prefer `passable_for_kind` so
    /// vehicle-body requests include vehicle-only blockers.
    fn passable(&self, tx: i32, ty: i32) -> bool {
        self.passable_for_movement_body(tx, ty, MovementBodyClass::InfantryLike)
    }
}

fn build_clearance_field(map: &Map, static_blocked: &[bool]) -> Vec<u16> {
    let size = map.size as i32;
    let len = (map.size * map.size) as usize;
    let mut clearance = vec![u16::MAX; len];
    let mut queue = VecDeque::new();

    for ty in 0..size {
        for tx in 0..size {
            let idx = (ty as u32 * map.size + tx as u32) as usize;
            if static_blocked[idx] {
                clearance[idx] = 0;
                queue.push_back((tx, ty));
            }
        }
    }

    while let Some((tx, ty)) = queue.pop_front() {
        let idx = (ty as u32 * map.size + tx as u32) as usize;
        let next_clearance = clearance[idx].saturating_add(1);
        for dy in -1..=1 {
            for dx in -1..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let nx = tx + dx;
                let ny = ty + dy;
                if !map.in_bounds(nx, ny) {
                    continue;
                }
                let nidx = (ny as u32 * map.size + nx as u32) as usize;
                if next_clearance < clearance[nidx] {
                    clearance[nidx] = next_clearance;
                    queue.push_back((nx, ny));
                }
            }
        }
    }

    for ty in 0..size {
        for tx in 0..size {
            let idx = (ty as u32 * map.size + tx as u32) as usize;
            let edge_clearance = (tx + 1).min(ty + 1).min(size - tx).min(size - ty) as u16;
            clearance[idx] = clearance[idx].min(edge_clearance);
        }
    }

    clearance
}

fn static_blocked_fingerprint(size: u32, static_blocked: &[bool]) -> u64 {
    let mut hash = FNV_OFFSET_BASIS;
    hash = fnv_mix(hash, size as u64);
    for (idx, blocked) in static_blocked.iter().enumerate() {
        if *blocked {
            hash = fnv_mix(hash, idx as u64 + 1);
        }
    }
    hash
}

fn fnv_mix(hash: u64, value: u64) -> u64 {
    (hash ^ value).wrapping_mul(FNV_PRIME)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::terrain;

    fn flat_test_map(size: u32) -> Map {
        Map {
            size,
            terrain: vec![terrain::GRASS; (size * size) as usize],
            starts: vec![],
            expansion_sites: vec![],
        }
    }

    #[test]
    fn clearance_is_zero_on_static_blocked_tiles() {
        let mut map = flat_test_map(10);
        let rock = map.index(4, 4);
        map.terrain[rock] = terrain::ROCK;
        let entities = EntityStore::new();
        let occ = Occupancy::build(&map, &entities);

        assert_eq!(occ.clearance_at_tile(4, 4), 0);
        assert_eq!(occ.clearance_at_tile(-1, 4), 0);
        assert_eq!(occ.clearance_at_tile(10, 4), 0);
    }

    #[test]
    fn clearance_increases_away_from_terrain_blockers() {
        let mut map = flat_test_map(12);
        let rock = map.index(4, 4);
        map.terrain[rock] = terrain::ROCK;
        let entities = EntityStore::new();
        let occ = Occupancy::build(&map, &entities);

        assert_eq!(occ.clearance_at_tile(5, 4), 1);
        assert_eq!(occ.clearance_at_tile(6, 4), 2);
        assert_eq!(occ.clearance_at_tile(7, 4), 3);
    }

    #[test]
    fn building_occupancy_updates_clearance_and_fingerprint() {
        let map = flat_test_map(12);
        let empty = EntityStore::new();
        let before = Occupancy::build(&map, &empty);
        let clear_before = before.clearance_at_tile(6, 4);
        let fingerprint_before = before.static_fingerprint();

        let mut entities = EntityStore::new();
        let (bx, by) = footprint_center(&map, EntityKind::Depot, 4, 4);
        entities
            .spawn_building(1, EntityKind::Depot, bx, by, true)
            .expect("depot should spawn");
        let after = Occupancy::build(&map, &entities);

        assert_eq!(after.clearance_at_tile(4, 4), 0);
        assert_eq!(after.clearance_at_tile(5, 5), 0);
        assert!(
            after.clearance_at_tile(6, 4) < clear_before,
            "adjacent clearance should shrink after building placement"
        );
        assert_ne!(after.static_fingerprint(), fingerprint_before);
    }

    #[test]
    fn tank_trap_occupancy_blocks_vehicle_body_only() {
        let map = flat_test_map(12);
        let empty = EntityStore::new();
        let before = Occupancy::build(&map, &empty);
        let mut entities = EntityStore::new();
        let (bx, by) = footprint_center(&map, EntityKind::TankTrap, 5, 5);
        entities
            .spawn_building(1, EntityKind::TankTrap, bx, by, true)
            .expect("tank trap should spawn");
        let after = Occupancy::build(&map, &entities);

        assert!(after.passable_for_kind(5, 5, EntityKind::Worker));
        assert!(after.passable_for_kind(5, 5, EntityKind::Rifleman));
        for kind in [
            EntityKind::AntiTankGun,
            EntityKind::MortarTeam,
            EntityKind::Artillery,
            EntityKind::ScoutCar,
            EntityKind::Tank,
            EntityKind::CommandCar,
        ] {
            assert!(!after.passable_for_kind(5, 5, kind), "{kind:?}");
        }
        assert_eq!(
            before.static_fingerprint(),
            after.static_fingerprint(),
            "infantry/all-ground fingerprint should ignore vehicle-only blockers"
        );
        assert_ne!(
            before.static_fingerprint_for_kind(EntityKind::Tank),
            after.static_fingerprint_for_kind(EntityKind::Tank),
            "vehicle-body fingerprint should include Tank Trap blockers"
        );
    }

    #[test]
    fn pump_jack_occupancy_blocks_no_unit_body() {
        let map = flat_test_map(12);
        let empty = EntityStore::new();
        let before = Occupancy::build(&map, &empty);
        let mut entities = EntityStore::new();
        let (x, y) = footprint_center(&map, EntityKind::PumpJack, 5, 5);
        entities
            .spawn_building(1, EntityKind::PumpJack, x, y, true)
            .expect("pump jack should spawn");
        let after = Occupancy::build(&map, &entities);

        for kind in [
            EntityKind::Worker,
            EntityKind::Rifleman,
            EntityKind::AntiTankGun,
            EntityKind::MortarTeam,
            EntityKind::Artillery,
            EntityKind::ScoutCar,
            EntityKind::Tank,
            EntityKind::CommandCar,
        ] {
            assert!(after.passable_for_kind(5, 5, kind), "{kind:?}");
        }
        assert_eq!(
            before.static_fingerprint(),
            after.static_fingerprint(),
            "pump jack should not change infantry/all-ground static pathing"
        );
        assert_eq!(
            before.static_fingerprint_for_kind(EntityKind::Tank),
            after.static_fingerprint_for_kind(EntityKind::Tank),
            "pump jack should not change vehicle-body static pathing"
        );
    }

    #[test]
    fn enemy_tank_traps_are_breachable_for_vehicle_pathing_only() {
        let map = flat_test_map(12);
        let mut entities = EntityStore::new();
        let (bx, by) = footprint_center(&map, EntityKind::TankTrap, 5, 5);
        entities
            .spawn_building(2, EntityKind::TankTrap, bx, by, true)
            .expect("tank trap should spawn");
        let occ = Occupancy::build(&map, &entities);
        let enemies = TeamRelations::from_player_teams([(1, 1), (2, 2)]);
        let allies = TeamRelations::from_player_teams([(1, 7), (2, 7)]);
        let enemy_relation = StaticPathingRelation::for_player(1, &enemies);
        let allied_relation = StaticPathingRelation::for_player(1, &allies);

        assert!(!occ.passable_for_kind(5, 5, EntityKind::Tank));
        assert!(occ.passable_for_kind_and_relation(5, 5, EntityKind::Tank, &enemy_relation));
        assert!(!occ.passable_for_kind_and_relation(5, 5, EntityKind::Tank, &allied_relation));
    }

    #[test]
    fn under_construction_tank_trap_blocks_vehicle_body_occupancy() {
        let map = flat_test_map(12);
        let mut entities = EntityStore::new();
        let (bx, by) = footprint_center(&map, EntityKind::TankTrap, 5, 5);
        entities
            .spawn_building(1, EntityKind::TankTrap, bx, by, false)
            .expect("tank trap scaffold should spawn");
        let occ = Occupancy::build(&map, &entities);

        assert!(occ.passable_for_kind(5, 5, EntityKind::Worker));
        assert!(!occ.passable_for_kind(5, 5, EntityKind::Tank));
    }

    #[test]
    fn tank_trap_pairs_close_one_tile_vehicle_gap_only() {
        let map = flat_test_map(12);
        let mut entities = EntityStore::new();
        for (tile_x, tile_y) in [(5, 5), (5, 7), (8, 4), (10, 4)] {
            let (x, y) = footprint_center(&map, EntityKind::TankTrap, tile_x, tile_y);
            entities
                .spawn_building(1, EntityKind::TankTrap, x, y, true)
                .expect("tank trap should spawn");
        }
        let occ = Occupancy::build(&map, &entities);

        for (gap_x, gap_y) in [(5, 6), (9, 4)] {
            assert!(
                occ.passable_for_kind(gap_x, gap_y, EntityKind::Worker),
                "infantry should still pass through Tank Trap pair gap"
            );
            assert!(
                !occ.passable_for_kind(gap_x, gap_y, EntityKind::ScoutCar),
                "Scout Car should not pass through Tank Trap pair gap"
            );
            assert!(
                !occ.passable_for_kind(gap_x, gap_y, EntityKind::Tank),
                "Tank should not pass through Tank Trap pair gap"
            );
        }
    }

    #[test]
    fn owner_aware_tank_trap_gap_closure_uses_only_non_enemy_pairs() {
        let map = flat_test_map(12);
        let teams = TeamRelations::from_player_teams([(1, 1), (2, 2), (3, 1)]);
        let player_one_relation = StaticPathingRelation::for_player(1, &teams);
        let mut enemy_entities = EntityStore::new();
        for tile_y in [4, 6] {
            let (x, y) = footprint_center(&map, EntityKind::TankTrap, 5, tile_y);
            enemy_entities
                .spawn_building(2, EntityKind::TankTrap, x, y, true)
                .expect("enemy tank trap should spawn");
        }
        let enemy_occ = Occupancy::build(&map, &enemy_entities);
        assert!(enemy_occ.passable_for_kind_and_relation(
            5,
            5,
            EntityKind::ScoutCar,
            &player_one_relation
        ));

        let mut allied_entities = EntityStore::new();
        for (owner, tile_y) in [(1, 4), (3, 6)] {
            let (x, y) = footprint_center(&map, EntityKind::TankTrap, 5, tile_y);
            allied_entities
                .spawn_building(owner, EntityKind::TankTrap, x, y, true)
                .expect("allied tank trap should spawn");
        }
        let allied_occ = Occupancy::build(&map, &allied_entities);
        assert!(!allied_occ.passable_for_kind_and_relation(
            5,
            5,
            EntityKind::ScoutCar,
            &player_one_relation
        ));
    }

    #[test]
    fn tank_trap_route_obstruction_distinguishes_forward_from_irrelevant_traps() {
        let map = flat_test_map(12);
        let relation = StaticPathingRelation::for_player(
            1,
            &TeamRelations::from_player_teams([(1, 1), (2, 2)]),
        );
        let mut entities = EntityStore::new();
        let tank = entities
            .spawn_unit(1, EntityKind::Tank, 100.0, 100.0)
            .expect("tank should spawn");
        let forward = entities
            .spawn_building(2, EntityKind::TankTrap, 150.0, 100.0, true)
            .expect("forward trap should spawn");
        let side = entities
            .spawn_building(2, EntityKind::TankTrap, 150.0, 160.0, true)
            .expect("side trap should spawn");
        entities
            .get_mut(tank)
            .expect("tank should exist")
            .set_path_goal(Some((300.0, 100.0)));
        let occ = Occupancy::build(&map, &entities);
        let attacker = entities.get(tank).expect("tank should exist");

        assert!(occ.tank_trap_obstructs_vehicle_route(
            attacker,
            entities.get(forward).expect("forward trap should exist"),
            &relation
        ));
        assert!(!occ.tank_trap_obstructs_vehicle_route(
            attacker,
            entities.get(side).expect("side trap should exist"),
            &relation
        ));
    }

    #[test]
    fn tank_trap_route_obstruction_marks_closed_gap_on_route() {
        let map = flat_test_map(12);
        let relation = StaticPathingRelation::for_player(
            1,
            &TeamRelations::from_player_teams([(1, 1), (2, 2)]),
        );
        let mut entities = EntityStore::new();
        let tank = entities
            .spawn_unit(1, EntityKind::Tank, 100.0, 100.0)
            .expect("tank should spawn");
        let upper = entities
            .spawn_building(2, EntityKind::TankTrap, 160.0, 68.0, true)
            .expect("upper trap should spawn");
        entities
            .spawn_building(2, EntityKind::TankTrap, 160.0, 132.0, true)
            .expect("lower trap should spawn");
        entities
            .get_mut(tank)
            .expect("tank should exist")
            .set_path_goal(Some((300.0, 100.0)));
        let occ = Occupancy::build(&map, &entities);

        assert!(occ.tank_trap_obstructs_vehicle_route(
            entities.get(tank).expect("tank should exist"),
            entities.get(upper).expect("upper trap should exist"),
            &relation
        ));
    }

    #[test]
    fn world_point_and_segment_clearance_sample_static_field() {
        let mut map = flat_test_map(12);
        let rock = map.index(5, 5);
        map.terrain[rock] = terrain::ROCK;
        let entities = EntityStore::new();
        let occ = Occupancy::build(&map, &entities);
        let blocked_center = map.tile_center(5, 5);
        let open_center = map.tile_center(8, 5);

        assert_eq!(
            occ.clearance_near_world_point(blocked_center.0, blocked_center.1),
            0
        );
        assert!(occ.clearance_near_world_point(open_center.0, open_center.1) > 0);
        assert_eq!(
            occ.min_clearance_along_segment(map.tile_center(3, 5), map.tile_center(7, 5)),
            0
        );
        assert!(occ.min_clearance_along_segment(map.tile_center(8, 5), map.tile_center(9, 5)) > 0);
    }
}

/// The set of tiles a building's footprint covers, centered on its position. Footprints are
/// `foot_w × foot_h`; we center them on the tile under the building center.
pub(crate) fn building_footprint(map: &Map, e: &Entity) -> Vec<(u32, u32)> {
    let Some(s) = config::building_stats(e.kind) else {
        return Vec::new();
    };
    let (cx, cy) = map.tile_of(e.pos_x, e.pos_y);
    let mut out = Vec::with_capacity((s.foot_w * s.foot_h) as usize);
    // Offsets so the footprint is centered on the building's tile.
    let ox = s.foot_w as i32 / 2;
    let oy = s.foot_h as i32 / 2;
    for dy in 0..s.foot_h as i32 {
        for dx in 0..s.foot_w as i32 {
            let tx = cx as i32 + dx - ox;
            let ty = cy as i32 + dy - oy;
            if tx >= 0 && ty >= 0 {
                out.push((tx as u32, ty as u32));
            }
        }
    }
    out
}

/// The tiles a footprint of `building` would cover if its top-left tile were `(tile_x,
/// tile_y)`. The command specifies the top-left tile of the footprint.
pub(crate) fn footprint_tiles(building: EntityKind, tile_x: u32, tile_y: u32) -> Vec<(u32, u32)> {
    let Some(s) = config::building_stats(building) else {
        return Vec::new();
    };
    let mut out = Vec::with_capacity((s.foot_w * s.foot_h) as usize);
    for dy in 0..s.foot_h {
        for dx in 0..s.foot_w {
            // Guard against coordinate overflow on huge tile_x/tile_y. An empty footprint is
            // treated as not-placeable by `footprint_placeable`, so the build is cleanly rejected.
            let (Some(tx), Some(ty)) = (tile_x.checked_add(dx), tile_y.checked_add(dy)) else {
                return Vec::new();
            };
            out.push((tx, ty));
        }
    }
    out
}

/// World-pixel center of a footprint placed at top-left tile `(tile_x, tile_y)`.
pub(crate) fn footprint_center(
    map: &Map,
    building: EntityKind,
    tile_x: u32,
    tile_y: u32,
) -> (f32, f32) {
    let Some(s) = config::building_stats(building) else {
        return (0.0, 0.0);
    };
    let ts = config::TILE_SIZE as f32;
    let x = tile_x as f32 * ts + (s.foot_w as f32 * ts) * 0.5;
    let y = tile_y as f32 * ts + (s.foot_h as f32 * ts) * 0.5;
    // map is unused beyond stats here, kept for signature symmetry / future clamping.
    let _ = map;
    (x, y)
}
