use crate::game::entity::{Entity, EntityKind};

use super::CheckpointPayloadError;

pub(super) fn validate_ownership(entity: &Entity) -> Result<(), CheckpointPayloadError> {
    if entity.kind == EntityKind::TankTrap
        && entity.under_construction() == (entity.owner == 0)
    {
        return Err(CheckpointPayloadError::InvalidValue {
            field: "entities.owner",
        });
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
