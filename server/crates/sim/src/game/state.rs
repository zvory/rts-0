use std::collections::BTreeSet;

use rand::rngs::SmallRng;

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
