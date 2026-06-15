#![allow(dead_code)]

use crate::game::ability::AbilityKind;
use crate::game::entity::EntityStore;

pub(in crate::game) const MAX_ACTIVE_ABILITY_OBJECTS: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(in crate::game) struct AbilityRuntimeObjectId(u32);

impl AbilityRuntimeObjectId {
    pub(in crate::game) fn get(self) -> u32 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub(in crate::game) enum ActiveAbilityInstanceKind {
    DashReturn,
    MagicAnchor,
    LineProjectile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub(in crate::game) enum AbilityWorldObjectKind {
    ReturnMarker,
    MagicAnchor,
    LineProjectile,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(in crate::game) enum AbilityObjectPayload {
    #[default]
    None,
    DashReturn {
        earliest_return_tick: u32,
    },
    MagicAnchor {
        hp: u16,
        radius: f32,
        destroyed_lockout_ticks: u32,
    },
    LineProjectile {
        distance_traveled: f32,
        ticks_out: u32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(in crate::game) struct ActiveAbilityInstance {
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub(in crate::game) struct AbilityWorldObject {
    pub(in crate::game) id: AbilityRuntimeObjectId,
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

impl AbilityWorldObject {
    fn active_at(self, tick: u32) -> bool {
        self.expires_tick > tick
    }

    pub(in crate::game) fn expires_in(self, tick: u32) -> Option<u16> {
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

#[derive(Debug, Clone)]
pub(in crate::game) struct AbilityWorldObjectStore {
    objects: Vec<AbilityWorldObject>,
}

impl AbilityWorldObjectStore {
    fn new() -> Self {
        AbilityWorldObjectStore {
            objects: Vec::new(),
        }
    }

    fn can_insert(&self, x: f32, y: f32) -> bool {
        if !x.is_finite() || !y.is_finite() {
            return false;
        }
        if self.objects.len() >= MAX_ACTIVE_ABILITY_OBJECTS {
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
}

#[derive(Debug, Clone)]
pub(in crate::game) struct AbilityRuntime {
    next_id: u32,
    instances: Vec<ActiveAbilityInstance>,
    world_objects: AbilityWorldObjectStore,
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
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(in crate::game) struct ActiveAbilityInstanceSpec {
    pub(in crate::game) owner: u32,
    pub(in crate::game) caster_id: u32,
    pub(in crate::game) ability: AbilityKind,
    pub(in crate::game) kind: ActiveAbilityInstanceKind,
    pub(in crate::game) created_tick: u32,
    pub(in crate::game) expires_tick: u32,
    pub(in crate::game) payload: AbilityObjectPayload,
}

#[derive(Debug, Clone, Copy, PartialEq)]
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
