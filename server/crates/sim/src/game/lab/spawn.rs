use super::*;

impl Game {
    pub(super) fn lab_spawn_entity(
        &mut self,
        input: LabSpawnEntity,
    ) -> Result<LabOpOutcome, LabError> {
        self.validate_owner(input.owner)?;
        if input.dao.is_some() && !input.kind.is_novice() {
            return Err(invalid_kind(
                input.kind,
                "Dao specialization requires a Novice",
            ));
        }
        if let Some(faction_id) = input.faction_id {
            let catalog = rts_rules::faction::catalog_for(faction_id)
                .ok_or_else(|| invalid_kind(input.kind, "unknown spawn faction"))?;
            if !catalog.units.contains(&input.kind) && !catalog.buildings.contains(&input.kind) {
                return Err(invalid_kind(
                    input.kind,
                    "kind is not in spawn faction catalog",
                ));
            }
        }
        if input.dao_master && input.kind.dao().is_none() {
            return Err(invalid_kind(
                input.kind,
                "Master rank requires a Wind, War, or Magic Adept",
            ));
        }
        let id = if input.dao_master {
            let mut unit = Entity::new_unit(input.owner, input.kind, input.x, input.y).ok_or(
                LabError::InvalidPosition {
                    x: input.x,
                    y: input.y,
                    reason: "position cannot be stored",
                },
            )?;
            unit.insight = config::DISCIPLE_EVOLVED_INSIGHT_CAP;
            unit.advance_dao_master();
            unit.dao_advance_ticks = 0;
            self.validate_unit_entity_position(&self.state.entities, &unit, input.x, input.y)?;
            self.state.entities.insert(unit)
        } else if input.kind.is_unit() {
            self.validate_unit_position(&self.state.entities, input.kind, input.x, input.y)?;
            self.state
                .entities
                .spawn_unit(input.owner, input.kind, input.x, input.y)
                .ok_or_else(|| invalid_kind(input.kind, "spawnEntity"))?
        } else if input.kind.is_building() {
            let (_, _, x, y) = self.validate_building_position(
                &self.state.entities,
                input.kind,
                input.x,
                input.y,
            )?;
            self.state
                .entities
                .spawn_building(input.owner, input.kind, x, y, input.completed)
                .ok_or_else(|| invalid_kind(input.kind, "spawnEntity"))?
        } else {
            return Err(invalid_kind(input.kind, "spawnEntity"));
        };
        if let Some(faction_id) = input.faction_id {
            if let Some(player) = self.state.players.iter_mut().find(|p| p.id == input.owner) {
                player.faction_id = faction_id.to_string();
            }
        }
        if let (Some(dao), Some(entity)) = (input.dao, self.state.entities.get_mut(id)) {
            use crate::game::ability::AbilityKind;
            use crate::protocol::LabSpawnDao;
            entity.insight = config::DISCIPLE_INSIGHT_REQUIRED;
            entity.evolve_dao(match dao {
                LabSpawnDao::Wind => AbilityKind::EvolveWind,
                LabSpawnDao::War => AbilityKind::EvolveWar,
                LabSpawnDao::Magic => AbilityKind::EvolveMagic,
            });
            entity.dao_advance_ticks = 0;
        }
        Ok(LabOpOutcome::Spawned { entity_id: id })
    }
}

pub(super) fn unit_entity_position_valid(
    map: &Map,
    entities: &EntityStore,
    entity: &Entity,
    x: f32,
    y: f32,
) -> bool {
    let occ = Occupancy::build(map, entities);
    standability::unit_spawn_standable_for_entity(map, &occ, entities, entity, x, y)
}
