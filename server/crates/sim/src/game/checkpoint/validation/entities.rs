use std::collections::BTreeSet;

use crate::game::entity::{Entity, EntityKind};

use super::CheckpointPayloadError;

pub(in crate::game::checkpoint) fn normalize_completed_tank_traps(
    entities: &mut [Entity],
) -> BTreeSet<u32> {
    entities
        .iter_mut()
        .filter_map(|entity| entity.neutralize_completed_tank_trap().then_some(entity.id))
        .collect()
}

pub(super) fn validate_ownership(entity: &Entity) -> Result<(), CheckpointPayloadError> {
    if entity.kind == EntityKind::TankTrap && entity.under_construction() == (entity.owner == 0) {
        return Err(CheckpointPayloadError::InvalidValue {
            field: "entities.owner",
        });
    }
    Ok(())
}

pub(super) fn validate_single_builders(entities: &[Entity]) -> Result<(), CheckpointPayloadError> {
    let mut active_sites = BTreeSet::new();
    for site in entities
        .iter()
        .filter(|entity| entity.hp > 0 && entity.is_unit())
        .filter_map(|entity| entity.order().build_site())
    {
        if !active_sites.insert(site) {
            return Err(CheckpointPayloadError::DuplicateId {
                field: "entities.order.buildSite",
                id: site,
            });
        }
    }
    Ok(())
}

pub(super) fn validate_tournament_state(entity: &Entity) -> Result<(), CheckpointPayloadError> {
    use crate::game::upgrade::UpgradeKind;
    if entity
        .movement
        .as_ref()
        .is_some_and(|m| m.tournament_target.is_some() && !entity.kind.is_novice())
    {
        return Err(CheckpointPayloadError::InvalidValue {
            field: "entities.tournamentTarget",
        });
    }
    if let Some(p) = entity.production.as_ref() {
        let invalid_training = if entity.kind == EntityKind::ProvenGrounds {
            p.dao_training_queue.len() > crate::config::MAX_DAO_TRAINING_QUEUE
                || p.dao_training_queue
                    .iter()
                    .any(|dao| !crate::game::services::dao_training::is_dao(*dao))
                || p.queue.len() > 1
                || p.queue.len() > p.dao_training_queue.len()
                || !p.research_queue.is_empty()
                || !p.repeat_units.is_empty()
                || p.queue.iter().any(|item| {
                    item.unit != EntityKind::Warrior
                        || !item.paid
                        || item.total != crate::config::DAO_TRAINING_TICKS
                        || item.progress > item.total
                })
        } else {
            !p.dao_training_queue.is_empty()
        };
        if invalid_training {
            return Err(CheckpointPayloadError::InvalidValue {
                field: "entities.daoTraining",
            });
        }
        let invalid = if entity.kind == EntityKind::TournamentGrounds {
            p.tournament_entrants > 4
                || p.tournament_winners.len() > 1
                || p.queue.len() != p.tournament_winners.len()
                || (p.tournament_winners.is_empty() && p.tournament_entrants == 4)
                || (!p.tournament_winners.is_empty() && p.tournament_entrants != 4)
                || p.queue
                    .iter()
                    .any(|item| item.unit != EntityKind::Warrior || !item.paid)
                || p.tournament_winners.iter().any(|dao| {
                    !matches!(
                        dao,
                        UpgradeKind::DaoWind
                            | UpgradeKind::DaoOfHeaviness
                            | UpgradeKind::DaoOfMagic
                    )
                })
        } else {
            p.tournament_entrants != 0 || !p.tournament_winners.is_empty()
        };
        if invalid {
            return Err(CheckpointPayloadError::InvalidValue {
                field: "entities.tournament",
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tank_trap_owner_matches_construction_state() {
        let scaffold = Entity::new_building(1, EntityKind::TankTrap, 32.0, 32.0, false)
            .expect("Tank Trap scaffold should spawn");
        let completed = Entity::new_building(1, EntityKind::TankTrap, 32.0, 32.0, true)
            .expect("completed Tank Trap should spawn");

        assert!(validate_ownership(&scaffold).is_ok());
        assert!(validate_ownership(&completed).is_ok());

        let mut neutral_scaffold = scaffold;
        neutral_scaffold.owner = 0;
        assert!(validate_ownership(&neutral_scaffold).is_err());

        let mut owned_obstacle = completed;
        owned_obstacle.owner = 1;
        assert!(validate_ownership(&owned_obstacle).is_err());
    }
}
