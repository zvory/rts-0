//! Known-world ground routes. These are planning hints, never authoritative path acceptance.
use super::{AiTile, AiWorldPoint, EntityKind, WorldQueries};
use std::cmp::Reverse;
use std::collections::{BTreeSet, BinaryHeap, VecDeque};

/// A body-aware known-world connected component. Reuse it for every unit whose position
/// is contained in the component; transient unit bodies and hidden entities are absent.
pub struct AiGroundReachability {
    grid: RouteGrid,
    reached: Vec<bool>,
}
impl AiGroundReachability {
    pub fn contains(&self, point: AiWorldPoint) -> bool {
        self.grid
            .world_index(point.as_tuple())
            .and_then(|i| self.reached.get(i))
            .copied()
            .unwrap_or(false)
    }
}

impl WorldQueries<'_> {
    /// Flood one reusable known-world component for a specified movement body.
    pub fn ground_reachability(
        &self,
        from: AiWorldPoint,
        mover: EntityKind,
    ) -> Option<AiGroundReachability> {
        let grid = RouteGrid::new(self, mover)?;
        let start = grid.world_index(from.as_tuple())?;
        if !grid.walk[start] {
            return None;
        }
        let reached = grid.component(&[start]);
        Some(AiGroundReachability { grid, reached })
    }

    /// Shortest octile ground route through public terrain and current known footprints.
    /// Own/currently visible blocking buildings obstruct it; walk-through mining structures,
    /// resource nodes, transient unit bodies and
    /// remembered or hidden entities do not. Unknown terrain is blocked. Diagonal steps cannot
    /// cut blocked corners. Tile-center clearance uses the mover's authoritative body radius.
    /// This neither guarantees dynamic pathing nor claims the route is safe from hidden enemies.
    pub fn ground_route(
        &self,
        from: AiWorldPoint,
        to: AiWorldPoint,
        mover: EntityKind,
    ) -> Option<Vec<AiWorldPoint>> {
        let grid = RouteGrid::new(self, mover)?;
        let start = grid.world_index(from.as_tuple())?;
        let goal = grid.world_index(to.as_tuple())?;
        let path = grid.search(start, &BTreeSet::from([goal]))?;
        let mut result = self.route_points(&grid, path)?;
        if let Some(first) = result.first_mut() {
            *first = from;
        }
        if let Some(last) = result.last_mut() {
            *last = to;
        }
        Some(result)
    }

    /// Route to a reachable exterior edge of a proposed or existing building footprint.
    /// A single multi-goal search selects the nearest edge; the building's interior is never
    /// used as a destination or shortcut. A mover already inside an unbuilt footprint may
    /// leave it; existing known building blockers still apply. Suitable for issuing/resuming Build.
    pub fn ground_route_to_build_site(
        &self,
        from: AiWorldPoint,
        kind: EntityKind,
        tile: AiTile,
        mover: EntityKind,
    ) -> Option<Vec<AiWorldPoint>> {
        let stats = rts_rules::balance::building_stats(kind)?;
        let right = tile.x().checked_add(stats.foot_w)?;
        let bottom = tile.y().checked_add(stats.foot_h)?;
        if right > self.frame.map().width || bottom > self.frame.map().height {
            return None;
        }
        let mut grid = RouteGrid::new(self, mover)?;
        let start = grid.world_index(from.as_tuple())?;
        let ts = self.frame.map().tile_size as f32;
        let rect = (
            tile.x() as f32 * ts,
            tile.y() as f32 * ts,
            right as f32 * ts,
            bottom as f32 * ts,
        );
        if !(from.x() >= rect.0 && from.x() < rect.2 && from.y() >= rect.1 && from.y() < rect.3) {
            grid.block_rect(rect);
        }
        let goals = grid.exterior_points(rect, (grid.radius / grid.size + 0.5).ceil().max(1.));
        let path = grid.search(start, &goals)?;
        let mut result = self.route_points(&grid, path)?;
        if let Some(first) = result.first_mut() {
            *first = from;
        }
        Some(result)
    }

    /// Conservative current-threat corridor around every route segment. Only currently visible,
    /// living enemies whose default weapon can damage the traveller contribute. Authoritative
    /// default range plus public trench occupation and body clearance define the corridor; LOS,
    /// cooldowns, hidden upgrades and minimum-range dead zones are deliberately not treated as
    /// promises of safety. An empty route is uncertain and therefore fails closed.
    pub fn route_has_visible_ground_threat(
        &self,
        route: &[AiWorldPoint],
        traveller: EntityKind,
    ) -> bool {
        if route.is_empty() {
            return true;
        }
        let size = self.frame.map().tile_size as f32;
        let body = rts_rules::balance::unit_placement_radius(traveller);
        self.visible_enemies().iter().any(|enemy| {
            if enemy.health.current == 0
                || !rts_rules::target::default_weapon_can_target(enemy.kind, traveller)
            {
                return false;
            }
            let profile = rts_rules::combat::attack_profile(enemy.kind);
            if profile.dmg == 0 || profile.cooldown == 0 {
                return false;
            }
            let trench = if enemy.occupied_trench_id.is_some() {
                rts_rules::balance::ENTRENCHMENT_RANGE_BONUS_TILES as f32
            } else {
                0.
            };
            let attacker_body = rts_rules::balance::building_stats(enemy.kind).map_or_else(
                || rts_rules::balance::unit_placement_radius(enemy.kind),
                |stats| stats.foot_w.max(stats.foot_h) as f32 * size * 0.5,
            );
            let range = (profile.range_tiles + trench) * size + body + attacker_body;
            let close = |a: AiWorldPoint, b: AiWorldPoint| {
                segment_distance2(enemy.position, a.as_tuple(), b.as_tuple()) <= range * range
            };
            close(route[0], route[0]) || route.windows(2).any(|pair| close(pair[0], pair[1]))
        })
    }

    /// Validate multi-side, body-aware production egress after a proposed footprint.
    /// Existing already-isolated producers do not veto every future construction request.
    pub(crate) fn placement_preserves_producer_access(
        &self,
        building: EntityKind,
        tile: AiTile,
    ) -> bool {
        let Some(stats) = rts_rules::balance::building_stats(building) else {
            return false;
        };
        let size = self.frame.map().tile_size as f32;
        let candidate = (
            tile.x() as f32 * size,
            tile.y() as f32 * size,
            (tile.x() + stats.foot_w) as f32 * size,
            (tile.y() + stats.foot_h) as f32 * size,
        );
        let largest = |kind| {
            rts_rules::economy::trainable_units_for_faction(self.frame.faction_id(), kind)
                .iter()
                .copied()
                .filter(|kind| kind.is_unit())
                .max_by(|a, b| {
                    rts_rules::balance::unit_placement_radius(*a)
                        .total_cmp(&rts_rules::balance::unit_placement_radius(*b))
                })
        };
        let mut producers = self
            .owned()
            .iter()
            .filter_map(|entity| {
                let kind = largest(entity.kind)?;
                let stats = rts_rules::balance::building_stats(entity.kind)?;
                let hw = stats.foot_w as f32 * size / 2.;
                let hh = stats.foot_h as f32 * size / 2.;
                Some((
                    kind,
                    (
                        entity.position.0 - hw,
                        entity.position.1 - hh,
                        entity.position.0 + hw,
                        entity.position.1 + hh,
                    ),
                    false,
                ))
            })
            .collect::<Vec<_>>();
        if let Some(kind) = largest(building) {
            producers.push((kind, candidate, true));
        }
        let mut grids = std::collections::BTreeMap::new();
        for (mover, rect, new) in producers {
            let base = grids
                .entry(mover)
                .or_insert_with(|| RouteGrid::new(self, mover));
            let Some(base) = base else { return false };
            let mut after = base.clone();
            after.block_rect(candidate);
            // The simulation samples all footprint sides, out to six map-tile rings.
            // Use its tile-center candidates rather than assuming a bottom-center exit.
            let exits = |grid: &RouteGrid| {
                let mut result = Vec::new();
                for ring in 1..=6 {
                    for i in grid.exterior_points(rect, ring as f32 * size / grid.size) {
                        let p = grid.point(i);
                        let tx = p.0 / size - 0.5;
                        let ty = p.1 / size - 0.5;
                        if (tx.round() - tx).abs() > 0.001 || (ty.round() - ty).abs() > 0.001 {
                            continue;
                        }
                        let r = (rect.0 / size - tx)
                            .max(tx - (rect.2 / size - 1.))
                            .max(rect.1 / size - ty)
                            .max(ty - (rect.3 / size - 1.));
                        if (r - ring as f32).abs() < 0.001 {
                            result.push(i);
                        }
                    }
                    // Native production prefers the first ring with full-rotation clearance.
                    if !result.is_empty() {
                        break;
                    }
                }
                result
            };
            let escaping = |grid: &RouteGrid| {
                let starts = exits(grid);
                if starts.is_empty() {
                    return false;
                }
                let distance = (6. * size).min(grid.world_width.min(grid.world_height) * 0.25);
                let mut accounted = vec![false; grid.walk.len()];
                for start in starts {
                    if accounted[start] {
                        continue;
                    }
                    let component = grid.component(&[start]);
                    let leaves = component.iter().enumerate().any(|(i, reached)| {
                        let p = grid.point(i);
                        *reached
                            && (p.0 < rect.0 - distance
                                || p.0 > rect.2 + distance
                                || p.1 < rect.1 - distance
                                || p.1 > rect.3 + distance)
                    });
                    if !leaves {
                        return false;
                    }
                    for (known, reached) in accounted.iter_mut().zip(component) {
                        *known |= reached;
                    }
                }
                true
            };
            if (new || escaping(base)) && !escaping(&after) {
                return false;
            }
        }
        true
    }

    fn route_points(&self, grid: &RouteGrid, path: Vec<usize>) -> Option<Vec<AiWorldPoint>> {
        path.into_iter()
            .map(|i| {
                let point = grid.point(i);
                self.world_point(point.0, point.1)
            })
            .collect()
    }
}

fn segment_distance2(point: (f32, f32), from: (f32, f32), to: (f32, f32)) -> f32 {
    let dx = to.0 - from.0;
    let dy = to.1 - from.1;
    let length2 = dx * dx + dy * dy;
    let t = if length2 > 0. {
        (((point.0 - from.0) * dx + (point.1 - from.1) * dy) / length2).clamp(0., 1.)
    } else {
        0.
    };
    let x = point.0 - (from.0 + t * dx);
    let y = point.1 - (from.1 + t * dy);
    x * x + y * y
}

#[derive(Clone)]
struct RouteGrid {
    w: usize,
    h: usize,
    size: f32,
    offset: f32,
    world_width: f32,
    world_height: f32,
    radius: f32,
    blocked_tree_edges: Vec<u8>,
    walk: Vec<bool>,
}
impl RouteGrid {
    fn new(query: &WorldQueries<'_>, mover: EntityKind) -> Option<Self> {
        let map = query.frame.map();
        let count = usize::try_from(map.width.checked_mul(map.height)?).ok()?;
        if map.width == 0 || map.height == 0 || map.tile_size == 0 || count > map.terrain.len() {
            return None;
        }
        let radius = rts_rules::balance::unit_placement_radius(mover);
        let vehicle =
            rts_rules::movement_body_class(mover) == rts_rules::MovementBodyClass::VehicleBody;
        // Wide bodies need tile-boundary samples too: a64px lane is passable by a61px
        // rotating Tank body, but neither32px tile-center sample lies on its centerline.
        let wide = radius > map.tile_size as f32 * 0.5;
        let scale = if wide { 2 } else { 1 };
        let w = map.width as usize * scale + usize::from(wide);
        let h = map.height as usize * scale + usize::from(wide);
        let mut grid = Self {
            w,
            h,
            size: map.tile_size as f32 / scale as f32,
            offset: if wide { 0. } else { 0.5 },
            world_width: map.width as f32 * map.tile_size as f32,
            world_height: map.height as f32 * map.tile_size as f32,
            radius,
            blocked_tree_edges: vec![0; w.checked_mul(h)?],
            walk: vec![true; w.checked_mul(h)?],
        };
        let tile_size = map.tile_size as f32;
        // Inflate static blocked cells by the mover radius as well as known footprints.
        for (i, terrain) in map.terrain.iter().take(count).enumerate() {
            let tile = (i as u32 % map.width, i as u32 / map.width);
            if matches!(
                terrain,
                super::AiTerrain::Rock | super::AiTerrain::Water | super::AiTerrain::Unknown(_)
            ) || map.unwalkable_tiles.contains(&tile)
                || vehicle && map.no_vehicle_tiles.contains(&tile)
            {
                let x = tile.0 as f32 * tile_size;
                let y = tile.1 as f32 * tile_size;
                grid.block_rect((x, y, x + tile_size, y + tile_size));
            }
        }
        for i in 0..grid.walk.len() {
            let p = grid.point(i);
            if p.0 < grid.radius
                || p.1 < grid.radius
                || p.0 + grid.radius > grid.world_width
                || p.1 + grid.radius > grid.world_height
            {
                grid.walk[i] = false;
            }
        }
        for entity in query
            .owned()
            .iter()
            .chain(query.visible_allies())
            .chain(query.visible_enemies())
            .chain(query.frame.tank_traps())
        {
            let class = rts_rules::static_blocker_class(entity.kind);
            if class == rts_rules::StaticBlockerClass::None
                || class == rts_rules::StaticBlockerClass::VehicleBodyOnly && !vehicle
            {
                continue;
            }
            let Some(stats) = rts_rules::balance::building_stats(entity.kind) else {
                continue;
            };
            // Automatic extractors retain rotated patch positions, but authoritative
            // building collision uses their quantized tile footprint, not a centered box.
            let tx = (entity.position.0 / tile_size).floor() as u32;
            let ty = (entity.position.1 / tile_size).floor() as u32;
            let Some(tx) = tx.checked_sub(stats.foot_w / 2) else {
                continue;
            };
            let Some(ty) = ty.checked_sub(stats.foot_h / 2) else {
                continue;
            };
            let x = tx as f32 * tile_size;
            let y = ty as f32 * tile_size;
            grid.block_rect((
                x,
                y,
                x + stats.foot_w as f32 * tile_size,
                y + stats.foot_h as f32 * tile_size,
            ));
        }
        if vehicle {
            let clearance = radius + rts_rules::balance::TREE_TRUNK_RADIUS_PX;
            let radius2 = clearance * clearance;
            // Precompute local swept-edge rejection around each trunk. Floods and route
            // searches then use O(1) edge masks, rather than scanning every tree per step.
            for &(x, y) in &map.tree_trunks {
                let tree = (x as f32, y as f32);
                let padding = clearance + grid.size;
                let left = ((tree.0 - padding) / grid.size - grid.offset)
                    .floor()
                    .max(0.) as usize;
                let top = ((tree.1 - padding) / grid.size - grid.offset)
                    .floor()
                    .max(0.) as usize;
                let right = ((tree.0 + padding) / grid.size - grid.offset)
                    .ceil()
                    .max(0.) as usize;
                let bottom = ((tree.1 + padding) / grid.size - grid.offset)
                    .ceil()
                    .max(0.) as usize;
                for yy in top..=bottom.min(grid.h.saturating_sub(1)) {
                    for xx in left..=right.min(grid.w.saturating_sub(1)) {
                        let i = yy * grid.w + xx;
                        let p = grid.point(i);
                        let dx = p.0 - tree.0;
                        let dy = p.1 - tree.1;
                        if dx * dx + dy * dy <= radius2 {
                            grid.walk[i] = false;
                        }
                        for (direction, (dx, dy)) in [
                            (-1, 0),
                            (1, 0),
                            (0, -1),
                            (0, 1),
                            (-1, -1),
                            (-1, 1),
                            (1, -1),
                            (1, 1),
                        ]
                        .into_iter()
                        .enumerate()
                        {
                            let nx = xx as i64 + dx;
                            let ny = yy as i64 + dy;
                            if nx < 0 || ny < 0 || nx >= grid.w as i64 || ny >= grid.h as i64 {
                                continue;
                            }
                            let j = ny as usize * grid.w + nx as usize;
                            if segment_distance2(tree, p, grid.point(j)) <= radius2 {
                                grid.blocked_tree_edges[i] |= 1 << direction;
                            }
                        }
                    }
                }
            }
        }
        Some(grid)
    }
    fn world_index(&self, point: (f32, f32)) -> Option<usize> {
        if !point.0.is_finite()
            || !point.1.is_finite()
            || point.0 < 0.
            || point.1 < 0.
            || point.0 >= self.world_width
            || point.1 >= self.world_height
        {
            return None;
        }
        let x = (point.0 / self.size - self.offset).round();
        let y = (point.1 / self.size - self.offset).round();
        if x < 0. || y < 0. {
            return None;
        }
        self.index((x as u32, y as u32))
    }
    fn exterior_points(&self, rect: (f32, f32, f32, f32), padding: f32) -> BTreeSet<usize> {
        let pad = padding * self.size;
        let left = ((rect.0 - pad) / self.size - self.offset).floor().max(0.) as usize;
        let top = ((rect.1 - pad) / self.size - self.offset).floor().max(0.) as usize;
        let right = ((rect.2 + pad) / self.size - self.offset).ceil().max(0.) as usize;
        let bottom = ((rect.3 + pad) / self.size - self.offset).ceil().max(0.) as usize;
        let mut points = BTreeSet::new();
        for y in top..=bottom.min(self.h.saturating_sub(1)) {
            for x in left..=right.min(self.w.saturating_sub(1)) {
                let i = y * self.w + x;
                let p = self.point(i);
                if p.0 < rect.0 - pad
                    || p.0 > rect.2 + pad
                    || p.1 < rect.1 - pad
                    || p.1 > rect.3 + pad
                {
                    continue;
                }
                if self.walk[i]
                    && !(p.0 >= rect.0 && p.0 <= rect.2 && p.1 >= rect.1 && p.1 <= rect.3)
                {
                    points.insert(i);
                }
            }
        }
        points
    }
    fn neighbors(&self, i: usize) -> impl Iterator<Item = usize> + '_ {
        let x = (i % self.w) as i64;
        let y = (i / self.w) as i64;
        [
            (-1, 0),
            (1, 0),
            (0, -1),
            (0, 1),
            (-1, -1),
            (-1, 1),
            (1, -1),
            (1, 1),
        ]
        .into_iter()
        .enumerate()
        .filter_map(move |(direction, (dx, dy))| {
            if self.blocked_tree_edges[i] & (1 << direction) != 0 {
                return None;
            }
            let nx = x + dx;
            let ny = y + dy;
            if nx < 0 || ny < 0 || nx >= self.w as i64 || ny >= self.h as i64 {
                return None;
            }
            let j = ny as usize * self.w + nx as usize;
            (self.walk[j]
                && (dx == 0
                    || dy == 0
                    || (self.walk[y as usize * self.w + nx as usize]
                        && self.walk[ny as usize * self.w + x as usize])))
                .then_some(j)
        })
    }
    fn component(&self, starts: &[usize]) -> Vec<bool> {
        let mut reached = vec![false; self.walk.len()];
        let mut queue = VecDeque::new();
        for &i in starts {
            if self.walk.get(i).copied().unwrap_or(false) && !reached[i] {
                reached[i] = true;
                queue.push_back(i);
            }
        }
        while let Some(i) = queue.pop_front() {
            for j in self.neighbors(i) {
                if !reached[j] {
                    reached[j] = true;
                    queue.push_back(j);
                }
            }
        }
        reached
    }
    fn index(&self, (x, y): (u32, u32)) -> Option<usize> {
        (x < self.w as u32 && y < self.h as u32).then_some(y as usize * self.w + x as usize)
    }
    fn point(&self, i: usize) -> (f32, f32) {
        (
            ((i % self.w) as f32 + self.offset) * self.size,
            ((i / self.w) as f32 + self.offset) * self.size,
        )
    }
    fn block_rect(&mut self, rect: (f32, f32, f32, f32)) {
        if ![rect.0, rect.1, rect.2, rect.3]
            .iter()
            .all(|v| v.is_finite())
        {
            return;
        }
        let left = ((rect.0 - self.radius) / self.size).floor().max(0.) as usize;
        let top = ((rect.1 - self.radius) / self.size).floor().max(0.) as usize;
        let right = ((rect.2 + self.radius) / self.size).ceil().max(0.) as usize;
        let bottom = ((rect.3 + self.radius) / self.size).ceil().max(0.) as usize;
        for y in top..bottom.min(self.h) {
            for x in left..right.min(self.w) {
                let i = y * self.w + x;
                if super::unit_circle_touches_rect(self.point(i), self.radius, rect) {
                    self.walk[i] = false;
                }
            }
        }
    }
    fn search(&self, start: usize, goals: &BTreeSet<usize>) -> Option<Vec<usize>> {
        if !self.walk.get(start).copied().unwrap_or(false) || goals.is_empty() {
            return None;
        }
        let mut costs = vec![u32::MAX; self.walk.len()];
        let mut parents = vec![None; self.walk.len()];
        let mut open = BinaryHeap::new();
        costs[start] = 0;
        open.push(Reverse((0u32, start)));
        while let Some(Reverse((cost, i))) = open.pop() {
            if costs[i] != cost {
                continue;
            }
            if goals.contains(&i) {
                let mut path = vec![i];
                let mut cursor = i;
                while cursor != start {
                    cursor = *parents.get(cursor)?.as_ref()?;
                    path.push(cursor);
                }
                path.reverse();
                return Some(path);
            }
            for next in self.neighbors(i) {
                let diagonal = i % self.w != next % self.w && i / self.w != next / self.w;
                let next_cost = cost.saturating_add(if diagonal { 14 } else { 10 });
                if next_cost < costs[next] {
                    costs[next] = next_cost;
                    parents[next] = Some(i);
                    open.push(Reverse((next_cost, next)));
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rts_sim::game::{Game, PlayerInit};
    use rts_sim::protocol::{MapTile, ResourceNode};

    fn fixture() -> (rts_sim::protocol::StartPayload, rts_sim::protocol::Snapshot) {
        let players = [1, 2].map(|id| PlayerInit {
            id,
            team_id: id,
            faction_id: "kriegsia".into(),
            name: format!("P{id}"),
            color: "#123456".into(),
            is_ai: true,
        });
        let game = Game::new_without_ai_controllers(&players, 7);
        let mut start = game.start_payload();
        let mut snapshot = game.snapshot_for(1);
        start.map.width = 16;
        start.map.height = 12;
        start.map.tile_size = 32;
        start.map.terrain = vec![0; 16 * 12];
        start.map.resources.clear();
        start.map.unwalkable_tiles.clear();
        start.map.no_vehicle_tiles.clear();
        start.map.doodads.clear();
        snapshot.entities.clear();
        snapshot.resource_deltas.clear();
        (start, snapshot)
    }
    fn frame(
        start: &rts_sim::protocol::StartPayload,
        snapshot: &rts_sim::protocol::Snapshot,
    ) -> super::super::AiFrame {
        super::super::AiFrame::from_host(start, snapshot, 1, [], Some(&[1, 2])).unwrap()
    }
    #[test]
    fn route_follows_wall_gap_and_never_cuts_a_blocked_corner() {
        let (mut start, s) = fixture();
        for y in 0..11 {
            start.map.unwalkable_tiles.push(MapTile { x: 7, y });
        }
        let f = frame(&start, &s);
        let q = WorldQueries::new(&f);
        let a = q.world_point(80., 80.).unwrap();
        let b = q.world_point(432., 80.).unwrap();
        let path = q.ground_route(a, b, EntityKind::Worker).unwrap();
        assert_eq!(path.first(), Some(&a));
        assert_eq!(path.last(), Some(&b));
        assert!(path.iter().any(|p| p.y() >= 11. * 32.));
        assert_eq!(
            path,
            q.ground_route(a, b, EntityKind::Worker).unwrap(),
            "stable equal-cost ties"
        );
        start.map.unwalkable_tiles.push(MapTile { x: 7, y: 11 });
        let f = frame(&start, &s);
        let q = WorldQueries::new(&f);
        assert!(q.ground_route(a, b, EntityKind::Worker).is_none());
        let (mut start, s) = fixture();
        start.map.unwalkable_tiles = vec![MapTile { x: 1, y: 0 }, MapTile { x: 0, y: 1 }];
        let f = frame(&start, &s);
        let q = WorldQueries::new(&f);
        assert!(q
            .ground_route(
                q.world_point(16., 16.).unwrap(),
                q.world_point(48., 48.).unwrap(),
                EntityKind::Worker
            )
            .is_none());
    }
    #[test]
    fn route_avoids_blocking_buildings_but_passes_resources_and_unit_bodies() {
        let (mut start, mut s) = fixture();
        start.map.resources.push(ResourceNode {
            id: 900,
            kind: "steel".into(),
            x: 176.,
            y: 176.,
        });
        let game = Game::new_without_ai_controllers(
            &[PlayerInit {
                id: 1,
                team_id: 1,
                faction_id: "kriegsia".into(),
                name: "P1".into(),
                color: "#123456".into(),
                is_ai: true,
            }],
            7,
        );
        let original = game.snapshot_for(1);
        let mut building = original
            .entities
            .iter()
            .find(|e| e.kind == "resource_depot")
            .unwrap()
            .clone();
        building.x = 320.;
        building.y = 176.;
        s.entities.push(building);
        let mut unit = original
            .entities
            .iter()
            .find(|e| e.kind == "worker")
            .unwrap()
            .clone();
        unit.x = 80.;
        unit.y = 176.;
        s.entities.push(unit);
        let f = frame(&start, &s);
        let q = WorldQueries::new(&f);
        let a = q.world_point(48., 176.).unwrap();
        let b = q.world_point(464., 176.).unwrap();
        let path = q.ground_route(a, b, EntityKind::Worker).unwrap();
        assert!(!path.is_empty());
        assert!(q
            .ground_route(a, q.world_point(176., 176.).unwrap(), EntityKind::Worker)
            .is_some());
        assert!(q
            .ground_route(a, q.world_point(320., 176.).unwrap(), EntityKind::Worker)
            .is_none());
        assert!(q
            .ground_route(a, q.world_point(80., 176.).unwrap(), EntityKind::Worker)
            .is_some());
    }
    #[test]
    fn build_route_stops_outside_footprint_and_rejects_unreachable_perimeters() {
        let (mut start, s) = fixture();
        let f = frame(&start, &s);
        let q = WorldQueries::new(&f);
        let tile = q.tile(8, 5).unwrap();
        let path = q
            .ground_route_to_build_site(
                q.world_point(48., 176.).unwrap(),
                EntityKind::ResourceDepot,
                tile,
                EntityKind::Worker,
            )
            .unwrap();
        let escape = q.ground_route_to_build_site(
            q.world_point(304., 208.).unwrap(),
            EntityKind::ResourceDepot,
            tile,
            EntityKind::Worker,
        );
        assert!(
            escape.is_some(),
            "an Engineer inside an unbuilt footprint can reach its exterior"
        );
        let stats = rts_rules::balance::building_stats(EntityKind::ResourceDepot).unwrap();
        let rect = (
            256.,
            160.,
            (8 + stats.foot_w) as f32 * 32.,
            (5 + stats.foot_h) as f32 * 32.,
        );
        assert!(path.iter().all(|p| !super::super::unit_circle_touches_rect(
            p.as_tuple(),
            rts_rules::balance::unit_placement_radius(EntityKind::Worker),
            rect
        )));
        let tank_path = q
            .ground_route_to_build_site(
                q.world_point(48., 176.).unwrap(),
                EntityKind::ResourceDepot,
                tile,
                EntityKind::Tank,
            )
            .expect("wide movers need a reachable exterior ring");
        assert!(tank_path
            .iter()
            .all(|p| !super::super::unit_circle_touches_rect(
                p.as_tuple(),
                rts_rules::balance::unit_placement_radius(EntityKind::Tank),
                rect
            )));
        for y in 0..12 {
            start.map.unwalkable_tiles.push(MapTile { x: 6, y });
        }
        let f = frame(&start, &s);
        let q = WorldQueries::new(&f);
        assert!(q
            .ground_route_to_build_site(
                q.world_point(48., 176.).unwrap(),
                EntityKind::ResourceDepot,
                tile,
                EntityKind::Worker
            )
            .is_none());
    }
    #[test]
    fn visible_threat_corridor_checks_segment_middle_and_ignores_offroute_dead_and_noncombat() {
        let (start, mut s) = fixture();
        let game = Game::new_without_ai_controllers(
            &[PlayerInit {
                id: 1,
                team_id: 1,
                faction_id: "kriegsia".into(),
                name: "P1".into(),
                color: "#123456".into(),
                is_ai: true,
            }],
            7,
        );
        let original = game.snapshot_for(1);
        let mut enemy = original
            .entities
            .iter()
            .find(|e| e.kind == "rifleman")
            .unwrap()
            .clone();
        enemy.id = 9876;
        enemy.owner = 2;
        enemy.x = 256.;
        enemy.y = 48.;
        s.entities.push(enemy);
        let threat = |snapshot: &rts_sim::protocol::Snapshot| {
            let f = frame(&start, snapshot);
            let q = WorldQueries::new(&f);
            // Sparse endpoints: the enemy threatens the middle, not either endpoint.
            let route = [
                q.world_point(16., 176.).unwrap(),
                q.world_point(496., 176.).unwrap(),
            ];
            q.route_has_visible_ground_threat(&route, EntityKind::Worker)
        };
        assert!(threat(&s));
        s.entities[0].y = 368.;
        assert!(!threat(&s), "off-route fight must not hold the Engineer");
        s.entities[0].y = 176.;
        s.entities[0].hp = 0;
        assert!(!threat(&s), "dead visible unit cannot threaten the route");
        s.entities[0].hp = 40;
        s.entities[0].kind = "resource_depot".into();
        assert!(!threat(&s), "noncombat building cannot threaten the route");
        s.entities[0].kind = "anti_tank_gun".into();
        assert!(!threat(&s), "default weapon cannot target this traveller");
        s.entities.clear();
        s.remembered_buildings
            .push(rts_sim::protocol::RememberedBuildingView {
                id: 9876,
                owner: 2,
                kind: "rifleman".into(),
                x: 256.,
                y: 176.,
                footprint: vec![],
                observed_tick: 0,
            });
        assert!(
            !threat(&s),
            "remembered locations are not current visible threats"
        );
    }
    #[test]
    fn wide_route_and_reusable_field_fit_a_two_tile_lane() {
        let (mut start, s) = fixture();
        for x in 0..16 {
            start.map.unwalkable_tiles.push(MapTile { x, y: 4 });
            start.map.unwalkable_tiles.push(MapTile { x, y: 7 });
        }
        let f = frame(&start, &s);
        let q = WorldQueries::new(&f);
        let from = q.world_point(112., 192.).unwrap();
        let to = q.world_point(400., 192.).unwrap();
        let path = q
            .ground_route(from, to, EntityKind::Tank)
            .expect("64px lane fits rotating Tank clearance");
        assert!(path.iter().all(|p| p.y() == 192.));
        let component = q.ground_reachability(from, EntityKind::Tank).unwrap();
        assert!(component.contains(to));
        assert!(!component.contains(q.world_point(112., 80.).unwrap()));
        for x in 0..16 {
            start.map.unwalkable_tiles.push(MapTile { x, y: 6 });
        }
        let f = frame(&start, &s);
        let q = WorldQueries::new(&f);
        assert!(
            q.ground_route(
                q.world_point(112., 176.).unwrap(),
                q.world_point(400., 176.).unwrap(),
                EntityKind::Tank
            )
            .is_none(),
            "32px lane cannot fit conservative rotation body"
        );
    }
    fn layout_fixture() -> (
        rts_sim::protocol::StartPayload,
        rts_sim::protocol::Snapshot,
        rts_sim::protocol::EntityView,
    ) {
        let (mut start, s) = fixture();
        start.map.width = 32;
        start.map.height = 32;
        start.map.terrain = vec![0; 32 * 32];
        start.map.no_building_tiles.clear();
        let players = [1, 2].map(|id| PlayerInit {
            id,
            team_id: id,
            faction_id: "kriegsia".into(),
            name: format!("P{id}"),
            color: "#123456".into(),
            is_ai: true,
        });
        let specimen = Game::new_without_ai_controllers(&players, 7)
            .snapshot_for(1)
            .entities
            .into_iter()
            .find(|e| e.kind == "resource_depot")
            .unwrap();
        (start, s, specimen)
    }
    fn add_building(
        s: &mut rts_sim::protocol::Snapshot,
        specimen: &rts_sim::protocol::EntityView,
        kind: EntityKind,
        x: u32,
        y: u32,
    ) {
        let mut e = specimen.clone();
        let stats = rts_rules::balance::building_stats(kind).unwrap();
        e.id = 9000 + s.entities.len() as u32;
        e.kind = kind.stable_id().into();
        e.x = (x as f32 + stats.foot_w as f32 / 2.) * 32.;
        e.y = (y as f32 + stats.foot_h as f32 / 2.) * 32.;
        s.entities.push(e);
    }
    #[test]
    fn layout_uses_two_tile_gaps_and_three_for_vehicle_works_in_both_build_orders() {
        use super::super::{KnownBuildSite, KnownBuildSiteBlocker};
        let (start, mut s, specimen) = layout_fixture();
        add_building(&mut s, &specimen, EntityKind::TrainingCentre, 10, 10);
        let f = frame(&start, &s);
        let q = WorldQueries::new(&f);
        let exclusions = Default::default();
        assert_eq!(
            q.known_build_site_with_access(
                EntityKind::EngineeringComplex,
                q.tile(14, 10).unwrap(),
                &exclusions
            ),
            KnownBuildSite::KnownBlocked(KnownBuildSiteBlocker::CurrentBuilding)
        );
        assert_eq!(
            q.known_build_site_with_access(
                EntityKind::EngineeringComplex,
                q.tile(15, 10).unwrap(),
                &exclusions
            ),
            KnownBuildSite::NoKnownConflict
        );
        assert_eq!(
            q.known_build_site_with_access(
                EntityKind::Factory,
                q.tile(15, 10).unwrap(),
                &exclusions
            ),
            KnownBuildSite::KnownBlocked(KnownBuildSiteBlocker::CurrentBuilding)
        );
        assert_eq!(
            q.known_build_site_with_access(
                EntityKind::Factory,
                q.tile(16, 10).unwrap(),
                &exclusions
            ),
            KnownBuildSite::NoKnownConflict
        );
        s.entities.clear();
        add_building(&mut s, &specimen, EntityKind::Factory, 10, 10);
        let f = frame(&start, &s);
        let q = WorldQueries::new(&f);
        assert_eq!(
            q.known_build_site_with_access(
                EntityKind::EngineeringComplex,
                q.tile(15, 10).unwrap(),
                &exclusions
            ),
            KnownBuildSite::KnownBlocked(KnownBuildSiteBlocker::CurrentBuilding)
        );
        assert_eq!(
            q.known_build_site_with_access(
                EntityKind::EngineeringComplex,
                q.tile(16, 10).unwrap(),
                &exclusions
            ),
            KnownBuildSite::NoKnownConflict
        );
    }
    #[test]
    fn layout_counts_solid_terrain_and_edges_but_accepts_an_alternate_production_exit() {
        use super::super::{KnownBuildSite, KnownBuildSiteBlocker};
        let (mut start, s, _) = layout_fixture();
        let f = frame(&start, &s);
        let q = WorldQueries::new(&f);
        let exclusions = Default::default();
        assert_eq!(
            q.known_build_site_with_access(
                EntityKind::Factory,
                q.tile(0, 10).unwrap(),
                &exclusions
            ),
            KnownBuildSite::KnownBlocked(KnownBuildSiteBlocker::KnownTerrain)
        );
        start.map.unwalkable_tiles.push(MapTile { x: 3, y: 7 });
        let f = frame(&start, &s);
        let q = WorldQueries::new(&f);
        assert_eq!(
            q.known_build_site_with_access(
                EntityKind::EngineeringComplex,
                q.tile(5, 5).unwrap(),
                &exclusions
            ),
            KnownBuildSite::KnownBlocked(KnownBuildSiteBlocker::KnownTerrain)
        );
        start.map.unwalkable_tiles.clear();
        // A public resource blocks the old bottom-center heuristic. Other sides have valid exits.
        start.map.resources.push(ResourceNode {
            id: 99999,
            kind: "steel".into(),
            x: 11. * 32. + 16.,
            y: 13. * 32. + 16.,
        });
        let f = frame(&start, &s);
        let q = WorldQueries::new(&f);
        assert_eq!(
            q.known_build_site_with_access(
                EntityKind::Factory,
                q.tile(10, 10).unwrap(),
                &exclusions
            ),
            KnownBuildSite::NoKnownConflict
        );
        assert_eq!(
            q.known_build_site(EntityKind::Factory, q.tile(10, 10).unwrap(), &exclusions),
            KnownBuildSite::KnownBlocked(KnownBuildSiteBlocker::ProductionExit)
        );
    }
    #[test]
    fn later_building_cannot_seal_a_previously_usable_vehicle_escape() {
        let (mut start, mut s, specimen) = layout_fixture();
        for x in 1..=10 {
            for y in [2, 11] {
                start.map.unwalkable_tiles.push(MapTile { x, y });
            }
        }
        for y in 2..=11 {
            start.map.unwalkable_tiles.push(MapTile { x: 1, y });
            if !(5..=7).contains(&y) {
                start.map.unwalkable_tiles.push(MapTile { x: 10, y });
            }
        }
        add_building(&mut s, &specimen, EntityKind::Factory, 3, 4);
        let f = frame(&start, &s);
        let q = WorldQueries::new(&f);
        assert!(q.placement_preserves_producer_access(
            EntityKind::EngineeringComplex,
            q.tile(20, 20).unwrap()
        ));
        assert!(
            !q.placement_preserves_producer_access(
                EntityKind::EngineeringComplex,
                q.tile(9, 5).unwrap()
            ),
            "later footprint closes Works' only wide-body path"
        );
    }
    #[test]
    fn rotated_resource_and_future_extractor_remain_walk_through() {
        let (mut start, mut s, specimen) = layout_fixture();
        start.map.resources.push(ResourceNode {
            id: 99999,
            kind: "steel".into(),
            x: 317.,
            y: 310.,
        });
        let before = frame(&start, &s);
        let before_q = WorldQueries::new(&before);
        let before_grid = RouteGrid::new(&before_q, EntityKind::Tank).unwrap();
        let mut mine = specimen;
        mine.id = 9999;
        mine.kind = "steel_mine".into();
        mine.x = 317.;
        mine.y = 310.;
        s.entities.push(mine);
        let after = frame(&start, &s);
        let after_q = WorldQueries::new(&after);
        let after_grid = RouteGrid::new(&after_q, EntityKind::Tank).unwrap();
        assert_eq!(
            before_grid.walk, after_grid.walk,
            "native resource nodes and1x1 extractors remain walk-through"
        );
    }
    #[test]
    fn vehicle_only_public_overlays_and_tree_trunks_do_not_block_infantry() {
        let (mut start, s) = fixture();
        for y in 0..12 {
            start.map.no_vehicle_tiles.push(MapTile { x: 8, y });
        }
        let f = frame(&start, &s);
        let q = WorldQueries::new(&f);
        let a = q.world_point(112., 176.).unwrap();
        let b = q.world_point(400., 176.).unwrap();
        assert!(q.ground_route(a, b, EntityKind::Tank).is_none());
        assert!(q.ground_route(a, b, EntityKind::Worker).is_some());
        start.map.no_vehicle_tiles.clear();
        start.map.doodads.push(rts_sim::protocol::MapDoodad {
            id: 1,
            type_id: "tree.oak".into(),
            x: 256,
            y: 176,
            color: None,
        });
        let f = frame(&start, &s);
        let q = WorldQueries::new(&f);
        let tank = q.ground_route(a, b, EntityKind::Tank).unwrap();
        let radius = rts_rules::balance::unit_placement_radius(EntityKind::Tank)
            + rts_rules::balance::TREE_TRUNK_RADIUS_PX;
        assert!(tank.windows(2).all(|p| segment_distance2(
            (256., 176.),
            p[0].as_tuple(),
            p[1].as_tuple()
        ) > radius * radius));
        assert!(q
            .ground_route(a, b, EntityKind::Worker)
            .unwrap()
            .iter()
            .all(|p| p.y() == 176.));
        let cell = q.world_point(256., 176.).unwrap();
        assert!(q
            .ground_reachability(a, EntityKind::Tank)
            .unwrap()
            .contains(b));
        assert!(!q
            .ground_reachability(a, EntityKind::Tank)
            .unwrap()
            .contains(cell));
    }
    #[test]
    fn tree_sweep_rejects_an_edge_even_when_both_endpoints_fit() {
        let (mut start, s) = fixture();
        start.map.doodads.push(rts_sim::protocol::MapDoodad {
            id: 1,
            type_id: "tree.oak".into(),
            x: 184,
            y: 211,
            color: None,
        });
        let f = frame(&start, &s);
        let q = WorldQueries::new(&f);
        let grid = RouteGrid::new(&q, EntityKind::Tank).unwrap();
        let a = grid.world_index((176., 176.)).unwrap();
        let b = grid.world_index((192., 176.)).unwrap();
        assert!(grid.walk[a] && grid.walk[b]);
        assert!(
            !grid.neighbors(a).any(|i| i == b),
            "continuous swept body cannot clip the trunk between valid samples"
        );
    }
    #[test]
    fn classic_frame_retains_authored_vehicle_masks_and_trunk_centers() {
        let players = [1, 2].map(|id| PlayerInit {
            id,
            team_id: id,
            faction_id: "kriegsia".into(),
            name: format!("P{id}"),
            color: "#123456".into(),
            is_ai: true,
        });
        let map =
            rts_sim::game::map::Map::load_for_players("Classic", &[(1, 1), (2, 2)], 7).unwrap();
        let game = Game::new_with_random_ai_profiles_and_map(&players, 7, map);
        let start = game.start_payload();
        let snapshot = game.snapshot_for(1);
        let f = frame(&start, &snapshot);
        assert_eq!(
            f.map().no_vehicle_tiles.len(),
            start.map.no_vehicle_tiles.len()
        );
        assert_eq!(
            f.map().tree_trunks.len(),
            start
                .map
                .doodads
                .iter()
                .filter(|d| d.type_id.starts_with("tree."))
                .count()
        );
        assert!(!f.map().no_vehicle_tiles.is_empty());
        assert!(!f.map().tree_trunks.is_empty());
        let q = WorldQueries::new(&f);
        let tile = start.map.no_vehicle_tiles.first().unwrap();
        let point = q
            .world_point((tile.x as f32 + 0.5) * 32., (tile.y as f32 + 0.5) * 32.)
            .unwrap();
        assert!(q.ground_reachability(point, EntityKind::Tank).is_none());
    }
}
