use super::geometry::{
    building_center, dist2, footprint_top_left_for_center, squared, tile_center,
};
use super::policies::active_expansion_policy;
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum ExpansionBlocker {
    NotDue,
    DefensivePanic,
    MissingRequiredBuilding,
    MissingDefensiveUnits,
    RequirementNotMet,
    AlreadyAtTarget,
    MaxPending,
    NoCandidateResources,
    NoValidSite,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ExpansionPlan {
    pub(super) policy: Option<ExpansionPolicy>,
    pub(super) should_save: bool,
    pub(super) blocks_tech_path: bool,
    pub(super) blockers: Vec<ExpansionBlocker>,
}

pub(super) fn plan_expansion(
    observation: &AiObservation,
    facts: &AiFacts,
    profile: &AiProfile,
    recovery_active: bool,
    defensive_panic_active: bool,
) -> ExpansionPlan {
    if defensive_panic_active {
        return ExpansionPlan {
            policy: None,
            should_save: false,
            blocks_tech_path: false,
            blockers: vec![ExpansionBlocker::DefensivePanic],
        };
    }
    let Some(expansion) = active_expansion(observation, profile, recovery_active) else {
        return ExpansionPlan {
            policy: None,
            should_save: false,
            blocks_tech_path: false,
            blockers: vec![ExpansionBlocker::NotDue],
        };
    };

    let building_count = facts.building_count(EntityKind::CityCentre);
    let blocks_tech_path =
        expansion.blocks_tech_path && building_count < expansion.target_city_centres;
    let mut blockers = Vec::new();
    if building_count >= expansion.target_city_centres {
        blockers.push(ExpansionBlocker::AlreadyAtTarget);
    }
    let counts = facts.building_counts(EntityKind::CityCentre);
    if counts.incomplete + counts.intended >= profile.buildings.max_pending_per_kind {
        blockers.push(ExpansionBlocker::MaxPending);
    }
    if facts.complete_building_count(expansion.required_complete_building) == 0 {
        blockers.push(ExpansionBlocker::MissingRequiredBuilding);
    }
    if facts.unit_count(expansion.defensive_unit) < expansion.defensive_unit_count {
        blockers.push(ExpansionBlocker::MissingDefensiveUnits);
    }
    if !rts_rules::economy::build_requirement_met(
        EntityKind::CityCentre,
        facts.complete_building_kinds(),
    ) {
        blockers.push(ExpansionBlocker::RequirementNotMet);
    }
    if expansion_candidate_resources(observation).is_empty() {
        blockers.push(ExpansionBlocker::NoCandidateResources);
    }
    blockers.sort();
    blockers.dedup();
    let should_save = blockers.is_empty();

    ExpansionPlan {
        policy: Some(expansion),
        should_save,
        blocks_tech_path,
        blockers,
    }
}

pub(super) fn active_expansion(
    observation: &AiObservation,
    profile: &AiProfile,
    recovery_active: bool,
) -> Option<ExpansionPolicy> {
    let expansion = active_expansion_policy(profile, recovery_active)?;
    if observation.economy.steel >= expansion.trigger_steel
        || observation.economy.supply_used >= expansion.trigger_supply_used
    {
        Some(expansion)
    } else {
        None
    }
}

pub(super) fn expansion_blocks_tech_path(
    observation: &AiObservation,
    facts: &AiFacts,
    profile: &AiProfile,
    recovery_active: bool,
) -> bool {
    let Some(expansion) = active_expansion(observation, profile, recovery_active) else {
        return false;
    };
    expansion.blocks_tech_path
        && facts.building_count(EntityKind::CityCentre) < expansion.target_city_centres
}

pub(super) fn should_save_for_expansion(
    observation: &AiObservation,
    facts: &AiFacts,
    profile: &AiProfile,
    recovery_active: bool,
) -> bool {
    let Some(expansion) = active_expansion(observation, profile, recovery_active) else {
        return false;
    };
    facts.building_count(EntityKind::CityCentre) < expansion.target_city_centres
        && expansion_prerequisites_met(facts, expansion)
}

pub(super) fn expansion_prerequisites_met(facts: &AiFacts, expansion: ExpansionPolicy) -> bool {
    facts.complete_building_count(expansion.required_complete_building) > 0
        && facts.unit_count(expansion.defensive_unit) >= expansion.defensive_unit_count
}

#[allow(clippy::too_many_arguments)]
pub(super) fn try_build_expansion_city_centre<F>(
    observation: &AiObservation,
    facts: &AiFacts,
    actions: &mut AiActionContext<'_>,
    builder_pools: &[&[u32]],
    profile: &AiProfile,
    recovery_active: bool,
    placeable: &mut F,
) -> Option<actions::BuildAction>
where
    F: FnMut(EntityKind, u32, u32) -> bool,
{
    let expansion = active_expansion(observation, profile, recovery_active)?;
    let kind = EntityKind::CityCentre;
    config::building_stats(kind)?;
    if !rts_rules::economy::build_requirement_met(kind, facts.complete_building_kinds()) {
        return None;
    }
    if facts.building_count(kind) >= expansion.target_city_centres {
        return None;
    }
    let counts = facts.building_counts(kind);
    if counts.incomplete + counts.intended >= profile.buildings.max_pending_per_kind {
        return None;
    }
    let (tile_x, tile_y) = expansion_city_centre_site(observation, expansion, kind, placeable)?;
    actions::try_build_at(actions, builder_pools, kind, tile_x, tile_y)
}

pub(super) fn expansion_city_centre_site<F>(
    observation: &AiObservation,
    expansion: ExpansionPolicy,
    kind: EntityKind,
    placeable: &mut F,
) -> Option<(u32, u32)>
where
    F: FnMut(EntityKind, u32, u32) -> bool,
{
    let stats = config::building_stats(kind)?;
    let resources = expansion_candidate_resources(observation);
    if resources.is_empty() {
        return None;
    }
    let mut best = None;
    for anchor in expansion_anchor_tiles(observation, &resources) {
        let cluster_resources =
            expansion_cluster_resources_for_anchor(observation, anchor, &resources);
        if cluster_resources.is_empty() {
            continue;
        }
        let required_steel = cluster_resources
            .iter()
            .filter(|resource| resource.kind == EntityKind::Steel)
            .count()
            .min(config::STEEL_PATCHES_PER_BASE as usize);
        let required_oil = cluster_resources
            .iter()
            .filter(|resource| resource.kind == EntityKind::Oil)
            .count()
            .min(config::OIL_PATCHES_PER_BASE as usize);
        let mut seen = BTreeSet::new();
        let Some(start_tile) = footprint_top_left_for_center(anchor, kind) else {
            continue;
        };
        let (sx, sy) = (start_tile.0 as i32, start_tile.1 as i32);
        for dy in -expansion.search_radius_tiles..=expansion.search_radius_tiles {
            for dx in -expansion.search_radius_tiles..=expansion.search_radius_tiles {
                if dx.abs().max(dy.abs()) > expansion.search_radius_tiles {
                    continue;
                }
                let tx = sx + dx;
                let ty = sy + dy;
                if tx < 0 || ty < 0 {
                    continue;
                }
                let (tx, ty) = (tx as u32, ty as u32);
                if tx > observation.map.width.saturating_sub(stats.foot_w)
                    || ty > observation.map.height.saturating_sub(stats.foot_h)
                    || !seen.insert((tx, ty))
                {
                    continue;
                }
                let Some(candidate) =
                    expansion_site_candidate(observation, kind, tx, ty, &cluster_resources)
                else {
                    continue;
                };
                if candidate.steel_in_range < required_steel
                    || candidate.oil_in_range < required_oil
                {
                    continue;
                }
                if !placeable(kind, tx, ty) {
                    continue;
                }
                if expansion_site_candidate_better(candidate, best) {
                    best = Some(candidate);
                }
            }
        }
    }

    best.map(|candidate: ExpansionSiteCandidate| candidate.tile)
}

pub(super) fn expansion_candidate_resources(
    observation: &AiObservation,
) -> Vec<&AiResourceSummary> {
    let start_resource_radius =
        (config::CC_RESOURCE_MAX_DIST_TILES + 1.5) * observation.map.tile_size as f32;
    let start_resource_radius2 = squared(start_resource_radius);
    observation
        .resources
        .iter()
        .filter(|resource| matches!(resource.kind, EntityKind::Steel | EntityKind::Oil))
        .filter(|resource| resource.remaining > 0)
        .filter(|resource| {
            !resource_is_near_player_start(observation, resource, start_resource_radius2)
        })
        .collect()
}

pub(super) fn expansion_anchor_tiles(
    observation: &AiObservation,
    resources: &[&AiResourceSummary],
) -> Vec<(u32, u32)> {
    let tile_size = observation.map.tile_size as f32;
    if tile_size <= 0.0 {
        return Vec::new();
    }
    let own = tile_center(observation.own_start_tile, observation.map.tile_size);
    let map_center_tiles = (
        observation.map.width as f32 * 0.5,
        observation.map.height as f32 * 0.5,
    );
    let mut anchors: Vec<((u32, u32), f32, u32)> = Vec::new();

    for resource in resources
        .iter()
        .copied()
        .filter(|resource| resource.kind == EntityKind::Steel)
    {
        let Some(tile) =
            estimated_expansion_center_tile(observation, resource, map_center_tiles, tile_size)
        else {
            continue;
        };
        let center = tile_center(tile, observation.map.tile_size);
        let distance2 = dist2(center.0, center.1, own.0, own.1);
        anchors.push((tile, distance2, resource.id));
    }

    anchors.sort_by(
        |(left_tile, left_distance, left_id), (right_tile, right_distance, right_id)| {
            left_distance
                .total_cmp(right_distance)
                .then_with(|| left_id.cmp(right_id))
                .then_with(|| left_tile.cmp(right_tile))
        },
    );
    anchors.dedup_by_key(|(tile, _, _)| *tile);
    anchors.into_iter().map(|(tile, _, _)| tile).collect()
}

pub(super) fn expansion_cluster_resources_for_anchor<'a>(
    observation: &AiObservation,
    anchor: (u32, u32),
    resources: &[&'a AiResourceSummary],
) -> Vec<&'a AiResourceSummary> {
    let center = tile_center(anchor, observation.map.tile_size);
    let radius = (config::MINING_CC_RANGE_TILES + 2.0) * observation.map.tile_size as f32;
    let radius2 = squared(radius);
    resources
        .iter()
        .copied()
        .filter(|resource| dist2(resource.x, resource.y, center.0, center.1) <= radius2)
        .collect()
}

pub(super) fn estimated_expansion_center_tile(
    observation: &AiObservation,
    resource: &AiResourceSummary,
    map_center_tiles: (f32, f32),
    tile_size: f32,
) -> Option<(u32, u32)> {
    let resource_tile = (resource.x / tile_size, resource.y / tile_size);
    let dir = (
        map_center_tiles.0 - resource_tile.0,
        map_center_tiles.1 - resource_tile.1,
    );
    let len = (dir.0 * dir.0 + dir.1 * dir.1).sqrt();
    if len <= f32::EPSILON {
        return None;
    }
    let estimated_center = (
        resource_tile.0 - dir.0 / len * config::STEEL_BLOCK_DIST_TILES,
        resource_tile.1 - dir.1 / len * config::STEEL_BLOCK_DIST_TILES,
    );
    if !estimated_center.0.is_finite() || !estimated_center.1.is_finite() {
        return None;
    }
    Some((
        estimated_center
            .0
            .round()
            .clamp(0.0, observation.map.width.saturating_sub(1) as f32) as u32,
        estimated_center
            .1
            .round()
            .clamp(0.0, observation.map.height.saturating_sub(1) as f32) as u32,
    ))
}

#[derive(Clone, Copy, Debug)]
pub(super) struct ExpansionSiteCandidate {
    tile: (u32, u32),
    steel_in_range: usize,
    oil_in_range: usize,
    max_resource_distance2: f32,
    sum_resource_distance2: f32,
    own_distance2: f32,
    approach_exposure: Option<f32>,
}

pub(super) fn expansion_site_candidate(
    observation: &AiObservation,
    kind: EntityKind,
    tile_x: u32,
    tile_y: u32,
    resources: &[&AiResourceSummary],
) -> Option<ExpansionSiteCandidate> {
    let (cx, cy) = building_center((tile_x, tile_y), kind, observation.map.tile_size)?;
    let max_dist = config::MINING_CC_RANGE_TILES * observation.map.tile_size as f32;
    let max_dist2 = squared(max_dist);
    let mut steel_in_range = 0usize;
    let mut oil_in_range = 0usize;
    let mut max_resource_distance2 = 0.0f32;
    let mut sum_resource_distance2 = 0.0f32;

    for resource in resources {
        let distance2 = dist2(cx, cy, resource.x, resource.y);
        if distance2 > max_dist2 {
            continue;
        }
        match resource.kind {
            EntityKind::Steel => steel_in_range += 1,
            EntityKind::Oil => oil_in_range += 1,
            _ => {}
        }
        max_resource_distance2 = max_resource_distance2.max(distance2);
        sum_resource_distance2 += distance2;
    }
    if steel_in_range == 0 && oil_in_range == 0 {
        return None;
    }
    let own = tile_center(observation.own_start_tile, observation.map.tile_size);
    let own_distance2 = dist2(cx, cy, own.0, own.1);
    let enemy_distance2 = nearest_enemy_start_distance2(observation, cx, cy);
    Some(ExpansionSiteCandidate {
        tile: (tile_x, tile_y),
        steel_in_range,
        oil_in_range,
        max_resource_distance2,
        sum_resource_distance2,
        own_distance2,
        approach_exposure: expansion_approach_exposure(own_distance2, enemy_distance2),
    })
}

pub(super) fn expansion_site_candidate_better(
    candidate: ExpansionSiteCandidate,
    current: Option<ExpansionSiteCandidate>,
) -> bool {
    let Some(current) = current else {
        return true;
    };
    candidate
        .oil_in_range
        .cmp(&current.oil_in_range)
        .then_with(|| candidate.steel_in_range.cmp(&current.steel_in_range))
        .then_with(|| {
            expansion_approach_exposure_order(
                candidate.approach_exposure,
                current.approach_exposure,
            )
        })
        .then_with(|| {
            current
                .max_resource_distance2
                .total_cmp(&candidate.max_resource_distance2)
        })
        .then_with(|| {
            current
                .sum_resource_distance2
                .total_cmp(&candidate.sum_resource_distance2)
        })
        .then_with(|| current.own_distance2.total_cmp(&candidate.own_distance2))
        .then_with(|| current.tile.cmp(&candidate.tile))
        == Ordering::Greater
}

pub(super) fn expansion_approach_exposure(
    own_distance2: f32,
    enemy_distance2: Option<f32>,
) -> Option<f32> {
    enemy_distance2
        .filter(|distance2| distance2.is_finite() && *distance2 > f32::EPSILON)
        .map(|distance2| own_distance2 / distance2)
}

pub(super) fn expansion_approach_exposure_order(
    candidate: Option<f32>,
    current: Option<f32>,
) -> Ordering {
    match (candidate, current) {
        (Some(candidate), Some(current)) => current.total_cmp(&candidate),
        (Some(_), None) => Ordering::Greater,
        (None, Some(_)) => Ordering::Less,
        (None, None) => Ordering::Equal,
    }
}

pub(super) fn nearest_enemy_start_distance2(
    observation: &AiObservation,
    x: f32,
    y: f32,
) -> Option<f32> {
    observation
        .players
        .iter()
        .filter(|player| player.is_alive && observation.is_enemy_player(player.id))
        .map(|player| {
            let center = tile_center(player.start_tile, observation.map.tile_size);
            dist2(x, y, center.0, center.1)
        })
        .min_by(|left, right| left.total_cmp(right))
}

pub(super) fn resource_is_near_player_start(
    observation: &AiObservation,
    resource: &AiResourceSummary,
    radius2: f32,
) -> bool {
    observation.players.iter().any(|player| {
        let center = tile_center(player.start_tile, observation.map.tile_size);
        dist2(resource.x, resource.y, center.0, center.1) <= radius2
    })
}
