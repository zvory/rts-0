use crate::game::entity::{Entity, EntityKind, EntityStore};
use crate::game::upgrade::{self, UpgradeKind};
use crate::rules;
use std::collections::BTreeSet;

pub(super) fn next_unlocked(
    entities: &mut EntityStore,
    producer_id: u32,
    faction_id: &str,
    completed_upgrades: Option<&BTreeSet<UpgradeKind>>,
    owned_complete: &[EntityKind],
    repeat_count: usize,
) -> Option<EntityKind> {
    for _ in 0..repeat_count {
        let unit = entities
            .get(producer_id)
            .and_then(Entity::repeat_production)?;
        let requirements_met =
            rules::economy::train_requirement_met_for_faction(faction_id, unit, owned_complete);
        let upgrade_met = upgrade::required_for_unit(unit).is_none_or(|required| {
            completed_upgrades.is_some_and(|upgrades| upgrades.contains(&required))
        });
        if requirements_met && upgrade_met {
            return Some(unit);
        }

        // Keep hard-locked choices enabled for when their prerequisite completes, but ignore them
        // while choosing work so another enabled unit cannot be starved.
        if !entities
            .get_mut(producer_id)
            .is_some_and(|producer| producer.set_repeat_production(None, true))
        {
            return None;
        }
    }
    None
}
