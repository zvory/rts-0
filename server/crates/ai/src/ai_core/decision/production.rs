use super::geometry::{clamp_to_map, normalized_direction};
use super::*;

pub(super) const PRODUCTION_BUILDINGS: [EntityKind; 4] = [
    EntityKind::Factory,
    EntityKind::Steelworks,
    EntityKind::Barracks,
    EntityKind::ResourceDepot,
];

pub(super) fn should_build_extra_factory(
    observation: &AiObservation,
    facts: &AiFacts,
    profile: &AiProfile,
    planned_factories: usize,
) -> bool {
    let Some(policy) = profile.extra_factories else {
        return false;
    };
    if facts.unit_count(policy.prerequisite_unit) < policy.minimum_units {
        return false;
    }
    if observation.economy.steel <= policy.resource_float.steel
        || observation.economy.oil <= policy.resource_float.oil
    {
        return false;
    }
    facts
        .building_count(EntityKind::Factory)
        .saturating_add(planned_factories)
        < policy.target_count
}

pub(super) fn should_build_extra_turtle_gun_works(
    observation: &AiObservation,
    facts: &AiFacts,
    profile: &AiProfile,
    planned_gun_works: usize,
) -> bool {
    let Some(policy) = profile.turtle_defense else {
        return false;
    };
    if observation.economy.steel <= policy.gun_works_resource_float.steel
        || observation.economy.oil <= policy.gun_works_resource_float.oil
    {
        return false;
    }
    facts.complete_building_count(EntityKind::Steelworks) > 0
        && facts
            .building_count(EntityKind::Steelworks)
            .saturating_add(planned_gun_works)
            < policy.gun_works_target
}

#[allow(clippy::too_many_arguments)]
pub(super) fn try_build_kind<F>(
    observation: &AiObservation,
    facts: &AiFacts,
    actions: &mut AiActionContext<'_>,
    builder_pools: &[&[u32]],
    profile: &AiProfile,
    kind: EntityKind,
    build_search: ai_shared::BuildSearch,
    placeable: &mut F,
) -> Option<actions::BuildAction>
where
    F: FnMut(EntityKind, u32, u32) -> bool,
{
    config::building_stats(kind)?;
    if !rts_rules::economy::build_requirement_met(kind, facts.complete_building_kinds()) {
        return None;
    }
    let counts = facts.building_counts(kind);
    if counts.incomplete + counts.intended >= profile.buildings.max_pending_per_kind {
        return None;
    }
    let build_search = build_search_for_kind(build_search, profile, kind);
    let empty = BTreeSet::new();
    if uses_jeff_opposite_spawn_layout(observation, profile) {
        let (tile_x, tile_y) = ai_shared::find_build_spot_mirrored_from_opposite_spawn_with(
            observation.map.width,
            observation.map.height,
            observation.own_start_tile,
            kind,
            build_search,
            &empty,
            |tx, ty| placeable(kind, tx, ty),
        )?;
        return actions::try_build_at(actions, builder_pools, kind, tile_x, tile_y);
    }
    actions::try_build(
        actions,
        builder_pools,
        BuildPlacementRequest {
            building: kind,
            map_width: observation.map.width,
            map_height: observation.map.height,
            start_tile: observation.own_start_tile,
            search: build_search,
            skip_tiles: &empty,
            placeable: |tx, ty| placeable(kind, tx, ty),
        },
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn relocate_machine_gunners_blocking_factory<F>(
    observation: &AiObservation,
    actions: &mut AiActionContext<'_>,
    profile: &AiProfile,
    build_search: ai_shared::BuildSearch,
    machine_gunners: &[u32],
    enemy_base: EnemyBaseFact,
    placeable: &mut F,
) -> Option<Vec<u32>>
where
    F: FnMut(EntityKind, u32, u32) -> bool,
{
    if machine_gunners.is_empty() || !actions.budget().can_afford_building(EntityKind::Factory) {
        return None;
    }
    let stats = config::building_stats(EntityKind::Factory)?;
    let search = build_search_for_kind(build_search, profile, EntityKind::Factory);
    let empty = BTreeSet::new();
    let units_by_id: BTreeMap<u32, &AiEntitySummary> = observation
        .owned
        .iter()
        .filter(|entity| machine_gunners.contains(&entity.id))
        .map(|entity| (entity.id, entity))
        .collect();
    let mut blocked_by_machine_gunner = |tile_x, tile_y| {
        if placeable(EntityKind::Factory, tile_x, tile_y) {
            return false;
        }
        units_by_id.values().any(|unit| {
            let unit_tile = (
                (unit.x / observation.map.tile_size as f32).floor() as u32,
                (unit.y / observation.map.tile_size as f32).floor() as u32,
            );
            unit_tile.0 >= tile_x
                && unit_tile.0 < tile_x.saturating_add(stats.foot_w)
                && unit_tile.1 >= tile_y
                && unit_tile.1 < tile_y.saturating_add(stats.foot_h)
        })
    };
    let blocked_site = if uses_jeff_opposite_spawn_layout(observation, profile) {
        ai_shared::find_build_spot_mirrored_from_opposite_spawn_with(
            observation.map.width,
            observation.map.height,
            observation.own_start_tile,
            EntityKind::Factory,
            search,
            &empty,
            &mut blocked_by_machine_gunner,
        )
    } else {
        ai_shared::find_build_spot_near_start_with(
            observation.map.width,
            observation.map.height,
            observation.own_start_tile,
            EntityKind::Factory,
            search,
            &empty,
            &mut blocked_by_machine_gunner,
        )
    }?;
    let blockers: Vec<u32> = units_by_id
        .values()
        .filter_map(|unit| {
            let unit_tile = (
                (unit.x / observation.map.tile_size as f32).floor() as u32,
                (unit.y / observation.map.tile_size as f32).floor() as u32,
            );
            (unit_tile.0 >= blocked_site.0
                && unit_tile.0 < blocked_site.0.saturating_add(stats.foot_w)
                && unit_tile.1 >= blocked_site.1
                && unit_tile.1 < blocked_site.1.saturating_add(stats.foot_h))
            .then_some(unit.id)
        })
        .collect();
    relocate_machine_gunners_from_factory_site(
        observation,
        actions,
        blocked_site,
        &blockers,
        enemy_base,
    )
}

fn uses_jeff_opposite_spawn_layout(observation: &AiObservation, profile: &AiProfile) -> bool {
    matches!(profile.id, JEFFS_AI_ID | JEFFS_AI_BETA_ID)
        && is_upper_left_diagonal_start(observation.map, observation.own_start_tile)
}

fn is_upper_left_diagonal_start(map: AiMapSummary, start: (u32, u32)) -> bool {
    start.0 == start.1 && start.0 < map.width / 2 && start.1 < map.height / 2
}

pub(super) fn relocate_machine_gunners_from_factory_site(
    observation: &AiObservation,
    actions: &mut AiActionContext<'_>,
    site: (u32, u32),
    machine_gunners: &[u32],
    enemy_base: EnemyBaseFact,
) -> Option<Vec<u32>> {
    let stats = config::building_stats(EntityKind::Factory)?;
    let tile_size = observation.map.tile_size as f32;
    let clearance = tile_size;
    let left = site.0 as f32 * tile_size - clearance;
    let top = site.1 as f32 * tile_size - clearance;
    let right = site.0.saturating_add(stats.foot_w) as f32 * tile_size + clearance;
    let bottom = site.1.saturating_add(stats.foot_h) as f32 * tile_size + clearance;
    let machine_gunners: BTreeSet<u32> = machine_gunners.iter().copied().collect();
    let mut moved = Vec::new();
    for unit in observation.owned.iter().filter(|unit| {
        machine_gunners.contains(&unit.id)
            && unit.kind == EntityKind::MachineGunner
            && unit.x >= left
            && unit.x <= right
            && unit.y >= top
            && unit.y <= bottom
    }) {
        let Some(direction) = normalized_direction((unit.x, unit.y), (enemy_base.x, enemy_base.y))
        else {
            continue;
        };
        let destination = clamp_to_map(
            (
                unit.x + direction.0 * 4.0 * tile_size,
                unit.y + direction.1 * 4.0 * tile_size,
            ),
            observation.map,
        );
        if let Some(units) = actions::move_units(actions, [unit.id], destination.0, destination.1) {
            moved.extend(units);
        }
    }
    (!moved.is_empty()).then_some(moved)
}

pub(super) fn build_search_for_kind(
    mut build_search: ai_shared::BuildSearch,
    profile: &AiProfile,
    kind: EntityKind,
) -> ai_shared::BuildSearch {
    match kind {
        EntityKind::Steelworks
            if profile.turtle_defense.is_some() || profile.home_anti_tank.is_some() =>
        {
            build_search.min_radius = build_search
                .min_radius
                .min(ai_shared::TURTLE_GUN_WORKS_BUILD_SEARCH_MAX_RADIUS);
            build_search.max_radius = build_search
                .max_radius
                .min(ai_shared::TURTLE_GUN_WORKS_BUILD_SEARCH_MAX_RADIUS)
                .max(build_search.min_radius);
            build_search.prefer_away_from_center = false;
            build_search.prefer_toward_center = profile.turtle_defense.is_some();
        }
        EntityKind::Factory if profile.fast_tank_timing.is_some() => {
            // The first Factory is on the Tank critical path. A compact site
            // avoids sending its builder to the edge of the generic forward
            // production band while retaining clearance around the main base.
            build_search.min_radius = build_search
                .min_radius
                .max(ai_shared::FAST_TANK_FACTORY_BUILD_SEARCH_MIN_RADIUS);
            build_search.max_radius = build_search
                .max_radius
                .min(ai_shared::FAST_TANK_FACTORY_BUILD_SEARCH_MAX_RADIUS)
                .max(build_search.min_radius);
            build_search.prefer_away_from_center = false;
            build_search.prefer_toward_center = false;
        }
        EntityKind::Factory | EntityKind::Steelworks => {
            build_search.max_radius = build_search
                .max_radius
                .max(ai_shared::FORWARD_PRODUCTION_BUILD_SEARCH_MAX_RADIUS);
            build_search.prefer_away_from_center = false;
            build_search.prefer_toward_center = true;
        }
        _ => {}
    }
    build_search
}

pub(super) fn should_save_for_first_tech_unit(
    facts: &AiFacts,
    production: ProductionPolicy,
) -> bool {
    let Some(unit) = production.save_for_first_tech_unit else {
        return false;
    };
    if facts.unit_count(unit) > 0 {
        return false;
    }
    let Some(producer) = producer_for_unit(unit) else {
        return false;
    };
    facts.building_count(producer) > 0
}

pub(super) fn should_save_for_required_tech_building(
    facts: &AiFacts,
    required_tech_path: &[EntityKind],
    production: ProductionPolicy,
) -> bool {
    let Some(unit) = production.save_for_first_tech_unit else {
        return false;
    };
    if facts.unit_count(unit) > 0 {
        return false;
    }
    let Some(producer) = producer_for_unit(unit) else {
        return false;
    };
    if facts.building_count(producer) == 0 {
        return required_tech_path.contains(&producer)
            && rts_rules::economy::build_requirement_met(
                producer,
                facts.complete_building_kinds(),
            );
    }
    if rts_rules::economy::train_requirement_met(unit, facts.complete_building_kinds()) {
        return false;
    }
    required_tech_path.iter().copied().any(|kind| {
        facts.building_count(kind) == 0
            && rts_rules::economy::build_requirement_met(kind, facts.complete_building_kinds())
    })
}

pub(super) fn producer_for_unit(unit: EntityKind) -> Option<EntityKind> {
    let rules = crate::sdk::AiRulebook::compatibility_default();
    PRODUCTION_BUILDINGS
        .into_iter()
        .find(|building| rules.can_train(*building, unit))
}

pub(super) fn production_building_order(unit_priorities: &[EntityKind]) -> Vec<EntityKind> {
    let mut order = Vec::new();
    for unit in unit_priorities {
        if let Some(building) = producer_for_unit(*unit) {
            if !order.contains(&building) {
                order.push(building);
            }
        }
    }
    order.retain(|kind| *kind != EntityKind::ResourceDepot);
    order
}

pub(super) fn production_uses_building(production: ProductionPolicy, building: EntityKind) -> bool {
    production
        .unit_priorities
        .iter()
        .copied()
        .any(|unit| producer_for_unit(unit) == Some(building))
}

pub(super) fn unit_counts_for_priorities(
    observation: &AiObservation,
    facts: &AiFacts,
    profile: &AiProfile,
    unit_priorities: &[EntityKind],
) -> Vec<(EntityKind, usize)> {
    let mut counts: BTreeMap<EntityKind, usize> = unit_priorities
        .iter()
        .copied()
        .map(|unit| (unit, facts.unit_count(unit)))
        .collect();
    if let Some(policy) = profile.defensive_machine_gunners {
        if let Some(threshold) = policy.replacement_health_percent {
            let healthy = observation
                .owned
                .iter()
                .filter(|entity| entity.kind == EntityKind::MachineGunner)
                .filter(|entity| machine_gunner_meets_replacement_health(entity.hp, threshold))
                .count();
            counts.insert(EntityKind::MachineGunner, healthy);
        }
    }
    for building in observation.owned.iter().filter(|entity| entity.is_complete) {
        let Some(kind) = building.production_kind else {
            continue;
        };
        if !unit_priorities.contains(&kind) {
            continue;
        }
        let queued = building.production_queue_len.unwrap_or(0);
        *counts.entry(kind).or_default() += queued;
    }
    unit_priorities
        .iter()
        .copied()
        .map(|unit| (unit, counts.get(&unit).copied().unwrap_or(0)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai_core::profiles::{AI_2_1, AI_TURTLE, JEFFS_AI};

    fn short_search() -> ai_shared::BuildSearch {
        ai_shared::BuildSearch {
            min_radius: 2,
            max_radius: 6,
            prefer_away_from_center: true,
            prefer_toward_center: false,
        }
    }

    #[test]
    fn vehicle_and_standard_gun_works_use_modest_forward_build_search() {
        for kind in [EntityKind::Factory, EntityKind::Steelworks] {
            let search = build_search_for_kind(short_search(), &AI_2_1, kind);
            assert_eq!(
                search.max_radius,
                ai_shared::FORWARD_PRODUCTION_BUILD_SEARCH_MAX_RADIUS
            );
            assert_eq!(
                search.max_radius,
                ai_shared::DEFAULT_BUILD_SEARCH_MAX_RADIUS + 2
            );
            assert!(!search.prefer_away_from_center);
            assert!(search.prefer_toward_center);
        }
    }

    #[test]
    fn turtle_gun_works_use_a_half_range_forward_build_search() {
        let search = build_search_for_kind(
            ai_shared::BuildSearch {
                min_radius: 2,
                max_radius: ai_shared::DEFAULT_BUILD_SEARCH_MAX_RADIUS,
                prefer_away_from_center: false,
                prefer_toward_center: false,
            },
            &AI_TURTLE,
            EntityKind::Steelworks,
        );

        assert_eq!(
            search.max_radius,
            ai_shared::TURTLE_GUN_WORKS_BUILD_SEARCH_MAX_RADIUS
        );
        assert_eq!(
            search.max_radius * 2,
            ai_shared::FORWARD_PRODUCTION_BUILD_SEARCH_MAX_RADIUS
        );
        assert!(!search.prefer_away_from_center);
        assert!(search.prefer_toward_center);
    }

    #[test]
    fn fast_tank_factory_uses_compact_build_search() {
        let search = build_search_for_kind(
            ai_shared::BuildSearch::default(),
            &JEFFS_AI,
            EntityKind::Factory,
        );
        assert_eq!(
            search.min_radius,
            ai_shared::FAST_TANK_FACTORY_BUILD_SEARCH_MIN_RADIUS
        );
        assert_eq!(
            search.max_radius,
            ai_shared::FAST_TANK_FACTORY_BUILD_SEARCH_MAX_RADIUS
        );
        assert!(!search.prefer_away_from_center);
        assert!(!search.prefer_toward_center);
    }

    #[test]
    fn ordinary_buildings_keep_their_requested_search_band() {
        let search = build_search_for_kind(short_search(), &AI_2_1, EntityKind::Barracks);

        assert_eq!(search.max_radius, 6);
        assert!(search.prefer_away_from_center);
        assert!(!search.prefer_toward_center);
    }

    #[test]
    fn mirrored_layout_scope_excludes_off_diagonal_crossroads_start() {
        let map = AiMapSummary {
            width: 126,
            height: 126,
            tile_size: 32,
        };

        assert!(is_upper_left_diagonal_start(map, (9, 9)));
        assert!(!is_upper_left_diagonal_start(map, (47, 8)));
        assert!(!is_upper_left_diagonal_start(map, (116, 116)));
    }
}
