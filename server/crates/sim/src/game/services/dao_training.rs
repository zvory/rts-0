//! One-for-one, oil-paid Disciple specialization. Plans never create units without entrants.
use crate::config;
use crate::game::ability::AbilityKind;
use crate::game::entity::{EntityKind, EntityStore, ProdItem};
use crate::game::upgrade::UpgradeKind;

pub(crate) fn is_dao(choice: UpgradeKind) -> bool {
    matches!(
        choice,
        UpgradeKind::DaoWind | UpgradeKind::DaoOfHeaviness | UpgradeKind::DaoOfMagic
    )
}

pub(in crate::game) fn enqueue(
    entities: &mut EntityStore,
    owner: u32,
    building: u32,
    choice: UpgradeKind,
    unlocked: bool,
) {
    if !is_dao(choice) || !unlocked {
        return;
    }
    if let Some(p) = entities
        .get_mut(building)
        .filter(|e| {
            e.owner == owner
                && e.hp > 0
                && e.kind == EntityKind::ProvenGrounds
                && !e.under_construction()
        })
        .and_then(|e| e.production.as_mut())
    {
        if p.dao_training_queue.len() < config::MAX_DAO_TRAINING_QUEUE {
            p.dao_training_queue.push(choice);
        }
    }
}

/// Called only after the shared entry flow validates ownership, unit eligibility and perimeter.
pub(super) fn admit(entities: &mut EntityStore, building: u32, unit: u32) {
    // The shared entry flow reserves the Adept's extra supply before admission. The
    // authoritative supply pass rebuilds it from this item; there is no free-supply window.
    entities.remove(unit);
    if let Some(p) = entities
        .get_mut(building)
        .and_then(|e| e.production.as_mut())
    {
        p.queue.push(ProdItem {
            unit: EntityKind::Warrior,
            progress: 0,
            total: config::DAO_TRAINING_TICKS,
            paid: true,
        });
    }
}

pub(super) fn upgrade_trainee(entities: &mut EntityStore, building: u32, trainee: u32) {
    let choice = entities
        .get_mut(building)
        .filter(|e| e.kind == EntityKind::ProvenGrounds)
        .and_then(|e| e.production.as_mut())
        .and_then(|p| (!p.dao_training_queue.is_empty()).then(|| p.dao_training_queue.remove(0)));
    let Some(choice) = choice else {
        return;
    };
    if let Some(e) = entities.get_mut(trainee) {
        e.gain_non_passive_insight(config::DISCIPLE_INSIGHT_REQUIRED.saturating_sub(e.insight));
        e.evolve_dao(match choice {
            UpgradeKind::DaoWind => AbilityKind::EvolveWind,
            UpgradeKind::DaoOfHeaviness => AbilityKind::EvolveWar,
            _ => AbilityKind::EvolveMagic,
        });
    }
}
