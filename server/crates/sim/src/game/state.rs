use std::collections::BTreeSet;

use rand::{rngs::SmallRng, Error as RngError, RngCore, SeedableRng};

use super::firing_reveal::FiringRevealSource;
// The architecture baseline keys direct PlayerState imports by their source line.
// Keep this long-standing import group stable across rustfmt versions.
#[rustfmt::skip]
use super::{
    ability_runtime::AbilityRuntime, artillery::ArtilleryShellStore, building_memory::BuildingMemory,
    commands, fog::Fog, fog::LingeringSightSource, map::Map, mortar::MortarShellStore,
    replay::CommandLogEntry, setup::StartingLoadout, smoke::SmokeCloudStore,
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
    pub(in crate::game) last_world_combat_tick: Option<u32>,
    pub(in crate::game) last_world_combat_position: Option<[f32; 2]>,
    pub(in crate::game) world_combat_active_through_tick: Option<u32>,
    pub(in crate::game) world_combat_position: Option<[f32; 2]>,
    pub(in crate::game) lingering_sight: Vec<LingeringSightSource>,
    pub(in crate::game) firing_reveals: Vec<FiringRevealSource>,
    pub(in crate::game) smokes: SmokeCloudStore,
    pub(in crate::game) trenches: TrenchStore,
    pub(in crate::game) ability_runtime: AbilityRuntime,
    pub(in crate::game) mortar_shells: MortarShellStore,
    pub(in crate::game) artillery_shells: ArtilleryShellStore,
    pub(in crate::game) panzerfaust_shots: super::panzerfaust_shot::PanzerfaustShotStore,
    pub(in crate::game) seed: u32,
    pub(in crate::game) starting_loadouts: Vec<PlayerStartingLoadout>,
    pub(in crate::game) map_metadata: MapMetadata,
    pub(in crate::game) active_construction_sites: BTreeSet<u32>,
    pub(in crate::game) lab_god_mode_players: BTreeSet<u32>,
    pub(in crate::game) starting_loadout: StartingLoadout,
    pub(in crate::game) rng: TrackedRng,
}

#[derive(Clone)]
pub(in crate::game) struct TrackedRng {
    seed: u64,
    draws_consumed: u64,
    inner: SmallRng,
}

impl TrackedRng {
    pub(in crate::game) fn seed_from_match_seed(seed: u32) -> Self {
        Self::from_seed_and_draws(seed as u64, 0)
    }

    pub(in crate::game) fn from_seed_and_draws(seed: u64, draws_consumed: u64) -> Self {
        let mut rng = Self {
            seed,
            draws_consumed: 0,
            inner: SmallRng::seed_from_u64(seed),
        };
        for _ in 0..draws_consumed {
            let _ = rng.next_u32();
        }
        rng
    }

    pub(in crate::game) fn seed(&self) -> u64 {
        self.seed
    }

    pub(in crate::game) fn draws_consumed(&self) -> u64 {
        self.draws_consumed
    }
}

impl RngCore for TrackedRng {
    fn next_u32(&mut self) -> u32 {
        self.draws_consumed = self.draws_consumed.saturating_add(1);
        self.inner.next_u32()
    }

    fn next_u64(&mut self) -> u64 {
        let lo = self.next_u32() as u64;
        let hi = self.next_u32() as u64;
        (hi << 32) | lo
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        let mut offset = 0;
        while offset < dest.len() {
            let bytes = self.next_u32().to_le_bytes();
            let remaining = dest.len() - offset;
            let count = remaining.min(bytes.len());
            dest[offset..offset + count].copy_from_slice(&bytes[..count]);
            offset += count;
        }
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), RngError> {
        self.fill_bytes(dest);
        Ok(())
    }
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
            last_world_combat_tick: None,
            last_world_combat_position: None,
            world_combat_active_through_tick: None,
            world_combat_position: None,
            lingering_sight: Vec::new(),
            firing_reveals: Vec::new(),
            smokes: SmokeCloudStore::new(),
            trenches: TrenchStore::new(),
            ability_runtime: AbilityRuntime::new(),
            mortar_shells: MortarShellStore::default(),
            artillery_shells: ArtilleryShellStore::default(),
            panzerfaust_shots: super::panzerfaust_shot::PanzerfaustShotStore::default(),
            seed,
            starting_loadouts,
            map_metadata,
            active_construction_sites: BTreeSet::new(),
            lab_god_mode_players: BTreeSet::new(),
            starting_loadout,
            rng: TrackedRng::seed_from_match_seed(seed),
        }
    }

    pub(in crate::game) fn player_ids(&self) -> Vec<u32> {
        self.players.iter().map(|player| player.id).collect()
    }
}
