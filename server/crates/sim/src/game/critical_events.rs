use super::{entity::EntityKind, Game};
use crate::rules::defs::{self, TechRequirement};

/// Read-only facts for room-owned chat notifications; never sent directly to clients.
#[derive(Debug, Clone)]
pub struct PlayerCriticalEventState {
    pub player_id: u32,
    pub supply_used: u32,
    pub supply_cap: u32,
    pub blocked_production: Vec<(EntityKind, EntityKind)>,
}

impl Game {
    /// Authoritative economy facts. The room owns notification cooldowns and recipient routing.
    pub fn critical_event_state(&self) -> Vec<PlayerCriticalEventState> {
        self.state
            .players
            .iter()
            .map(|player| {
                let buildings: Vec<_> = self
                    .state
                    .entities
                    .iter()
                    .filter(|entity| {
                        entity.owner == player.id
                            && entity.hp > 0
                            && entity.is_building()
                            && !entity.under_construction()
                    })
                    .collect();
                let completed: Vec<_> = buildings.iter().map(|entity| entity.kind).collect();
                let mut blocked_production = Vec::new();
                for building in buildings {
                    if let Some(production) = &building.production {
                        for &unit in &production.repeat_units {
                            let Some(definition) = defs::unit_def(unit) else {
                                continue;
                            };
                            if definition.train_requirement.is_met(&completed) {
                                continue;
                            }
                            let requirements = match definition.train_requirement {
                                TechRequirement::All(requirements)
                                | TechRequirement::Any(requirements) => requirements,
                            };
                            for &required in requirements {
                                if !completed.contains(&required)
                                    && !blocked_production.contains(&(unit, required))
                                {
                                    blocked_production.push((unit, required));
                                }
                            }
                        }
                    }
                }
                PlayerCriticalEventState {
                    player_id: player.id,
                    supply_used: player.supply_used,
                    supply_cap: crate::config::PLAYER_SUPPLY_CAP,
                    blocked_production,
                }
            })
            .collect()
    }
}
