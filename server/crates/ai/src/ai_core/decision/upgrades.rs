use super::*;

pub(super) fn queue_upgrade_if_available(
    actions: &mut AiActionContext<'_>,
    facts: &AiFacts,
    memory: &mut AiDecisionMemory,
    intents: &mut Vec<AiIntent>,
    upgrade: UpgradeKind,
) {
    if facts.completed_upgrades().contains(&upgrade) || memory.pending_upgrades.contains(&upgrade) {
        return;
    }
    let definition = upgrade::definition(upgrade);
    if facts.complete_building_count(EntityKind::Factory) > 0
        && memory.expansion_security.site.is_some()
        && facts.building_count(EntityKind::ResourceDepot) < 2
        && planned_in_intents(intents, EntityKind::ResourceDepot) == 0
        && definition.cost_oil > 0
    {
        return;
    }
    if facts.complete_building_count(definition.researched_at) == 0 {
        return;
    }
    if let Some(researched) = actions::try_research_upgrade(
        actions,
        facts.production_buildings(definition.researched_at),
        upgrade,
    ) {
        memory.pending_upgrades.insert(researched.upgrade);
        intents.push(AiIntent::Research {
            upgrade: researched.upgrade,
        });
    }
}

pub(super) fn queue_profile_upgrades(
    actions: &mut AiActionContext<'_>,
    facts: &AiFacts,
    memory: &mut AiDecisionMemory,
    intents: &mut Vec<AiIntent>,
    profile: &AiProfile,
) {
    for upgrade in profile.upgrade_priorities {
        if profile.fast_tank_timing.is_some()
            && *upgrade == UpgradeKind::TankUnlock
            && if is_jeffs_ai_profile(profile.id) {
                facts.building_counts(EntityKind::Factory).existing == 0
            } else {
                facts.building_count(EntityKind::Factory) == 0
            }
        {
            continue;
        }
        queue_upgrade_if_available(actions, facts, memory, intents, *upgrade);
    }
}

pub(super) fn queue_fast_tank_optional_upgrades(
    actions: &mut AiActionContext<'_>,
    facts: &AiFacts,
    memory: &mut AiDecisionMemory,
    intents: &mut Vec<AiIntent>,
    profile: &AiProfile,
) {
    let Some(timing) = profile.fast_tank_timing else {
        return;
    };
    if facts.unit_count(EntityKind::Tank) < timing.tanks_before_optional_upgrades {
        return;
    }
    for upgrade in timing.optional_upgrades {
        queue_upgrade_if_available(actions, facts, memory, intents, *upgrade);
    }
}

pub(super) fn queue_jeff_infantry_mass_methamphetamines(
    actions: &mut AiActionContext<'_>,
    facts: &AiFacts,
    memory: &mut AiDecisionMemory,
    intents: &mut Vec<AiIntent>,
    profile: &AiProfile,
) {
    if !is_jeffs_ai_profile(profile.id)
        || facts
            .unit_count(EntityKind::Rifleman)
            .saturating_add(facts.unit_count(EntityKind::MachineGunner))
            <= 15
    {
        return;
    }
    queue_upgrade_if_available(
        actions,
        facts,
        memory,
        intents,
        UpgradeKind::Methamphetamines,
    );
}

pub(super) fn queue_required_unit_unlocks(
    actions: &mut AiActionContext<'_>,
    facts: &AiFacts,
    unit_priorities: &[EntityKind],
    memory: &mut AiDecisionMemory,
    intents: &mut Vec<AiIntent>,
    profile: &AiProfile,
) {
    for unit in unit_priorities {
        let Some(upgrade) = upgrade::required_for_unit(*unit) else {
            continue;
        };
        if profile.fast_tank_timing.is_some()
            && upgrade == UpgradeKind::TankUnlock
            && if is_jeffs_ai_profile(profile.id) {
                facts.building_counts(EntityKind::Factory).existing == 0
            } else {
                facts.building_count(EntityKind::Factory) == 0
            }
        {
            continue;
        }
        queue_upgrade_if_available(actions, facts, memory, intents, upgrade);
    }
}
