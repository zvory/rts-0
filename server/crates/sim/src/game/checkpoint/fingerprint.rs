//! Versioned end-state projection. Never serialize a HashMap in its native iteration order.
use std::io::{self, BufWriter, Write};

use rand::RngCore;
use serde::{ser::SerializeMap, Serialize, Serializer};
use sha2::{Digest, Sha256};

use super::*;

pub(in crate::game) const END_STATE_HASH_ALGORITHM: &str = "rts-end-state-json-sha256-v1";

impl Game {
    /// Fingerprint committed simulation state, excluding unapplied input and historical commands.
    /// Ordered JSON is streamed through a small buffer; no full checkpoint or command-log clone.
    pub fn replay_end_state_hash(&self) -> Result<String, String> {
        let mut writer = BufWriter::with_capacity(64 * 1024, HashWriter(Sha256::new()));
        writer
            .write_all(END_STATE_HASH_ALGORITHM.as_bytes())
            .map_err(|e| e.to_string())?;
        serde_json::to_writer(&mut writer, &EndState(&self.state)).map_err(|e| e.to_string())?;
        writer.flush().map_err(|e| e.to_string())?;
        Ok(format!("{:x}", writer.get_ref().0.clone().finalize()))
    }
}

struct HashWriter(Sha256);
impl Write for HashWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.update(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct EndState<'a>(&'a GameState);
impl Serialize for EndState<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let state = self.0;
        let mut out = serializer.serialize_map(None)?;
        macro_rules! field {
            ($name:literal, $value:expr) => {
                out.serialize_entry($name, &$value)?;
            };
        }
        field!("simSchemaVersion", SIM_SCHEMA_VERSION);
        field!("map", MapBindingV1::from_state(state));
        field!("seed", state.seed);
        field!("tick", state.tick);
        field!("lastWorldCombatTick", state.last_world_combat_tick);
        field!("lastWorldCombatPosition", state.last_world_combat_position);
        field!(
            "worldCombatActiveThroughTick",
            state.world_combat_active_through_tick
        );
        field!("worldCombatPosition", state.world_combat_position);
        field!("rng", RngDescriptorV1::from_rng(&state.rng));
        // SmallRng is platform-dependent. Probe its future output without consuming live draws,
        // so equal seed/draw counters alone cannot mask different generator implementations.
        let mut rng = state.rng.clone();
        let probe: Vec<u32> = (0..8).map(|_| rng.next_u32()).collect();
        field!("rngProbe", probe);
        field!("players", state.players);
        field!("startingLoadouts", state.starting_loadouts);
        field!("startingLoadout", state.starting_loadout);
        field!("nextEntityId", state.entities.checkpoint_next_id());
        let entities: BTreeMap<_, _> = state.entities.iter().map(|e| (e.id, e)).collect();
        field!("entities", entities);
        field!("fog", FogStateV1::from_fog(&state.fog));
        field!(
            "buildingMemory",
            BuildingMemoryV1::from_memory(&state.building_memory)
        );
        field!(
            "antiTankGunMemory",
            AntiTankGunMemoryV1::from_memory(&state.anti_tank_gun_memory)
        );
        field!("lingeringSight", state.lingering_sight);
        field!("firingReveals", state.firing_reveals);
        field!("smokes", state.smokes);
        field!("trenches", state.trenches);
        field!("groundDecals", state.ground_decals);
        field!("abilityRuntime", state.ability_runtime);
        field!("mortarShells", state.mortar_shells);
        field!("artilleryShells", state.artillery_shells);
        field!("panzerfaustShots", state.panzerfaust_shots);
        field!("activeConstructionSites", state.active_construction_sites);
        field!("labGodModePlayers", state.lab_god_mode_players);
        out.end()
    }
}

#[cfg(test)]
mod tests;
