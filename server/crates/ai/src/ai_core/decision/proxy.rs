use super::geometry::{
    building_center, dist2, footprint_edge_distance_tiles, point_line_distance2, squared,
};
use super::*;

pub(super) const PROXY_DISTANCE_BAND_TILES: f32 = 2.0;

pub(super) const PROXY_WORKER_BUILD_SEARCH_RADIUS_TILES: i32 = 4;

#[allow(clippy::too_many_arguments)]
pub(super) fn try_proxy_barracks<F>(
    observation: &AiObservation,
    facts: &AiFacts,
    actions: &mut AiActionContext<'_>,
    memory: &mut AiDecisionMemory,
    profile: &AiProfile,
    placeable: &mut F,
) -> Option<AiIntent>
where
    F: FnMut(EntityKind, u32, u32) -> bool,
{
    let policy = profile.buildings.proxy_barracks?;
    let kind = EntityKind::Barracks;
    if !rts_rules::economy::build_requirement_met(kind, facts.complete_building_kinds()) {
        return None;
    }
    let counts = facts.building_counts(kind);
    if counts.total_planned() > 0 {
        return None;
    }
    let enemy_base = facts.nearest_public_enemy_base?;
    let proxy_worker_was_committed = memory.proxy_worker_id.is_some();
    let worker = select_proxy_worker(observation, facts, memory)?;
    let worker_pool = [worker];
    let worker_entity = observation
        .owned
        .iter()
        .find(|entity| entity.id == worker && entity.kind == EntityKind::Worker)?;

    if proxy_worker_was_committed {
        if let Some((tile_x, tile_y)) =
            proxy_barracks_site_near_worker(observation, worker_entity, kind, placeable)
        {
            if actions::try_build_at(actions, &[&worker_pool], kind, tile_x, tile_y).is_some() {
                memory.proxy_worker_id = Some(worker);
                return Some(AiIntent::Build { kind });
            }
        }
    }

    let Some(transit_site) =
        proxy_barracks_transit_site(observation, enemy_base, kind, policy, placeable)
    else {
        if !proxy_worker_was_committed {
            memory.proxy_worker_id = None;
        }
        return None;
    };

    if actions::try_build_at(
        actions,
        &[&worker_pool],
        kind,
        transit_site.0,
        transit_site.1,
    )
    .is_some()
    {
        memory.proxy_worker_id = Some(worker);
        return Some(AiIntent::Build { kind });
    }

    if !actions.reserve_worker(worker) {
        return None;
    }
    let (x, y) = building_center(transit_site, kind, observation.map.tile_size)?;
    actions.emit_command(Command::Move {
        units: vec![worker],
        x,
        y,
        queued: false,
    });
    Some(AiIntent::Move {
        units: vec![worker],
    })
}

pub(super) fn should_use_proxy_barracks(facts: &AiFacts, profile: &AiProfile) -> bool {
    profile.buildings.proxy_barracks.is_some() && facts.building_count(EntityKind::Barracks) == 0
}

pub(super) fn proxy_barracks_transit_site<F>(
    observation: &AiObservation,
    enemy_base: EnemyBaseFact,
    kind: EntityKind,
    policy: ProxyBarracksPolicy,
    placeable: &mut F,
) -> Option<(u32, u32)>
where
    F: FnMut(EntityKind, u32, u32) -> bool,
{
    let stats = config::building_stats(kind)?;
    if stats.foot_w > observation.map.width || stats.foot_h > observation.map.height {
        return None;
    }
    let search_radius_tiles = policy
        .search_radius_tiles
        .max(policy.min_enemy_base_distance_tiles)
        .max(0);
    let target_distance = policy.min_enemy_base_distance_tiles.max(0) as f32;
    let min_distance2 = squared(target_distance);
    let enemy_center = (
        enemy_base.start_tile.0 as f32 + 0.5,
        enemy_base.start_tile.1 as f32 + 0.5,
    );
    let own_center = (
        observation.own_start_tile.0 as f32 + 0.5,
        observation.own_start_tile.1 as f32 + 0.5,
    );
    let (sx, sy) = (
        enemy_base.start_tile.0 as i32,
        enemy_base.start_tile.1 as i32,
    );

    let mut best = None;
    for dy in -search_radius_tiles..=search_radius_tiles {
        for dx in -search_radius_tiles..=search_radius_tiles {
            if dx.abs().max(dy.abs()) > search_radius_tiles {
                continue;
            }
            let tx = sx + dx;
            let ty = sy + dy;
            if tx < 0 || ty < 0 {
                continue;
            }
            let (tx, ty) = (tx as u32, ty as u32);
            if tx > observation.map.width - stats.foot_w
                || ty > observation.map.height - stats.foot_h
            {
                continue;
            }

            let center_x = tx as f32 + stats.foot_w as f32 * 0.5;
            let center_y = ty as f32 + stats.foot_h as f32 * 0.5;
            let dx = center_x - enemy_center.0;
            let dy = center_y - enemy_center.1;
            let distance2 = dx * dx + dy * dy;
            if distance2 < min_distance2 || !placeable(kind, tx, ty) {
                continue;
            }
            let distance = distance2.sqrt();
            let distance_over_target = (distance - target_distance).max(0.0);
            let distance_band = (distance_over_target / PROXY_DISTANCE_BAND_TILES).floor() as i32;
            let candidate = ProxySiteCandidate {
                tile: (tx, ty),
                distance_band,
                distance_over_target,
                edge_distance_tiles: footprint_edge_distance_tiles(
                    (tx, ty),
                    &stats,
                    observation.map.width,
                    observation.map.height,
                ),
                scout_path_distance2: point_line_distance2(
                    (center_x, center_y),
                    own_center,
                    enemy_center,
                ),
            };
            if proxy_site_candidate_better(candidate, best) {
                best = Some(candidate);
            }
        }
    }

    best.map(|candidate| candidate.tile)
}

pub(super) fn proxy_barracks_site_near_worker<F>(
    observation: &AiObservation,
    worker: &AiEntitySummary,
    kind: EntityKind,
    placeable: &mut F,
) -> Option<(u32, u32)>
where
    F: FnMut(EntityKind, u32, u32) -> bool,
{
    let stats = config::building_stats(kind)?;
    if stats.foot_w > observation.map.width || stats.foot_h > observation.map.height {
        return None;
    }
    let tile_size = observation.map.tile_size as f32;
    if tile_size <= 0.0 {
        return None;
    }
    let worker_tile = (worker.x / tile_size, worker.y / tile_size);
    let sx = worker_tile.0.floor() as i32;
    let sy = worker_tile.1.floor() as i32;
    let mut best = None;

    for dy in -PROXY_WORKER_BUILD_SEARCH_RADIUS_TILES..=PROXY_WORKER_BUILD_SEARCH_RADIUS_TILES {
        for dx in -PROXY_WORKER_BUILD_SEARCH_RADIUS_TILES..=PROXY_WORKER_BUILD_SEARCH_RADIUS_TILES {
            if dx.abs().max(dy.abs()) > PROXY_WORKER_BUILD_SEARCH_RADIUS_TILES {
                continue;
            }
            let tx = sx + dx;
            let ty = sy + dy;
            if tx < 0 || ty < 0 {
                continue;
            }
            let (tx, ty) = (tx as u32, ty as u32);
            if tx > observation.map.width - stats.foot_w
                || ty > observation.map.height - stats.foot_h
                || !placeable(kind, tx, ty)
            {
                continue;
            }
            let center_x = tx as f32 + stats.foot_w as f32 * 0.5;
            let center_y = ty as f32 + stats.foot_h as f32 * 0.5;
            let candidate = WorkerBuildSiteCandidate {
                tile: (tx, ty),
                worker_distance2: dist2(center_x, center_y, worker_tile.0, worker_tile.1),
            };
            if worker_build_site_candidate_better(candidate, best) {
                best = Some(candidate);
            }
        }
    }

    best.map(|candidate| candidate.tile)
}

#[derive(Clone, Copy, Debug)]
pub(super) struct ProxySiteCandidate {
    tile: (u32, u32),
    distance_band: i32,
    distance_over_target: f32,
    edge_distance_tiles: u32,
    scout_path_distance2: f32,
}

pub(super) fn proxy_site_candidate_better(
    candidate: ProxySiteCandidate,
    current: Option<ProxySiteCandidate>,
) -> bool {
    let Some(current) = current else {
        return true;
    };
    if candidate.distance_band != current.distance_band {
        return candidate.distance_band < current.distance_band;
    }
    if candidate.edge_distance_tiles != current.edge_distance_tiles {
        return candidate.edge_distance_tiles < current.edge_distance_tiles;
    }
    match candidate
        .scout_path_distance2
        .total_cmp(&current.scout_path_distance2)
    {
        Ordering::Greater => return true,
        Ordering::Less => return false,
        Ordering::Equal => {}
    }
    match candidate
        .distance_over_target
        .total_cmp(&current.distance_over_target)
    {
        Ordering::Less => return true,
        Ordering::Greater => return false,
        Ordering::Equal => {}
    }
    candidate.tile < current.tile
}

#[derive(Clone, Copy, Debug)]
pub(super) struct WorkerBuildSiteCandidate {
    tile: (u32, u32),
    worker_distance2: f32,
}

pub(super) fn worker_build_site_candidate_better(
    candidate: WorkerBuildSiteCandidate,
    current: Option<WorkerBuildSiteCandidate>,
) -> bool {
    let Some(current) = current else {
        return true;
    };
    match candidate
        .worker_distance2
        .total_cmp(&current.worker_distance2)
    {
        Ordering::Less => return true,
        Ordering::Greater => return false,
        Ordering::Equal => {}
    }
    candidate.tile < current.tile
}

pub(super) fn select_proxy_worker(
    observation: &AiObservation,
    facts: &AiFacts,
    memory: &mut AiDecisionMemory,
) -> Option<u32> {
    let workers_by_id: BTreeMap<u32, &AiEntitySummary> = observation
        .owned
        .iter()
        .filter(|entity| entity.kind == EntityKind::Worker)
        .map(|entity| (entity.id, entity))
        .collect();
    if let Some(worker_id) = memory.proxy_worker_id {
        let worker = workers_by_id.get(&worker_id).copied()?;
        return (worker.state != AiEntityState::Build).then_some(worker.id);
    }

    let mut candidates = facts.idle_workers.clone();
    candidates.extend(facts.gathering_workers.iter().copied());
    candidates.extend(facts.build_capable_workers.iter().copied());
    candidates.sort_unstable();
    candidates.dedup();

    for worker_id in candidates {
        let Some(worker) = workers_by_id.get(&worker_id).copied() else {
            continue;
        };
        if worker.state == AiEntityState::Build {
            continue;
        }
        memory.proxy_worker_id = Some(worker.id);
        return Some(worker.id);
    }
    memory.proxy_worker_id = None;
    None
}
