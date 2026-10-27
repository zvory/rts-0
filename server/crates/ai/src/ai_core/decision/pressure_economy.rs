//! Pressure's ranked expansion and five-base production policy.
use super::*;
use crate::sdk::{actions::AiActionRequest, AiBaseSiteId, WorldQueries};

pub(super) const TARGET_BASES: usize = 5;
pub(super) const MAX_FACTORIES: usize = 4;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct PressureEconomy {
    tank_repeat_enabled: bool,
    configured_factories: BTreeSet<u32>,
}

impl PressureEconomy {
    pub(super) fn tank_repeat_enabled(&self) -> bool {
        self.tank_repeat_enabled
    }

    pub(super) fn configure_tanks(
        &mut self,
        observation: &AiObservation,
        facts: &AiFacts,
        actions: &mut AiActionContext<'_>,
    ) {
        self.tank_repeat_enabled |=
            facts.complete_building_count(EntityKind::ResourceDepot) >= TARGET_BASES;
        self.configured_factories
            .retain(|id| observation.owned.iter().any(|entity| entity.id == *id));
        if !self.tank_repeat_enabled {
            return;
        }
        // Keep standing production after a base loss; the expansion policy can replace Depots.
        // Existing paid/unpaid manual queues finish before repeat takes over, without cancelling.
        for factory in observation
            .owned
            .iter()
            .filter(|entity| entity.kind == EntityKind::Factory && entity.is_complete)
        {
            if self.configured_factories.insert(factory.id) {
                actions.emit_action(AiActionRequest::AdjustProductionRepeat {
                    buildings: vec![factory.id],
                    unit: EntityKind::Tank,
                    delta: 1,
                });
            }
        }
    }
}

pub(super) fn spend_surplus_only(profile: &AiProfile, facts: &AiFacts) -> bool {
    profile.id == crate::ai_core::profiles::PRESSURE_AI_ID
        && facts.complete_building_count(EntityKind::ResourceDepot) >= 3
}

pub(super) fn protect_depot_in_transit(
    observation: &AiObservation,
    profile: &AiProfile,
    actions: &mut AiActionContext<'_>,
) {
    if profile.id == crate::ai_core::profiles::PRESSURE_AI_ID
        && observation
            .pending_builds
            .iter()
            .any(|build| build.kind == EntityKind::ResourceDepot)
    {
        // The shared budget already deducts travelling builds' Steel. Protect Oil as well,
        // allowing ordinary Tank orders above the commitment instead of pausing production.
        actions.holdback_resources(0, rts_rules::economy::cost(EntityKind::ResourceDepot).1);
    }
}

/// Pressure's own copy of the secured-site check: its ranked site must still be clear of known
/// Depots and placeable. The current Jeff's branch in `expansion.rs` is his alone.
pub(super) fn checked_site<F>(
    observation: &AiObservation,
    site: Option<(u32, u32)>,
    placeable: &mut F,
) -> Option<(u32, u32)>
where
    F: FnMut(EntityKind, u32, u32) -> bool,
{
    let site = site?;
    (expansion::expansion_site_is_separate(observation, site)
        && placeable(EntityKind::ResourceDepot, site.0, site.1))
    .then_some(site)
}

pub(super) fn plan_site<F>(
    observation: &AiObservation,
    profile: &AiProfile,
    queries: Option<&WorldQueries<'_>>,
    plan: &mut expansion::ExpansionPlan,
    placeable: &mut F,
) -> Option<(u32, u32)>
where
    F: FnMut(EntityKind, u32, u32) -> bool,
{
    if profile.id != crate::ai_core::profiles::PRESSURE_AI_ID || !plan.should_save {
        return None;
    }
    let policy = plan.policy?;
    let facts = AiFacts::from_observation(observation);
    let (steel, oil) = rts_rules::economy::cost(EntityKind::ResourceDepot);
    if spend_surplus_only(profile, &facts)
        && (facts.complete_building_count(EntityKind::Factory) < 2
            || observation.economy.steel < steel
            || observation.economy.oil < oil)
    {
        plan.should_save = false;
        plan.blockers.push(ExpansionBlocker::NotDue);
        return None;
    }
    let site = if let Some(queries) = queries {
        ranked_expansion_site(observation, policy, queries, placeable)
    } else {
        // Synthetic decision tests without a host frame retain the shared footprint search.
        expansion::expansion_resource_depot_site(
            observation,
            policy,
            EntityKind::ResourceDepot,
            profile.id,
            None,
            placeable,
        )
    };
    if site.is_none() {
        // A map with fewer usable sites must not freeze all production while saving forever.
        plan.should_save = false;
        plan.blockers.push(ExpansionBlocker::NoValidSite);
    }
    site
}

fn ranked_expansion_site<F>(
    observation: &AiObservation,
    policy: ExpansionPolicy,
    queries: &WorldQueries<'_>,
    placeable: &mut F,
) -> Option<(u32, u32)>
where
    F: FnMut(EntityKind, u32, u32) -> bool,
{
    let resources = expansion::expansion_candidate_resources(observation);
    for base in queries.ranked_bases(observation.player_id) {
        // Geography includes every player's main. Never expand into an original start or a site
        // with owned, allied, visible enemy, or remembered Depot evidence.
        if base.distance_from_main.is_none()
            || matches!(base.site.id, AiBaseSiteId::Start(_))
            || !queries.known_base_depots(base.site.id).is_empty()
        {
            continue;
        }
        let cluster: Vec<_> = resources
            .iter()
            .copied()
            .filter(|resource| base.site.resource_ids.contains(&resource.id))
            .collect();
        if !cluster
            .iter()
            .any(|resource| resource.kind == EntityKind::Steel)
        {
            continue;
        }
        if let Some(tile) = expansion::generic_expansion_depot_site(
            observation,
            policy,
            EntityKind::ResourceDepot,
            &cluster,
            placeable,
        ) {
            return Some(tile);
        }
    }
    None
}
