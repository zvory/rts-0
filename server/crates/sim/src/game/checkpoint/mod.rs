use std::collections::{BTreeMap, BTreeSet};

use serde::{de::DeserializeOwned, Deserialize, Serialize};

use super::ability_runtime::{AbilityRuntime, MAX_ACTIVE_ABILITY_OBJECTS};
use super::artillery::ArtilleryShellStore;
use super::building_memory::{BuildingMemory, BuildingMemoryEntry};
use super::commands::PendingCommand;
use super::entity::{Entity, EntityStore};
use super::firing_reveal::FiringRevealSource;
use super::fog::{FiringRevealVisibility, Fog, LingeringSightSource};
use super::map::Map;
use super::mortar::MortarShellStore;
use super::panzerfaust_shot::PanzerfaustShotStore;
use super::replay::CommandLogEntry;
use super::setup::StartingLoadout;
use super::smoke::SmokeCloudStore;
use super::state::{GameState, TrackedRng};
use super::trench::TrenchStore;
use super::world_combat;
use super::{setup, Game, MapMetadata, PlayerStartingLoadout};

mod error;
mod metadata;
mod player_dto;
mod validation;

pub(in crate::game) use error::CheckpointPayloadError;
use metadata::{CheckpointCompatibilityV1, CommandLogMetadataV1, MapBindingV1, RngDescriptorV1};
use player_dto::PlayerStateV1;
use validation::*;

fn serde_convert<T, U>(value: T) -> Result<U, CheckpointPayloadError>
where
    T: Serialize,
    U: DeserializeOwned,
{
    serde_json::from_value(
        serde_json::to_value(value)
            .map_err(|err| CheckpointPayloadError::MalformedJson(err.to_string()))?,
    )
    .map_err(|err| CheckpointPayloadError::MalformedJson(err.to_string()))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PlayerCheckpointRef<'a, T> {
    #[serde(flatten)]
    state: &'a T,
    supply_cap: u32,
}

const CHECKPOINT_SCHEMA: &str = "rts.gameCheckpoint";
const CHECKPOINT_VERSION: u32 = 1;
// Construction funding provenance is authoritative state: restoring a scaffold without it can
// either lose a legitimate refund or mint resources for an authored scaffold. Keep older payloads
// outside this compatibility boundary instead of silently defaulting the receipt.
const SIM_SCHEMA_VERSION: u32 = 3;
const RULES_VERSION: u32 = 1;
const PROTOCOL_VERSION: u32 = 1;
const RNG_ALGORITHM: &str = "rts-small-rng-0.8-draws-v1";
const MAX_PAYLOAD_BYTES: usize = 4 * 1024 * 1024;
const MAX_RNG_DRAWS_CONSUMED: u64 = 10_000_000;
const MAX_PLAYERS: usize = 8;
const MAX_ENTITIES: usize = 2_000;
const MAX_PENDING_COMMANDS: usize = 1_024;
const MAX_COMMAND_LOG_ENTRIES: usize = 200_000;
const MAX_SMOKE_CLOUDS: usize = 256;
const MAX_TRENCHES: usize = 4_096;
const MAX_SCHEDULED_MORTAR_SHELLS: usize = 4_096;
const MAX_SCHEDULED_ARTILLERY_SHELLS: usize = 4_096;
const MAX_SCHEDULED_PANZERFAUST_SHOTS: usize = 4_096;
const MAX_COMPLETED_UPGRADES_PER_PLAYER: usize = 32;
const MAX_UNITS_PER_CHECKPOINT_COMMAND: usize = 4_096;

#[allow(dead_code)]
impl Game {
    pub(in crate::game) fn checkpoint_payload_text(
        &self,
    ) -> Result<String, CheckpointPayloadError> {
        GameCheckpointV1::from_state(&self.state)?.to_text()
    }

    pub(in crate::game) fn checkpoint_payload_text_for_container(
        &self,
        created_by: &str,
        server_build_sha: &str,
    ) -> Result<String, CheckpointPayloadError> {
        GameCheckpointV1::from_state_with_compatibility(
            &self.state,
            CheckpointCompatibilityV1::new(created_by, server_build_sha),
        )?
        .to_text()
    }

    pub(in crate::game) fn restore_checkpoint_payload_text(
        text: &str,
        map: Map,
        map_metadata: MapMetadata,
    ) -> Result<Self, CheckpointPayloadError> {
        let state = GameCheckpointV1::from_text(text)?.into_state(map, map_metadata)?;
        let derived = setup::live_derived_state(&state.map, &state.entities, state.tick);
        let mut game = Self { state, derived };
        if !game.state.lab_god_mode_players.is_empty() {
            game.sync_lab_god_mode_flags();
        }
        Ok(game)
    }

    #[cfg(test)]
    pub(in crate::game) fn checkpoint_payload_text_for_test(
        &self,
    ) -> Result<String, CheckpointPayloadError> {
        self.checkpoint_payload_text()
    }

    #[cfg(test)]
    pub(in crate::game) fn restore_checkpoint_payload_text_for_test(
        text: &str,
        map: Map,
        map_metadata: MapMetadata,
    ) -> Result<Self, CheckpointPayloadError> {
        Self::restore_checkpoint_payload_text(text, map, map_metadata)
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GameCheckpointV1 {
    schema: String,
    version: u32,
    compatibility: CheckpointCompatibilityV1,
    map_binding: MapBindingV1,
    seed: u32,
    tick: u32,
    #[serde(default)]
    last_world_combat_tick: Option<u32>,
    #[serde(default)]
    last_world_combat_position: Option<[f32; 2]>,
    #[serde(default)]
    world_combat_active_through_tick: Option<u32>,
    #[serde(default)]
    world_combat_position: Option<[f32; 2]>,
    rng: RngDescriptorV1,
    players: Vec<PlayerStateV1>,
    starting_loadouts: Vec<PlayerStartingLoadout>,
    starting_loadout: StartingLoadout,
    entities: EntityStoreV1,
    pending_commands: Vec<PendingCommand>,
    command_log: Vec<CommandLogEntry>,
    command_log_metadata: CommandLogMetadataV1,
    fog: FogStateV1,
    building_memory: BuildingMemoryV1,
    lingering_sight: Vec<LingeringSightSource>,
    firing_reveals: Vec<FiringRevealSource>,
    smokes: SmokeCloudStore,
    trenches: TrenchStore,
    ability_runtime: AbilityRuntime,
    mortar_shells: MortarShellStore,
    artillery_shells: ArtilleryShellStore,
    #[serde(default)]
    panzerfaust_shots: PanzerfaustShotStore,
    active_construction_sites: BTreeSet<u32>,
    lab_god_mode_players: BTreeSet<u32>,
}

impl GameCheckpointV1 {
    fn from_state(state: &GameState) -> Result<Self, CheckpointPayloadError> {
        Self::from_state_with_compatibility(state, CheckpointCompatibilityV1::debug_default())
    }

    fn from_state_with_compatibility(
        state: &GameState,
        compatibility: CheckpointCompatibilityV1,
    ) -> Result<Self, CheckpointPayloadError> {
        let checkpoint = Self {
            schema: CHECKPOINT_SCHEMA.to_string(),
            version: CHECKPOINT_VERSION,
            compatibility,
            map_binding: MapBindingV1::from_state(state),
            seed: state.seed,
            tick: state.tick,
            last_world_combat_tick: state.last_world_combat_tick,
            last_world_combat_position: state.last_world_combat_position,
            world_combat_active_through_tick: state.world_combat_active_through_tick,
            world_combat_position: state.world_combat_position,
            rng: RngDescriptorV1::from_rng(&state.rng),
            players: state
                .players
                .iter()
                .map(|player| {
                    serde_convert(PlayerCheckpointRef {
                        state: player,
                        supply_cap: crate::config::PLAYER_SUPPLY_CAP,
                    })
                })
                .collect::<Result<_, _>>()?,
            starting_loadouts: state.starting_loadouts.clone(),
            starting_loadout: state.starting_loadout,
            entities: EntityStoreV1::from_store(&state.entities),
            pending_commands: state.pending.clone(),
            command_log: state.command_log.clone(),
            command_log_metadata: CommandLogMetadataV1::from_command_log(&state.command_log),
            fog: FogStateV1::from_fog(&state.fog),
            building_memory: BuildingMemoryV1::from_memory(&state.building_memory),
            lingering_sight: state.lingering_sight.clone(),
            firing_reveals: state.firing_reveals.clone(),
            smokes: state.smokes.clone(),
            trenches: state.trenches.clone(),
            ability_runtime: state.ability_runtime.clone(),
            mortar_shells: state.mortar_shells.clone(),
            artillery_shells: state.artillery_shells.clone(),
            panzerfaust_shots: state.panzerfaust_shots.clone(),
            active_construction_sites: state.active_construction_sites.clone(),
            lab_god_mode_players: state.lab_god_mode_players.clone(),
        };
        checkpoint.validate_against(&state.map, &state.map_metadata)?;
        Ok(checkpoint)
    }

    fn to_text(&self) -> Result<String, CheckpointPayloadError> {
        let text = serde_json::to_string(self)
            .map_err(|err| CheckpointPayloadError::MalformedJson(err.to_string()))?;
        if text.len() > MAX_PAYLOAD_BYTES {
            return Err(CheckpointPayloadError::PayloadTooLarge {
                bytes: text.len(),
                max: MAX_PAYLOAD_BYTES,
            });
        }
        Ok(text)
    }

    fn from_text(text: &str) -> Result<Self, CheckpointPayloadError> {
        if text.len() > MAX_PAYLOAD_BYTES {
            return Err(CheckpointPayloadError::PayloadTooLarge {
                bytes: text.len(),
                max: MAX_PAYLOAD_BYTES,
            });
        }
        serde_json::from_str(text)
            .map_err(|err| CheckpointPayloadError::MalformedJson(err.to_string()))
    }

    fn into_state(
        self,
        map: Map,
        map_metadata: MapMetadata,
    ) -> Result<GameState, CheckpointPayloadError> {
        self.validate_against(&map, &map_metadata)?;
        let entities = self.entities.into_store();
        let panzerfaust_shots = self.panzerfaust_shots;
        Ok(GameState {
            map,
            entities,
            fog: self.fog.into_fog(),
            building_memory: self.building_memory.into_memory(),
            players: serde_convert(&self.players)?,
            pending: self.pending_commands,
            command_log: self.command_log,
            tick: self.tick,
            last_world_combat_tick: self.last_world_combat_tick,
            last_world_combat_position: self.last_world_combat_position,
            world_combat_active_through_tick: self.world_combat_active_through_tick,
            world_combat_position: self.world_combat_position,
            lingering_sight: self.lingering_sight,
            firing_reveals: self.firing_reveals,
            smokes: self.smokes,
            trenches: self.trenches,
            ability_runtime: self.ability_runtime,
            mortar_shells: self.mortar_shells,
            artillery_shells: self.artillery_shells,
            panzerfaust_shots,
            seed: self.seed,
            starting_loadouts: self.starting_loadouts,
            map_metadata,
            active_construction_sites: self.active_construction_sites,
            lab_god_mode_players: self.lab_god_mode_players,
            starting_loadout: self.starting_loadout,
            rng: TrackedRng::from_seed_and_draws(self.rng.seed, self.rng.draws_consumed),
        })
    }

    fn validate_against(
        &self,
        map: &Map,
        map_metadata: &MapMetadata,
    ) -> Result<(), CheckpointPayloadError> {
        if self.schema != CHECKPOINT_SCHEMA {
            return Err(CheckpointPayloadError::UnsupportedSchema {
                found: self.schema.clone(),
            });
        }
        if self.version != CHECKPOINT_VERSION {
            return Err(CheckpointPayloadError::UnsupportedVersion {
                found: self.version,
            });
        }
        self.compatibility.validate()?;
        self.rng.validate(self.seed)?;
        if self
            .last_world_combat_tick
            .is_some_and(|last| last > self.tick)
        {
            return Err(CheckpointPayloadError::InvalidValue {
                field: "lastWorldCombatTick",
            });
        }
        if !world_combat::valid_checkpoint_signal_state(
            self.last_world_combat_tick,
            self.last_world_combat_position,
            self.world_combat_active_through_tick,
            self.world_combat_position,
            map.world_size_px(),
        ) {
            return Err(CheckpointPayloadError::InvalidValue {
                field: "worldCombatActiveThroughTick",
            });
        }
        self.map_binding.validate_against(map, map_metadata)?;
        validate_supplied_map(map)?;
        validate_count("players", self.players.len(), MAX_PLAYERS)?;
        validate_count("entities", self.entities.entities.len(), MAX_ENTITIES)?;
        validate_count(
            "pendingCommands",
            self.pending_commands.len(),
            MAX_PENDING_COMMANDS,
        )?;
        validate_count(
            "commandLog",
            self.command_log.len(),
            MAX_COMMAND_LOG_ENTRIES,
        )?;
        validate_count("smokes", self.smokes.checkpoint_len(), MAX_SMOKE_CLOUDS)?;
        validate_count("trenches", self.trenches.checkpoint_len(), MAX_TRENCHES)?;
        validate_count(
            "abilityRuntime",
            self.ability_runtime.instances().count()
                + self.ability_runtime.world_objects().count()
                + self.ability_runtime.projectiles().count(),
            MAX_ACTIVE_ABILITY_OBJECTS,
        )?;
        validate_count(
            "mortarShells",
            self.mortar_shells.checkpoint_len(),
            MAX_SCHEDULED_MORTAR_SHELLS,
        )?;
        validate_count(
            "artilleryShells",
            self.artillery_shells.checkpoint_len(),
            MAX_SCHEDULED_ARTILLERY_SHELLS,
        )?;
        validate_count(
            "panzerfaustShots",
            self.panzerfaust_shots.checkpoint_len(),
            MAX_SCHEDULED_PANZERFAUST_SHOTS,
        )?;

        let player_ids = validate_players(&self.players, self.tick)?;
        let entity_ids = validate_entities(&self.entities, &player_ids, map, self.tick)?;
        validate_player_supply(&self.players, &self.entities)?;
        validate_fog(
            &self.fog,
            &player_ids,
            self.entities.next_id,
            &self.firing_reveals,
            map,
            self.tick,
        )?;
        validate_reaction_gates_against_visibility(
            &self.entities.entities,
            &entity_ids,
            &self.fog,
        )?;
        validate_building_memory(&self.building_memory, &player_ids)?;
        validate_pending_commands(&self.pending_commands, &player_ids)?;
        validate_command_log(&self.command_log, self.tick, &player_ids)?;
        self.command_log_metadata
            .validate_against(&self.command_log)?;
        validate_active_sources(
            &self.lingering_sight,
            &self.firing_reveals,
            self.tick,
            &player_ids,
            &entity_ids,
        )?;
        validate_panzerfaust_shots(
            &self.panzerfaust_shots,
            &player_ids,
            self.entities.next_id,
            map,
            self.tick,
        )?;
        validate_id_set(
            "activeConstructionSites",
            &self.active_construction_sites,
            &entity_ids,
        )?;
        validate_id_set("labGodModePlayers", &self.lab_god_mode_players, &player_ids)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct EntityStoreV1 {
    next_id: u32,
    entities: Vec<Entity>,
}

impl EntityStoreV1 {
    fn from_store(store: &EntityStore) -> Self {
        Self {
            next_id: store.checkpoint_next_id(),
            entities: store.checkpoint_entities(),
        }
    }

    fn into_store(self) -> EntityStore {
        EntityStore::from_checkpoint_entities(self.next_id, self.entities)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FogStateV1 {
    size: u32,
    grids: BTreeMap<u32, Vec<bool>>,
    firing_reveal_visibility: BTreeMap<u32, BTreeMap<u32, FiringRevealVisibility>>,
}

impl FogStateV1 {
    fn from_fog(fog: &Fog) -> Self {
        Self {
            size: fog.checkpoint_size(),
            grids: fog.checkpoint_grids(),
            firing_reveal_visibility: fog.checkpoint_firing_reveal_visibility(),
        }
    }

    fn into_fog(self) -> Fog {
        Fog::from_checkpoint_grids(self.size, self.grids, self.firing_reveal_visibility)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BuildingMemoryV1 {
    entries: Vec<BuildingMemoryEntryV1>,
}

impl BuildingMemoryV1 {
    fn from_memory(memory: &BuildingMemory) -> Self {
        Self {
            entries: memory
                .checkpoint_entries()
                .into_iter()
                .map(|(player_id, building_id, entry)| BuildingMemoryEntryV1 {
                    player_id,
                    building_id,
                    entry,
                })
                .collect(),
        }
    }

    fn into_memory(self) -> BuildingMemory {
        BuildingMemory::from_checkpoint_entries(
            self.entries
                .into_iter()
                .map(|entry| (entry.player_id, entry.building_id, entry.entry))
                .collect(),
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BuildingMemoryEntryV1 {
    player_id: u32,
    building_id: u32,
    entry: BuildingMemoryEntry,
}
