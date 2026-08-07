use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::entity::{EntityKind, EntityStore};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VehicleOilRecord {
    pub entity_id: u32,
    pub owner_id: u32,
    pub unit_kind: String,
    pub first_seen_tick: u32,
    pub last_seen_tick: u32,
    pub first_moved_tick: Option<u32>,
    pub last_moved_tick: Option<u32>,
    pub lifetime_oil_spend: f32,
    pub survived_to_end: bool,
}

#[derive(Debug, Default)]
pub(in crate::game) struct VehicleOilCollector {
    records: BTreeMap<u32, VehicleOilRecord>,
}

impl VehicleOilCollector {
    pub(in crate::game) fn observe(&mut self, tick: u32, entities: &EntityStore) {
        for entity in entities.iter() {
            let unit_kind = match entity.kind {
                EntityKind::ScoutCar => "scout_car",
                EntityKind::CommandCar => "command_car",
                EntityKind::Tank => "tank",
                _ => continue,
            };
            let oil = entity
                .movement
                .as_ref()
                .map(|movement| movement.lifetime_oil_used)
                .unwrap_or(0.0);
            let record = self.records.entry(entity.id).or_insert(VehicleOilRecord {
                entity_id: entity.id,
                owner_id: entity.owner,
                unit_kind: unit_kind.to_string(),
                first_seen_tick: tick,
                last_seen_tick: tick,
                first_moved_tick: (oil > 0.0).then_some(tick),
                last_moved_tick: (oil > 0.0).then_some(tick),
                lifetime_oil_spend: oil,
                survived_to_end: false,
            });
            record.last_seen_tick = tick;
            if oil > record.lifetime_oil_spend {
                if record.first_moved_tick.is_none() {
                    record.first_moved_tick = Some(tick);
                }
                record.last_moved_tick = Some(tick);
                record.lifetime_oil_spend = oil;
            }
        }
    }

    pub(in crate::game) fn finish(mut self, entities: &EntityStore) -> Vec<VehicleOilRecord> {
        let survivors: BTreeSet<u32> = entities.iter().map(|entity| entity.id).collect();
        for record in self.records.values_mut() {
            record.survived_to_end = survivors.contains(&record.entity_id);
        }
        self.records.into_values().collect()
    }
}
