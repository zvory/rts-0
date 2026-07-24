use std::collections::HashMap;

use crate::game::entity::{Entity, EntityKind, EntityStore, NEUTRAL};
use crate::game::fog::Fog;
use crate::game::map::Map;
use crate::game::services::occupancy::building_footprint;
use crate::game::smoke::SmokeCloudStore;
use crate::game::teams::TeamRelations;
use crate::rules::projection;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct BuildingMemoryEntry {
    pub(crate) id: u32,
    pub(crate) owner: u32,
    pub(crate) kind: EntityKind,
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) footprint: Vec<(u32, u32)>,
    pub(crate) hp: u32,
    pub(crate) max_hp: u32,
    pub(crate) build_progress: Option<f32>,
    pub(crate) under_construction: bool,
    pub(crate) observed_tick: u32,
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub(crate) struct BuildingMemory {
    entries: HashMap<(u32, u32), BuildingMemoryEntry>,
}

impl BuildingMemory {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn refresh(
        &mut self,
        player_ids: &[u32],
        entities: &EntityStore,
        fog: &Fog,
        map: &Map,
        smokes: &SmokeCloudStore,
        teams: &TeamRelations,
        tick: u32,
    ) {
        for &player_id in player_ids {
            self.remove_ineligible_or_scouted_destroyed(player_id, entities, fog, teams);
            for entity in entities.iter() {
                if !visible_enemy_building(player_id, entity, fog, smokes, teams) {
                    continue;
                }
                self.entries
                    .insert((player_id, entity.id), entry_from_entity(entity, map, tick));
            }
        }
    }

    fn remove_ineligible_or_scouted_destroyed(
        &mut self,
        player_id: u32,
        entities: &EntityStore,
        fog: &Fog,
        teams: &TeamRelations,
    ) {
        self.entries.retain(|(entry_player, entity_id), entry| {
            if *entry_player != player_id {
                return true;
            }
            if let Some(entity) = entities.get(*entity_id) {
                return enemy_building_memory_eligible(player_id, entity, teams);
            }
            !entry.footprint.iter().any(|&(tx, ty)| {
                teams
                    .same_team_player_ids(player_id)
                    .into_iter()
                    .any(|team_player| fog.is_visible(team_player, tx, ty))
            })
        });
    }

    pub(crate) fn get(&self, player_id: u32, building_id: u32) -> Option<&BuildingMemoryEntry> {
        self.entries.get(&(player_id, building_id))
    }

    pub(crate) fn entries_for_player(
        &self,
        player_id: u32,
    ) -> impl Iterator<Item = &BuildingMemoryEntry> {
        self.entries
            .iter()
            .filter_map(move |(&(entry_player, _), entry)| {
                (entry_player == player_id).then_some(entry)
            })
    }

    pub(in crate::game) fn from_checkpoint_entries(
        entries: Vec<(u32, u32, BuildingMemoryEntry)>,
    ) -> Self {
        BuildingMemory {
            entries: entries
                .into_iter()
                .map(|(player_id, building_id, entry)| ((player_id, building_id), entry))
                .collect(),
        }
    }

    pub(in crate::game) fn checkpoint_entries(&self) -> Vec<(u32, u32, BuildingMemoryEntry)> {
        let mut entries = self
            .entries
            .iter()
            .map(|(&(player_id, building_id), entry)| (player_id, building_id, entry.clone()))
            .collect::<Vec<_>>();
        entries.sort_by_key(|(player_id, building_id, _)| (*player_id, *building_id));
        entries
    }

    #[cfg(test)]
    pub(crate) fn entries_for_player_for_test(
        &self,
        player_id: u32,
    ) -> impl Iterator<Item = &BuildingMemoryEntry> {
        self.entries_for_player(player_id)
    }
}

fn visible_enemy_building(
    player_id: u32,
    entity: &Entity,
    fog: &Fog,
    smokes: &SmokeCloudStore,
    teams: &TeamRelations,
) -> bool {
    enemy_building_memory_eligible(player_id, entity, teams)
        && teams
            .same_team_player_ids(player_id)
            .into_iter()
            .any(|team_player| {
                projection::entity_visible_to_with_smoke(team_player, entity, fog, smokes)
            })
}

fn enemy_building_memory_eligible(
    player_id: u32,
    entity: &Entity,
    teams: &TeamRelations,
) -> bool {
    !teams.same_team_or_same_owner(player_id, entity.owner)
        && entity.owner != NEUTRAL
        && entity.is_building()
}

fn entry_from_entity(entity: &Entity, map: &Map, observed_tick: u32) -> BuildingMemoryEntry {
    BuildingMemoryEntry {
        id: entity.id,
        owner: entity.owner,
        kind: entity.kind,
        x: entity.pos_x,
        y: entity.pos_y,
        footprint: building_footprint(map, entity),
        hp: entity.hp,
        max_hp: entity.max_hp,
        build_progress: entity.build_progress_fraction(),
        under_construction: entity.under_construction(),
        observed_tick,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config;
    use crate::game::fog::Fog;
    use crate::game::map::Map;
    use crate::game::smoke::SmokeCloudStore;
    use crate::protocol::terrain;

    const PLAYERS: [u32; 2] = [1, 2];

    fn flat_map(size: u32) -> Map {
        Map {
            size,
            terrain: vec![terrain::GRASS; (size * size) as usize],
            starts: Vec::new(),
            base_sites: Vec::new(),
        }
    }

    fn refresh(
        memory: &mut BuildingMemory,
        entities: &EntityStore,
        fog: &mut Fog,
        map: &Map,
        smokes: &SmokeCloudStore,
        tick: u32,
    ) {
        fog.recompute_with_smoke(&PLAYERS, entities, map, smokes);
        let teams = TeamRelations::from_player_teams([(1, 1), (2, 2)]);
        memory.refresh(&PLAYERS, entities, fog, map, smokes, &teams, tick);
    }

    #[test]
    fn records_visible_enemy_building_state() {
        let map = flat_map(64);
        let mut entities = EntityStore::new();
        let mut fog = Fog::new(map.size);
        let smokes = SmokeCloudStore::new();
        let mut memory = BuildingMemory::default();
        let scout_pos = map.tile_center(8, 8);
        let depot_pos = map.tile_center(10, 8);
        entities
            .spawn_unit(1, EntityKind::Rifleman, scout_pos.0, scout_pos.1)
            .expect("scout should spawn");
        let depot = entities
            .spawn_building(2, EntityKind::Depot, depot_pos.0, depot_pos.1, false)
            .expect("depot should spawn");

        refresh(&mut memory, &entities, &mut fog, &map, &smokes, 7);

        let entry = memory
            .get(1, depot)
            .expect("visible enemy building should be remembered");
        assert_eq!(entry.id, depot);
        assert_eq!(entry.owner, 2);
        assert_eq!(entry.kind, EntityKind::Depot);
        assert_eq!((entry.x, entry.y), depot_pos);
        assert_eq!(entry.hp, entities.get(depot).expect("depot exists").hp);
        assert_eq!(
            entry.max_hp,
            config::building_stats(EntityKind::Depot).unwrap().hp
        );
        assert_eq!(entry.build_progress, Some(0.0));
        assert!(entry.under_construction);
        assert_eq!(entry.observed_tick, 7);
        assert!(!entry.footprint.is_empty());
        assert!(
            memory
                .entries_for_player_for_test(2)
                .all(|entry| entry.id != depot),
            "owners do not record their own buildings as enemy memory"
        );
    }

    #[test]
    fn does_not_record_never_scouted_enemy_building() {
        let map = flat_map(64);
        let mut entities = EntityStore::new();
        let mut fog = Fog::new(map.size);
        let smokes = SmokeCloudStore::new();
        let mut memory = BuildingMemory::default();
        let scout_pos = map.tile_center(4, 4);
        let depot_pos = map.tile_center(40, 40);
        entities
            .spawn_unit(1, EntityKind::Rifleman, scout_pos.0, scout_pos.1)
            .expect("scout should spawn");
        let depot = entities
            .spawn_building(2, EntityKind::Depot, depot_pos.0, depot_pos.1, true)
            .expect("depot should spawn");

        refresh(&mut memory, &entities, &mut fog, &map, &smokes, 1);

        assert!(memory.get(1, depot).is_none());
    }

    #[test]
    fn forgets_scouted_tank_trap_scaffold_when_it_becomes_neutral() {
        let map = flat_map(64);
        let mut entities = EntityStore::new();
        let mut fog = Fog::new(map.size);
        let smokes = SmokeCloudStore::new();
        let mut memory = BuildingMemory::default();
        let scout_pos = map.tile_center(8, 8);
        let trap_pos = map.tile_center(10, 8);
        entities
            .spawn_unit(1, EntityKind::Rifleman, scout_pos.0, scout_pos.1)
            .expect("scout should spawn");
        let trap_id = entities
            .spawn_building(2, EntityKind::TankTrap, trap_pos.0, trap_pos.1, false)
            .expect("Tank Trap scaffold should spawn");

        refresh(&mut memory, &entities, &mut fog, &map, &smokes, 1);
        assert!(memory.get(1, trap_id).is_some());

        let trap = entities.get_mut(trap_id).expect("Tank Trap should exist");
        let total = trap
            .construction
            .as_ref()
            .expect("Tank Trap should be under construction")
            .total;
        assert!(trap.set_construction_progress(total.saturating_sub(1)));
        assert_eq!(trap.advance_construction(), Some(true));
        refresh(&mut memory, &entities, &mut fog, &map, &smokes, 2);

        assert!(
            memory.get(1, trap_id).is_none(),
            "completed neutral Tank Traps must not retain player-owned building memory"
        );
    }

    #[test]
    fn keeps_hidden_destroyed_building_until_location_is_scouted() {
        let map = flat_map(64);
        let mut entities = EntityStore::new();
        let mut fog = Fog::new(map.size);
        let smokes = SmokeCloudStore::new();
        let mut memory = BuildingMemory::default();
        let scout_pos = map.tile_center(8, 8);
        let depot_pos = map.tile_center(10, 8);
        let scout = entities
            .spawn_unit(1, EntityKind::Rifleman, scout_pos.0, scout_pos.1)
            .expect("scout should spawn");
        let depot = entities
            .spawn_building(2, EntityKind::Depot, depot_pos.0, depot_pos.1, true)
            .expect("depot should spawn");
        refresh(&mut memory, &entities, &mut fog, &map, &smokes, 1);
        assert!(memory.get(1, depot).is_some());

        entities.remove(scout);
        let far = map.tile_center(40, 40);
        entities
            .spawn_unit(1, EntityKind::Rifleman, far.0, far.1)
            .expect("far scout should spawn");
        entities.remove(depot);
        refresh(&mut memory, &entities, &mut fog, &map, &smokes, 2);
        assert!(
            memory.get(1, depot).is_some(),
            "hidden destruction remains stale"
        );

        let scout_pos = map.tile_center(10, 8);
        entities
            .spawn_unit(1, EntityKind::Rifleman, scout_pos.0, scout_pos.1)
            .expect("new scout should spawn");
        refresh(&mut memory, &entities, &mut fog, &map, &smokes, 3);
        assert!(
            memory.get(1, depot).is_none(),
            "scouting the remembered footprint clears destroyed building memory"
        );
    }

    #[test]
    fn does_not_refresh_enemy_building_inside_smoke() {
        let map = flat_map(64);
        let mut entities = EntityStore::new();
        let mut fog = Fog::new(map.size);
        let mut smokes = SmokeCloudStore::new();
        let mut memory = BuildingMemory::default();
        let scout_pos = map.tile_center(8, 8);
        let depot_pos = map.tile_center(10, 8);
        entities
            .spawn_unit(1, EntityKind::Rifleman, scout_pos.0, scout_pos.1)
            .expect("scout should spawn");
        let depot = entities
            .spawn_building(2, EntityKind::Depot, depot_pos.0, depot_pos.1, true)
            .expect("depot should spawn");
        refresh(&mut memory, &entities, &mut fog, &map, &smokes, 1);

        smokes
            .spawn(
                depot_pos.0,
                depot_pos.1,
                config::SMOKE_CLOUD_RADIUS_TILES,
                config::SMOKE_CLOUD_DURATION_TICKS,
                2,
            )
            .expect("smoke should spawn");
        let remembered_hp = memory.get(1, depot).unwrap().hp;
        entities
            .get_mut(depot)
            .expect("depot exists")
            .apply_damage(1, Some((1, scout_pos, 2)));
        refresh(&mut memory, &entities, &mut fog, &map, &smokes, 2);

        assert_eq!(
            memory.get(1, depot).unwrap().hp,
            remembered_hp,
            "smoke-covered buildings should not refresh stale memory"
        );
    }
}
