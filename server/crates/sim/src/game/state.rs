use std::collections::BTreeSet;

use rand::rngs::SmallRng;
use rand::SeedableRng;

use super::{
    ability_runtime::AbilityRuntime, artillery::ArtilleryShellStore, building_memory::BuildingMemory,
    commands, firing_reveal::FiringRevealSource, fog::Fog, fog::LingeringSightSource, map::Map,
    mortar::MortarShellStore, replay::CommandLogEntry, setup::StartingLoadout, smoke::SmokeCloudStore,
    trench::TrenchStore, EntityStore, MapMetadata, PlayerStartingLoadout, PlayerState,
};

#[derive(Clone)]
pub(in crate::game) struct GameState {
    pub(in crate::game) map: Map,
    pub(in crate::game) entities: EntityStore,
    pub(in crate::game) fog: Fog,
    pub(in crate::game) building_memory: BuildingMemory,
    pub(in crate::game) players: Vec<PlayerState>,
    pub(in crate::game) pending: Vec<commands::PendingCommand>,
    pub(in crate::game) command_log: Vec<CommandLogEntry>,
    pub(in crate::game) tick: u32,
    pub(in crate::game) lingering_sight: Vec<LingeringSightSource>,
    pub(in crate::game) firing_reveals: Vec<FiringRevealSource>,
    pub(in crate::game) smokes: SmokeCloudStore,
    pub(in crate::game) trenches: TrenchStore,
    pub(in crate::game) ability_runtime: AbilityRuntime,
    pub(in crate::game) mortar_shells: MortarShellStore,
    pub(in crate::game) artillery_shells: ArtilleryShellStore,
    pub(in crate::game) seed: u32,
    pub(in crate::game) starting_loadouts: Vec<PlayerStartingLoadout>,
    pub(in crate::game) map_metadata: MapMetadata,
    pub(in crate::game) active_construction_sites: BTreeSet<u32>,
    pub(in crate::game) lab_god_mode_players: BTreeSet<u32>,
    pub(in crate::game) starting_loadout: StartingLoadout,
    pub(in crate::game) rng: SmallRng,
}

#[cfg(test)]
pub(in crate::game) struct GameCheckpoint {
    map: Map,
    entities: EntityStore,
    fog: Fog,
    building_memory: BuildingMemory,
    players: Vec<PlayerState>,
    pending: Vec<commands::PendingCommand>,
    command_log: Vec<CommandLogEntry>,
    tick: u32,
    lingering_sight: Vec<LingeringSightSource>,
    firing_reveals: Vec<FiringRevealSource>,
    smokes: SmokeCloudStore,
    trenches: TrenchStore,
    ability_runtime: AbilityRuntime,
    mortar_shells: MortarShellStore,
    artillery_shells: ArtilleryShellStore,
    seed: u32,
    starting_loadouts: Vec<PlayerStartingLoadout>,
    map_metadata: MapMetadata,
    active_construction_sites: BTreeSet<u32>,
    lab_god_mode_players: BTreeSet<u32>,
    starting_loadout: StartingLoadout,
    rng: SmallRng,
}

impl GameState {
    #[allow(clippy::too_many_arguments)]
    pub(in crate::game) fn new(
        map: Map,
        entities: EntityStore,
        fog: Fog,
        players: Vec<PlayerState>,
        seed: u32,
        starting_loadouts: Vec<PlayerStartingLoadout>,
        map_metadata: MapMetadata,
        starting_loadout: StartingLoadout,
    ) -> Self {
        Self {
            map,
            entities,
            fog,
            building_memory: BuildingMemory::default(),
            players,
            pending: Vec::new(),
            command_log: Vec::new(),
            tick: 0,
            lingering_sight: Vec::new(),
            firing_reveals: Vec::new(),
            smokes: SmokeCloudStore::new(),
            trenches: TrenchStore::new(),
            ability_runtime: AbilityRuntime::new(),
            mortar_shells: MortarShellStore::default(),
            artillery_shells: ArtilleryShellStore::default(),
            seed,
            starting_loadouts,
            map_metadata,
            active_construction_sites: BTreeSet::new(),
            lab_god_mode_players: BTreeSet::new(),
            starting_loadout,
            rng: SmallRng::seed_from_u64(seed as u64),
        }
    }

    pub(in crate::game) fn player_ids(&self) -> Vec<u32> {
        self.players.iter().map(|player| player.id).collect()
    }

    #[cfg(test)]
    pub(in crate::game) fn export_checkpoint(&self) -> GameCheckpoint {
        GameCheckpoint {
            map: self.map.clone(),
            entities: self.entities.clone(),
            fog: self.fog.clone(),
            building_memory: self.building_memory.clone(),
            players: self.players.clone(),
            pending: self.pending.clone(),
            command_log: self.command_log.clone(),
            tick: self.tick,
            lingering_sight: self.lingering_sight.clone(),
            firing_reveals: self.firing_reveals.clone(),
            smokes: self.smokes.clone(),
            trenches: self.trenches.clone(),
            ability_runtime: self.ability_runtime.clone(),
            mortar_shells: self.mortar_shells.clone(),
            artillery_shells: self.artillery_shells.clone(),
            seed: self.seed,
            starting_loadouts: self.starting_loadouts.clone(),
            map_metadata: self.map_metadata.clone(),
            active_construction_sites: self.active_construction_sites.clone(),
            lab_god_mode_players: self.lab_god_mode_players.clone(),
            starting_loadout: self.starting_loadout,
            rng: self.rng.clone(),
        }
    }

    #[cfg(test)]
    pub(in crate::game) fn import_checkpoint(checkpoint: GameCheckpoint) -> Self {
        Self {
            map: checkpoint.map,
            entities: checkpoint.entities,
            fog: checkpoint.fog,
            building_memory: checkpoint.building_memory,
            players: checkpoint.players,
            pending: checkpoint.pending,
            command_log: checkpoint.command_log,
            tick: checkpoint.tick,
            lingering_sight: checkpoint.lingering_sight,
            firing_reveals: checkpoint.firing_reveals,
            smokes: checkpoint.smokes,
            trenches: checkpoint.trenches,
            ability_runtime: checkpoint.ability_runtime,
            mortar_shells: checkpoint.mortar_shells,
            artillery_shells: checkpoint.artillery_shells,
            seed: checkpoint.seed,
            starting_loadouts: checkpoint.starting_loadouts,
            map_metadata: checkpoint.map_metadata,
            active_construction_sites: checkpoint.active_construction_sites,
            lab_god_mode_players: checkpoint.lab_god_mode_players,
            starting_loadout: checkpoint.starting_loadout,
            rng: checkpoint.rng,
        }
    }
}
