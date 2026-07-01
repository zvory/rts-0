#![allow(dead_code)]

use crate::config;
use crate::game::ability::AbilityKind;
use crate::game::ability_projectile::{AbilityProjectile, AbilityProjectileSpec};
use crate::game::entity::EntityStore;
use crate::game::services::spatial::SpatialIndex;
use crate::game::teams::TeamRelations;
use serde::{Deserialize, Serialize};

pub(in crate::game) const MAX_ACTIVE_ABILITY_OBJECTS: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct AbilityRuntimeObjectId(u32);

impl AbilityRuntimeObjectId {
    pub(crate) fn get(self) -> u32 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[allow(dead_code)]
pub(in crate::game) enum ActiveAbilityInstanceKind {
    DashReturn,
    MagicAnchor,
    LineProjectile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[allow(dead_code)]
pub(in crate::game) enum AbilityWorldObjectKind {
    ReturnMarker,
    MagicAnchor,
    LineProjectile,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub(crate) enum AbilityObjectPayload {
    #[default]
    None,
    DashReturn {
        earliest_return_tick: u32,
    },
    MagicAnchor {
        radius: f32,
    },
    LineProjectile {
        distance_traveled: f32,
        ticks_out: u32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub(crate) struct ActiveAbilityInstance {
    pub(in crate::game) id: AbilityRuntimeObjectId,
    pub(in crate::game) owner: u32,
    pub(in crate::game) caster_id: u32,
    pub(in crate::game) ability: AbilityKind,
    pub(in crate::game) kind: ActiveAbilityInstanceKind,
    pub(in crate::game) created_tick: u32,
    pub(in crate::game) expires_tick: u32,
    pub(in crate::game) payload: AbilityObjectPayload,
}

impl ActiveAbilityInstance {
    fn active_at(self, tick: u32) -> bool {
        self.expires_tick > tick
    }

    fn caster_alive(self, entities: &EntityStore) -> bool {
        entities
            .get(self.caster_id)
            .is_some_and(|entity| entity.owner == self.owner && entity.hp > 0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub(crate) struct AbilityWorldObject {
    pub(crate) id: AbilityRuntimeObjectId,
    pub(crate) owner: u32,
    pub(crate) caster_id: u32,
    pub(crate) ability: AbilityKind,
    pub(in crate::game) kind: AbilityWorldObjectKind,
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(in crate::game) created_tick: u32,
    pub(in crate::game) expires_tick: u32,
    pub(crate) payload: AbilityObjectPayload,
}

impl AbilityWorldObject {
    fn active_at(self, tick: u32) -> bool {
        self.expires_tick > tick
    }

    pub(crate) fn expires_in(self, tick: u32) -> Option<u16> {
        self.expires_tick
            .checked_sub(tick)
            .map(|remaining| remaining.min(u16::MAX as u32) as u16)
    }

    fn caster_alive(self, entities: &EntityStore) -> bool {
        entities
            .get(self.caster_id)
            .is_some_and(|entity| entity.owner == self.owner && entity.hp > 0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(in crate::game) struct AbilityWorldObjectStore {
    objects: Vec<AbilityWorldObject>,
}

impl AbilityWorldObjectStore {
    fn new() -> Self {
        Self {
            objects: Vec::new(),
        }
    }

    pub(super) fn can_insert(&self, x: f32, y: f32) -> bool {
        if !x.is_finite() || !y.is_finite() || self.objects.len() >= MAX_ACTIVE_ABILITY_OBJECTS {
            return false;
        }
        true
    }

    fn insert(&mut self, object: AbilityWorldObject) {
        self.objects.push(object);
    }

    fn retain_active(&mut self, entities: &EntityStore, tick: u32) {
        self.objects
            .retain(|object| object.active_at(tick) && object.caster_alive(entities));
    }

    pub(in crate::game) fn iter(&self) -> impl Iterator<Item = &AbilityWorldObject> {
        self.objects.iter()
    }

    fn get(&self, id: u32) -> Option<&AbilityWorldObject> {
        self.objects.iter().find(|object| object.id.get() == id)
    }

    pub(super) fn get_mut(&mut self, id: u32) -> Option<&mut AbilityWorldObject> {
        self.objects.iter_mut().find(|object| object.id.get() == id)
    }

    pub(super) fn remove(&mut self, id: u32) -> Option<AbilityWorldObject> {
        let index = self
            .objects
            .iter()
            .position(|object| object.id.get() == id)?;
        Some(self.objects.remove(index))
    }

    fn remove_active_return_markers(&mut self, owner: u32, caster_id: u32, ability: AbilityKind) {
        self.objects.retain(|object| {
            !(object.owner == owner
                && object.caster_id == caster_id
                && object.ability == ability
                && object.kind == AbilityWorldObjectKind::ReturnMarker)
        });
    }

    fn remove_active_anchors(&mut self, owner: u32, caster_id: u32, ability: AbilityKind) {
        self.objects.retain(|object| {
            !(object.owner == owner
                && object.caster_id == caster_id
                && object.ability == ability
                && object.kind == AbilityWorldObjectKind::MagicAnchor)
        });
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AbilityRuntime {
    next_id: u32,
    instances: Vec<ActiveAbilityInstance>,
    world_objects: AbilityWorldObjectStore,
    projectiles: Vec<AbilityProjectile>,
}

impl Default for AbilityRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl AbilityRuntime {
    pub(in crate::game) fn new() -> Self {
        AbilityRuntime {
            next_id: 1,
            instances: Vec::new(),
            world_objects: AbilityWorldObjectStore::new(),
            projectiles: Vec::new(),
        }
    }

    fn allocate_id(&mut self) -> AbilityRuntimeObjectId {
        let id = AbilityRuntimeObjectId(self.next_id);
        self.next_id = self.next_id.wrapping_add(1).max(1);
        id
    }

    pub(in crate::game) fn spawn_world_object(
        &mut self,
        spec: AbilityWorldObjectSpec,
    ) -> Option<u32> {
        if !self.world_objects.can_insert(spec.x, spec.y) {
            return None;
        }
        let id = self.allocate_id();
        let object = AbilityWorldObject {
            id,
            owner: spec.owner,
            caster_id: spec.caster_id,
            ability: spec.ability,
            kind: spec.kind,
            x: spec.x,
            y: spec.y,
            created_tick: spec.created_tick,
            expires_tick: spec.expires_tick,
            payload: spec.payload,
        };
        self.world_objects.insert(object);
        Some(id.get())
    }

    pub(in crate::game) fn insert_instance(&mut self, spec: ActiveAbilityInstanceSpec) -> u32 {
        let id = self.allocate_id();
        self.instances.push(ActiveAbilityInstance {
            id,
            owner: spec.owner,
            caster_id: spec.caster_id,
            ability: spec.ability,
            kind: spec.kind,
            created_tick: spec.created_tick,
            expires_tick: spec.expires_tick,
            payload: spec.payload,
        });
        id.get()
    }

    pub(in crate::game) fn spawn_projectile(&mut self, spec: AbilityProjectileSpec) -> Option<u32> {
        if !AbilityProjectile::can_spawn(&self.projectiles, &self.world_objects, &spec) {
            return None;
        }
        let id = self.allocate_id();
        let projectile = AbilityProjectile::new(id, spec);
        self.world_objects.insert(projectile.visual_object());
        self.projectiles.push(projectile);
        Some(id.get())
    }

    pub(in crate::game) fn tick(&mut self, entities: &EntityStore, tick: u32) {
        self.instances
            .retain(|instance| instance.active_at(tick) && instance.caster_alive(entities));
        self.world_objects.retain_active(entities, tick);
    }

    pub(in crate::game) fn instances(&self) -> impl Iterator<Item = &ActiveAbilityInstance> {
        self.instances.iter()
    }

    pub(in crate::game) fn world_objects(&self) -> impl Iterator<Item = &AbilityWorldObject> {
        self.world_objects.iter()
    }

    pub(crate) fn active_return_marker(
        &self,
        owner: u32,
        caster_id: u32,
        ability: AbilityKind,
        target_object_id: Option<u32>,
        tick: u32,
    ) -> Option<&AbilityWorldObject> {
        let matches_return_marker = |object: &&AbilityWorldObject| {
            object.owner == owner
                && object.caster_id == caster_id
                && object.ability == ability
                && object.kind == AbilityWorldObjectKind::ReturnMarker
                && object.active_at(tick)
                && matches!(object.payload, AbilityObjectPayload::DashReturn { .. })
                && target_object_id.is_none_or(|id| object.id.get() == id)
        };
        match target_object_id {
            Some(id) => self.world_objects.get(id).filter(matches_return_marker),
            None => self.world_objects.iter().find(matches_return_marker),
        }
    }

    pub(crate) fn consume_active_return_marker(
        &mut self,
        owner: u32,
        caster_id: u32,
        ability: AbilityKind,
        target_object_id: Option<u32>,
        tick: u32,
    ) -> Option<AbilityWorldObject> {
        let id = self
            .active_return_marker(owner, caster_id, ability, target_object_id, tick)?
            .id
            .get();
        self.world_objects.remove(id)
    }

    pub(crate) fn clear_return_markers(
        &mut self,
        owner: u32,
        caster_id: u32,
        ability: AbilityKind,
    ) {
        self.world_objects
            .remove_active_return_markers(owner, caster_id, ability);
    }

    pub(in crate::game) fn active_anchor_id(
        &self,
        owner: u32,
        caster_id: u32,
        ability: AbilityKind,
        tick: u32,
    ) -> Option<u32> {
        self.world_objects()
            .find(|object| {
                object.owner == owner
                    && object.caster_id == caster_id
                    && object.ability == ability
                    && object.kind == AbilityWorldObjectKind::MagicAnchor
                    && object.active_at(tick)
            })
            .map(|object| object.id.get())
    }

    pub(crate) fn clear_active_anchors(
        &mut self,
        owner: u32,
        caster_id: u32,
        ability: AbilityKind,
    ) {
        self.world_objects
            .remove_active_anchors(owner, caster_id, ability);
    }

    pub(in crate::game) fn projectiles(&self) -> impl Iterator<Item = &AbilityProjectile> {
        self.projectiles.iter()
    }

    pub(crate) fn active_anchor(
        &self,
        owner: u32,
        caster_id: u32,
        ability: AbilityKind,
        tick: u32,
    ) -> Option<&AbilityWorldObject> {
        self.world_objects().find(|object| {
            object.owner == owner
                && object.caster_id == caster_id
                && object.ability == ability
                && object.kind == AbilityWorldObjectKind::MagicAnchor
                && object.active_at(tick)
        })
    }

    pub(crate) fn magic_anchor_movement_multiplier(
        &self,
        x: f32,
        y: f32,
        move_dir: (f32, f32),
        tick: u32,
    ) -> f32 {
        if !x.is_finite() || !y.is_finite() {
            return 1.0;
        }
        let move_len = (move_dir.0 * move_dir.0 + move_dir.1 * move_dir.1).sqrt();
        if move_len <= 0.0001 {
            return 1.0;
        }
        let move_dir = (move_dir.0 / move_len, move_dir.1 / move_len);
        self.world_objects()
            .filter(|object| {
                object.kind == AbilityWorldObjectKind::MagicAnchor
                    && object.active_at(tick)
                    && matches!(object.payload, AbilityObjectPayload::MagicAnchor { .. })
            })
            .filter_map(|object| {
                let AbilityObjectPayload::MagicAnchor { radius } = object.payload else {
                    return None;
                };
                if radius <= 0.0 || !radius.is_finite() {
                    return None;
                }
                let dx = object.x - x;
                let dy = object.y - y;
                let dist = (dx * dx + dy * dy).sqrt();
                if dist > radius || dist <= 0.0001 {
                    return None;
                }
                let toward = (dx / dist, dy / dist);
                let alignment = move_dir.0 * toward.0 + move_dir.1 * toward.1;
                let strength = 1.0 - (dist / radius).clamp(0.0, 1.0);
                let directional = if alignment >= 0.0 {
                    (config::EKAT_MAGIC_ANCHOR_PULL_TOWARD_MULTIPLIER - 1.0) * alignment
                } else {
                    (1.0 - config::EKAT_MAGIC_ANCHOR_PULL_AWAY_MULTIPLIER) * alignment
                };
                Some(1.0 + directional * strength)
            })
            .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or(1.0)
            .max(0.0)
    }

    pub(crate) fn magic_anchor_stationary_pull(
        &self,
        x: f32,
        y: f32,
        tick: u32,
    ) -> Option<((f32, f32), f32)> {
        if !x.is_finite() || !y.is_finite() {
            return None;
        }
        self.world_objects()
            .filter(|object| {
                object.kind == AbilityWorldObjectKind::MagicAnchor
                    && object.active_at(tick)
                    && matches!(object.payload, AbilityObjectPayload::MagicAnchor { .. })
            })
            .filter_map(|object| {
                let AbilityObjectPayload::MagicAnchor { radius } = object.payload else {
                    return None;
                };
                if radius <= 0.0 || !radius.is_finite() {
                    return None;
                }
                let dx = object.x - x;
                let dy = object.y - y;
                let dist = (dx * dx + dy * dy).sqrt();
                if dist > radius || dist <= 0.0001 {
                    return None;
                }
                let strength = 1.0 - (dist / radius).clamp(0.0, 1.0);
                Some((((dx / dist), (dy / dist)), strength))
            })
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
    }

    pub(in crate::game) fn tick_projectiles(
        &mut self,
        entities: &mut EntityStore,
        teams: &TeamRelations,
        spatial: &SpatialIndex,
        tick: u32,
    ) {
        let mut active = Vec::with_capacity(self.projectiles.len());
        for mut projectile in std::mem::take(&mut self.projectiles) {
            let keep = projectile.advance(entities, teams, spatial, tick);
            if keep {
                projectile.sync_visual_object(&mut self.world_objects);
                active.push(projectile);
            } else {
                self.world_objects.remove(projectile.id.get());
            }
        }
        self.projectiles = active;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub(in crate::game) struct ActiveAbilityInstanceSpec {
    pub(in crate::game) owner: u32,
    pub(in crate::game) caster_id: u32,
    pub(in crate::game) ability: AbilityKind,
    pub(in crate::game) kind: ActiveAbilityInstanceKind,
    pub(in crate::game) created_tick: u32,
    pub(in crate::game) expires_tick: u32,
    pub(in crate::game) payload: AbilityObjectPayload,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub(in crate::game) struct AbilityWorldObjectSpec {
    pub(in crate::game) owner: u32,
    pub(in crate::game) caster_id: u32,
    pub(in crate::game) ability: AbilityKind,
    pub(in crate::game) kind: AbilityWorldObjectKind,
    pub(in crate::game) x: f32,
    pub(in crate::game) y: f32,
    pub(in crate::game) created_tick: u32,
    pub(in crate::game) expires_tick: u32,
    pub(in crate::game) payload: AbilityObjectPayload,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::entity::{EntityKind, EntityStore};

    fn spawn_caster(entities: &mut EntityStore, owner: u32) -> u32 {
        entities
            .spawn_unit(owner, EntityKind::Ekat, 128.0, 128.0)
            .expect("test caster should spawn")
    }

    fn world_object_spec(caster_id: u32, expires_tick: u32) -> AbilityWorldObjectSpec {
        AbilityWorldObjectSpec {
            owner: 1,
            caster_id,
            ability: AbilityKind::EkatTeleport,
            kind: AbilityWorldObjectKind::ReturnMarker,
            x: 128.0,
            y: 128.0,
            created_tick: 3,
            expires_tick,
            payload: AbilityObjectPayload::DashReturn {
                earliest_return_tick: 4,
            },
        }
    }

    #[test]
    fn ids_are_stable_and_not_reused_after_expiry() {
        let mut entities = EntityStore::default();
        let caster = spawn_caster(&mut entities, 1);
        let mut runtime = AbilityRuntime::new();

        let first = runtime
            .spawn_world_object(world_object_spec(caster, 10))
            .expect("first object id");
        runtime.tick(&entities, 10);
        let second = runtime
            .spawn_world_object(world_object_spec(caster, 20))
            .expect("second object id");

        assert_ne!(first, second);
        assert_eq!(first, 1);
        assert_eq!(second, 2);
        assert_eq!(runtime.world_objects().count(), 1);
    }

    #[test]
    fn expired_objects_and_instances_are_removed_on_tick() {
        let mut entities = EntityStore::default();
        let caster = spawn_caster(&mut entities, 1);
        let mut runtime = AbilityRuntime::new();

        runtime
            .spawn_world_object(world_object_spec(caster, 10))
            .expect("object id");
        runtime.insert_instance(ActiveAbilityInstanceSpec {
            owner: 1,
            caster_id: caster,
            ability: AbilityKind::EkatTeleport,
            kind: ActiveAbilityInstanceKind::DashReturn,
            created_tick: 3,
            expires_tick: 10,
            payload: AbilityObjectPayload::DashReturn {
                earliest_return_tick: 4,
            },
        });

        runtime.tick(&entities, 9);
        assert_eq!(runtime.world_objects().count(), 1);
        assert_eq!(runtime.instances().count(), 1);

        runtime.tick(&entities, 10);
        assert_eq!(runtime.world_objects().count(), 0);
        assert_eq!(runtime.instances().count(), 0);
    }

    #[test]
    fn cloned_runtime_preserves_active_state_and_next_id() {
        let mut entities = EntityStore::default();
        let caster = spawn_caster(&mut entities, 1);
        let mut runtime = AbilityRuntime::new();

        let first = runtime
            .spawn_world_object(world_object_spec(caster, 20))
            .expect("object id");
        let mut cloned = runtime.clone();
        let second = cloned
            .spawn_world_object(world_object_spec(caster, 30))
            .expect("object id");

        assert_eq!(first, 1);
        assert_eq!(second, 2);
        assert_eq!(runtime.world_objects().count(), 1);
        assert_eq!(cloned.world_objects().count(), 2);
    }

    #[test]
    fn stale_caster_cleanup_is_panic_free() {
        let mut entities = EntityStore::default();
        let caster = spawn_caster(&mut entities, 1);
        let mut runtime = AbilityRuntime::new();

        runtime
            .spawn_world_object(world_object_spec(caster, 20))
            .expect("object id");
        runtime.insert_instance(ActiveAbilityInstanceSpec {
            owner: 1,
            caster_id: caster + 1000,
            ability: AbilityKind::EkatTeleport,
            kind: ActiveAbilityInstanceKind::DashReturn,
            created_tick: 3,
            expires_tick: 20,
            payload: AbilityObjectPayload::None,
        });
        entities.remove(caster);

        runtime.tick(&entities, 4);

        assert_eq!(runtime.world_objects().count(), 0);
        assert_eq!(runtime.instances().count(), 0);
    }

    #[test]
    fn non_finite_world_object_is_rejected_without_consuming_id() {
        let mut entities = EntityStore::default();
        let caster = spawn_caster(&mut entities, 1);
        let mut runtime = AbilityRuntime::new();
        let mut spec = world_object_spec(caster, 20);
        spec.x = f32::NAN;

        assert_eq!(runtime.spawn_world_object(spec), None);
        let next = runtime
            .spawn_world_object(world_object_spec(caster, 20))
            .expect("object id");

        assert_eq!(next, 1);
    }
}
