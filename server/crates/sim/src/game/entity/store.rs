use std::{
    collections::{BTreeMap, HashMap},
    hash::{BuildHasherDefault, Hasher},
};

use super::{Entity, EntityKind, GatherPhase};
use serde::{Deserialize, Serialize};

/// Hashes the server-allocated `u32` ids used only by [`EntityStore`].
///
/// The identity fast path preserves distinct low bucket bits for monotonically allocated ids. The
/// phase benchmark also checked it against an odd-multiplier mix because sequential ids share
/// high-bit control fingerprints.
///
/// The fallback byte path is total because `Hasher` supplies the other integer writers in terms of
/// it, even though this private map's `u32` keys use `write_u32` directly.
#[derive(Default)]
struct EntityIdHasher(u64);

impl Hasher for EntityIdHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.0 ^= u64::from(byte);
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }

    fn write_u32(&mut self, value: u32) {
        self.0 = u64::from(value);
    }
}

type EntityMap = HashMap<u32, Entity, BuildHasherDefault<EntityIdHasher>>;

/// The authoritative collection of all entities, keyed by stable id.
///
/// Ids increase monotonically and are never reused. All access is fallible so the tick loop
/// can freely reference ids that may have been removed (dead units, depleted state) without
/// risking a panic.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EntityStore {
    next_id: u32,
    map: EntityMap,
}

impl EntityStore {
    pub fn new() -> Self {
        EntityStore {
            // Start ids at 1 so 0 can never collide with the neutral-owner sentinel in
            // any accidental id/owner confusion, and so `0` reads as "no entity".
            next_id: 1,
            map: EntityMap::default(),
        }
    }

    /// Allocate the next stable id.
    fn alloc_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        id
    }

    /// Insert a fully-formed entity, assigning it a fresh id. Returns the new id.
    pub fn insert(&mut self, mut e: Entity) -> u32 {
        let id = self.alloc_id();
        e.id = id;
        self.map.insert(id, e);
        id
    }

    /// Spawn a unit of `kind` for `owner` at a world position, fully built and idle.
    /// Returns `None` if `kind` is not a known unit.
    pub fn spawn_unit(&mut self, owner: u32, kind: EntityKind, x: f32, y: f32) -> Option<u32> {
        let e = Entity::new_unit(owner, kind, x, y)?;
        Some(self.insert(e))
    }

    /// Spawn a building of `kind` for `owner`. The position is the building center in world
    /// pixels. If `finished` is true the building starts fully built; otherwise it begins in
    /// CONSTRUCT state with zero progress. Returns `None` if `kind` is not a known building.
    pub fn spawn_building(
        &mut self,
        owner: u32,
        kind: EntityKind,
        x: f32,
        y: f32,
        finished: bool,
    ) -> Option<u32> {
        let e = Entity::new_building(owner, kind, x, y, finished)?;
        Some(self.insert(e))
    }

    /// Spawn a neutral resource node of `kind` (`steel` | `oil`) at a world position.
    pub fn spawn_node(&mut self, kind: EntityKind, x: f32, y: f32) -> Option<u32> {
        let e = Entity::new_node(kind, x, y)?;
        Some(self.insert(e))
    }

    pub fn get(&self, id: u32) -> Option<&Entity> {
        self.map.get(&id)
    }

    pub fn get_mut(&mut self, id: u32) -> Option<&mut Entity> {
        self.map.get_mut(&id)
    }

    /// Whether an entity with this id still exists.
    pub fn contains(&self, id: u32) -> bool {
        self.map.contains_key(&id)
    }

    /// Remove an entity, returning it if present.
    pub fn remove(&mut self, id: u32) -> Option<Entity> {
        self.map.remove(&id)
    }

    /// Iterate over all entities (shared).
    pub fn iter(&self) -> impl Iterator<Item = &Entity> {
        let mut ids: Vec<u32> = self.map.keys().copied().collect();
        ids.sort_unstable();
        ids.into_iter().filter_map(|id| self.map.get(&id))
    }

    /// All currently-live entity ids in stable ascending order. Useful for index-free iteration
    /// when the body needs `&mut self` on the store.
    pub fn ids(&self) -> Vec<u32> {
        let mut ids: Vec<u32> = self.map.keys().copied().collect();
        ids.sort_unstable();
        ids
    }

    #[cfg(test)]
    pub(crate) fn next_id_for_test(&self) -> u32 {
        self.next_id
    }

    pub(in crate::game) fn checkpoint_next_id(&self) -> u32 {
        self.next_id
    }

    pub(in crate::game) fn checkpoint_entities(&self) -> Vec<Entity> {
        let mut entities = self.iter().cloned().collect::<Vec<_>>();
        entities.sort_by_key(|entity| entity.id);
        entities
    }

    pub(in crate::game) fn from_checkpoint_entities(next_id: u32, entities: Vec<Entity>) -> Self {
        let map = entities
            .into_iter()
            .map(|entity| (entity.id, entity))
            .collect::<BTreeMap<_, _>>()
            .into_iter()
            .collect();
        Self { next_id, map }
    }

    /// Whether `player` owns at least one entity (unit or building).
    pub fn player_alive(&self, player: u32) -> bool {
        self.map.values().any(|e| e.owner == player)
    }

    /// Whichever gatherer currently holds `node_id`'s single harvest slot, if any.
    ///
    /// A reservation is only authoritative while the recorded gatherer is alive, is still
    /// gathering this exact node, and is in the `Harvesting` phase. Stale ids are ignored so
    /// command handling and economy progression agree on when a slot is actually occupied.
    pub fn node_slot_holder(&self, node_id: u32) -> Option<u32> {
        let miner_id = self.get(node_id).and_then(|n| n.miner())?;
        if self.worker_holds_node_slot(miner_id, node_id) {
            Some(miner_id)
        } else {
            None
        }
    }

    /// Claim `node_id`'s harvest slot for `worker_id` if the gatherer is in the authoritative
    /// slot-holding state and no other valid gatherer already holds it.
    pub fn claim_miner(&mut self, node_id: u32, worker_id: u32) -> bool {
        if matches!(self.node_slot_holder(node_id), Some(holder) if holder != worker_id) {
            return false;
        }
        if !self.worker_holds_node_slot(worker_id, node_id) {
            return false;
        }
        let Some(node) = self.get_mut(node_id).and_then(|n| n.resource_node.as_mut()) else {
            return false;
        };
        node.miner = Some(worker_id);
        true
    }

    /// Clear any stale node `miner` fields that no longer point at a valid slot holder.
    pub fn clear_stale_miner_slots(&mut self) {
        let stale_nodes: Vec<u32> = self
            .iter()
            .filter(|e| e.miner().is_some() && self.node_slot_holder(e.id).is_none())
            .map(|e| e.id)
            .collect();
        for node_id in stale_nodes {
            if let Some(node) = self.get_mut(node_id).and_then(|n| n.resource_node.as_mut()) {
                node.miner = None;
            }
        }
    }

    fn worker_holds_node_slot(&self, worker_id: u32, node_id: u32) -> bool {
        let Some(worker) = self.get(worker_id) else {
            return false;
        };
        worker.hp > 0
            && matches!(worker.kind, EntityKind::Worker | EntityKind::Golem)
            && worker.order().gather_node() == Some(node_id)
            && worker.gather_phase() == Some(GatherPhase::Harvesting)
    }

    /// Clear every node reservation pointing to this gatherer, even if the gatherer's order has
    /// already changed or the gatherer has already been removed.
    pub fn release_miner(&mut self, worker_id: u32) {
        // Order-independent: every matching resource node receives the same idempotent clear.
        for entity in self.map.values_mut() {
            if let Some(node) = entity.resource_node.as_mut() {
                if node.miner == Some(worker_id) {
                    node.miner = None;
                }
            }
        }
    }
}
