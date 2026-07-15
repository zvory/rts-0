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
    if snapshot.supply_cap > config::PLAYER_SUPPLY_CAP {
        return Err(SelfPlayFailure::new(format!(
            "player {player_id} exceeded supply cap max: {}",
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
    let world = map.width as f32 * map.tile_size as f32;
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
            || entity.x >= world
            || entity.y >= world
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
            | kinds::CITY_CENTRE
            | kinds::DEPOT
            | kinds::BARRACKS
            | kinds::TRAINING_CENTRE
            | kinds::RESEARCH_COMPLEX
            | kinds::FACTORY
            | kinds::STEELWORKS
            | kinds::STEEL
            | kinds::OIL
    )
}
