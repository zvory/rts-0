use std::collections::HashSet;

use super::replay::SelfPlayFailure;
use crate::config;
use rts_sim::protocol::{kinds, MapInfo, Snapshot};

const RESOURCE_SANITY_LIMIT: u32 = 1_000_000;

pub(super) fn validate_snapshot(
    player_id: u32,
    map: &MapInfo,
    snapshot: &Snapshot,
) -> Result<(), SelfPlayFailure> {
    if snapshot.supply_cap != config::PLAYER_SUPPLY_CAP {
        return Err(SelfPlayFailure::new(format!(
            "player {player_id} has invalid intrinsic supply cap: {}",
            snapshot.supply_cap
        )));
    }
    if snapshot.steel > RESOURCE_SANITY_LIMIT || snapshot.oil > RESOURCE_SANITY_LIMIT {
        return Err(SelfPlayFailure::new(format!(
            "player {player_id} resources look invalid: steel={} oil={}",
            snapshot.steel, snapshot.oil
        )));
    }

    let mut ids = HashSet::new();
    let world_width = map.width as f32 * map.tile_size as f32;
    let world_height = map.height as f32 * map.tile_size as f32;
    for entity in &snapshot.entities {
        if !ids.insert(entity.id) {
            return Err(SelfPlayFailure::new(format!(
                "player {player_id} snapshot has duplicate entity id {}",
                entity.id
            )));
        }
        if !known_kind(&entity.kind) {
            return Err(SelfPlayFailure::new(format!(
                "player {player_id} saw unknown entity kind {}",
                entity.kind
            )));
        }
        if entity.hp > entity.max_hp {
            return Err(SelfPlayFailure::new(format!(
                "player {player_id} saw entity {} with hp {}/{}",
                entity.id, entity.hp, entity.max_hp
            )));
        }
        if !entity.x.is_finite()
            || !entity.y.is_finite()
            || entity.x < 0.0
            || entity.y < 0.0
            || entity.x >= world_width
            || entity.y >= world_height
        {
            return Err(SelfPlayFailure::new(format!(
                "player {player_id} saw entity {} out of bounds at {},{}",
                entity.id, entity.x, entity.y
            )));
        }
        if let Some(progress) = entity.prod_progress {
            if !(0.0..=1.0).contains(&progress) || !progress.is_finite() {
                return Err(SelfPlayFailure::new(format!(
                    "player {player_id} saw invalid production progress {progress}"
                )));
            }
        }
        if let Some(progress) = entity.build_progress {
            if !(0.0..=1.0).contains(&progress) || !progress.is_finite() {
                return Err(SelfPlayFailure::new(format!(
                    "player {player_id} saw invalid build progress {progress}"
                )));
            }
        }
    }

    Ok(())
}

fn known_kind(kind: &str) -> bool {
    matches!(
        kind,
        kinds::WORKER
            | kinds::RIFLEMAN
            | kinds::MACHINE_GUNNER
            | kinds::ANTI_TANK_GUN
            | kinds::MORTAR_TEAM
            | kinds::ARTILLERY
            | kinds::SCOUT_CAR
            | kinds::TANK
            | kinds::COMMAND_CAR
            | kinds::RESOURCE_DEPOT
            | kinds::DEPOT
            | kinds::BARRACKS
            | kinds::TRAINING_CENTRE
            | kinds::ENGINEERING_COMPLEX
            | kinds::FACTORY
            | kinds::STEELWORKS
            | kinds::PUMP_JACK
            | kinds::STEEL
            | kinds::OIL
    )
}

#[cfg(test)]
mod tests {
    use super::{known_kind, validate_snapshot};
    use crate::{config, selfplay::replay::SelfPlayFailure};
    use rts_sim::protocol::{kinds, EntityView, MapInfo, Snapshot, SnapshotNetStatus};

    #[test]
    fn pump_jack_is_a_known_snapshot_entity() {
        assert!(known_kind(kinds::PUMP_JACK));
    }

    fn validate_entity_on_map(map: MapInfo, x: f32, y: f32) -> Result<(), SelfPlayFailure> {
        let snapshot = Snapshot {
            tick: 1,
            ground_decal_revision: 0,
            ground_decal_delta: None,
            world_combat_position: None,
            steel: 0,
            oil: 0,
            supply_used: 0,
            supply_cap: config::PLAYER_SUPPLY_CAP,
            auto_build: None,
            entities: vec![EntityView::new(1, 1, kinds::WORKER, x, y, 40, 40, "idle")],
            resource_deltas: Vec::new(),
            smokes: Vec::new(),
            ability_objects: Vec::new(),
            trenches: Vec::new(),
            visible_tiles: Vec::new(),
            explored_tiles: Vec::new(),
            remembered_buildings: Vec::new(),
            remembered_anti_tank_guns: Vec::new(),
            events: Vec::new(),
            upgrades: Vec::new(),
            player_resources: Vec::new(),
            net_status: SnapshotNetStatus::default(),
        };
        validate_snapshot(1, &map, &snapshot)
    }

    #[test]
    fn snapshot_bounds_use_rectangular_map_axes() {
        let wide = MapInfo {
            width: 8,
            height: 4,
            tile_size: 32,
            terrain: vec![0; 8 * 4],
            resources: Vec::new(),
            doodads: Vec::new(),
            concealment_tiles: Vec::new(),
            no_vehicle_tiles: Vec::new(),
            damage_reduction_tiles: Vec::new(),
            slow_movement_tiles: Vec::new(),
        };
        assert!(validate_entity_on_map(wide.clone(), 255.0, 127.0).is_ok());
        assert!(validate_entity_on_map(wide, 10.0, 128.0).is_err());

        let tall = MapInfo {
            width: 4,
            height: 8,
            tile_size: 32,
            terrain: vec![0; 4 * 8],
            resources: Vec::new(),
            doodads: Vec::new(),
            concealment_tiles: Vec::new(),
            no_vehicle_tiles: Vec::new(),
            damage_reduction_tiles: Vec::new(),
            slow_movement_tiles: Vec::new(),
        };
        assert!(validate_entity_on_map(tall.clone(), 10.0, 200.0).is_ok());
        assert!(validate_entity_on_map(tall, 128.0, 10.0).is_err());
    }
}
