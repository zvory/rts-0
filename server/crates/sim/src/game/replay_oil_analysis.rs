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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collector_retains_dead_vehicles_and_all_fuel_kinds() {
        let mut entities = EntityStore::new();
        let scout = entities
            .spawn_unit(1, EntityKind::ScoutCar, 64.0, 64.0)
            .expect("scout car");
        let command = entities
            .spawn_unit(1, EntityKind::CommandCar, 96.0, 64.0)
            .expect("command car");
        let tank = entities
            .spawn_unit(2, EntityKind::Tank, 128.0, 64.0)
            .expect("tank");
        entities
            .spawn_unit(2, EntityKind::Rifleman, 160.0, 64.0)
            .expect("rifleman");
        for (id, oil) in [(scout, 1.25), (command, 2.5), (tank, 7.75)] {
            entities
                .get_mut(id)
                .and_then(|entity| entity.movement.as_mut())
                .expect("vehicle movement")
                .lifetime_oil_used = oil;
        }

        let mut collector = VehicleOilCollector::default();
        collector.observe(20, &entities);
        entities.remove(command);
        collector.observe(21, &entities);
        let records = collector.finish(&entities);

        assert_eq!(records.len(), 3);
        assert_eq!(records[0].unit_kind, "scout_car");
        assert_eq!(records[1].unit_kind, "command_car");
        assert_eq!(records[2].unit_kind, "tank");
        assert!(records[0].survived_to_end);
        assert!(!records[1].survived_to_end);
        assert!(records[2].survived_to_end);
        assert_eq!(records[1].last_seen_tick, 20);
        assert_eq!(records[1].lifetime_oil_spend, 2.5);
    }
}
