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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::entity::{BuildPhase, Order};

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

    #[test]
    fn rejects_multiple_active_builders_for_one_site() {
        let site = 7;
        let mut workers = [1, 2].map(|id| {
            let mut worker =
                Entity::new_unit(1, EntityKind::Worker, 32.0, 32.0).expect("worker should spawn");
            worker.id = id;
            worker.set_order(Order::build(EntityKind::Depot, 1, 1));
            worker.mark_build_phase(BuildPhase::Constructing { site });
            worker
        });
        assert!(matches!(
            validate_single_builders(&workers),
            Err(CheckpointPayloadError::DuplicateId {
                field: "entities.order.buildSite",
                id: 7
            })
        ));
        workers[1].clear_active_order();
        assert!(validate_single_builders(&workers).is_ok());
    }
}
