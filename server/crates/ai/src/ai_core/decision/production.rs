use super::*;
use crate::ai_core::profiles::UnitCountRequirement;

pub(super) const PRODUCTION_BUILDINGS: [EntityKind; 4] = [
    EntityKind::Factory,
    EntityKind::Steelworks,
    EntityKind::Barracks,
    EntityKind::CityCentre,
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

pub(super) fn build_search_for_kind(
    mut build_search: ai_shared::BuildSearch,
    profile: &AiProfile,
    kind: EntityKind,
) -> ai_shared::BuildSearch {
    match kind {
        EntityKind::Steelworks if profile.turtle_defense.is_some() => {
            build_search.min_radius = build_search
                .min_radius
                .min(ai_shared::TURTLE_GUN_WORKS_BUILD_SEARCH_MAX_RADIUS);
            build_search.max_radius = build_search
                .max_radius
                .min(ai_shared::TURTLE_GUN_WORKS_BUILD_SEARCH_MAX_RADIUS)
                .max(build_search.min_radius);
            build_search.prefer_away_from_center = false;
            build_search.prefer_toward_center = true;
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
    PRODUCTION_BUILDINGS
        .into_iter()
        .find(|building| rts_rules::economy::trainable_units(*building).contains(&unit))
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
    order.retain(|kind| *kind != EntityKind::CityCentre);
    order
}

pub(super) fn production_uses_building(production: ProductionPolicy, building: EntityKind) -> bool {
    production
        .unit_priorities
        .iter()
        .copied()
        .any(|unit| producer_for_unit(unit) == Some(building))
}

pub(super) fn prioritize_required_units(
    unit_priorities: &[EntityKind],
    requirements: &[UnitCountRequirement],
    current_counts: &[(EntityKind, usize)],
) -> Vec<EntityKind> {
    let current_counts: BTreeMap<EntityKind, usize> = current_counts.iter().copied().collect();
    let is_missing = |unit: EntityKind| {
        requirements.iter().any(|requirement| {
            requirement.kind == unit
                && current_counts.get(&unit).copied().unwrap_or(0) < requirement.count
        })
    };

    unit_priorities
        .iter()
        .copied()
        .filter(|unit| is_missing(*unit))
        .chain(
            unit_priorities
                .iter()
                .copied()
                .filter(|unit| !is_missing(*unit)),
        )
        .collect()
}

pub(super) fn unit_counts_for_priorities(
    observation: &AiObservation,
    facts: &AiFacts,
    unit_priorities: &[EntityKind],
) -> Vec<(EntityKind, usize)> {
    let mut counts: BTreeMap<EntityKind, usize> = unit_priorities
        .iter()
        .copied()
        .map(|unit| (unit, facts.unit_count(unit)))
        .collect();
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
    use crate::ai_core::profiles::{AI_2_1, AI_TURTLE};

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
    fn ordinary_buildings_keep_their_requested_search_band() {
        let search = build_search_for_kind(short_search(), &AI_2_1, EntityKind::Barracks);

        assert_eq!(search.max_radius, 6);
        assert!(search.prefer_away_from_center);
        assert!(!search.prefer_toward_center);
    }
}
