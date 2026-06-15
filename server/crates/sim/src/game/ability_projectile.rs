use std::collections::BTreeSet;

use crate::game::ability::AbilityKind;
use crate::game::ability_runtime::{
    AbilityObjectPayload, AbilityRuntimeObjectId, AbilityWorldObject, AbilityWorldObjectKind,
    AbilityWorldObjectStore,
};
use crate::game::entity::EntityStore;
use crate::game::services::spatial::SpatialIndex;
use crate::game::teams::TeamRelations;

const MAX_ACTIVE_ABILITY_PROJECTILES: usize = 512;
const PROJECTILE_EPSILON: f32 = 0.001;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum AbilityProjectileLeg {
    Outbound,
    Return,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(dead_code)]
pub(crate) enum AbilityProjectileReturnTarget {
    FixedPoint { x: f32, y: f32 },
    Entity { id: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct AbilityProjectileSpec {
    pub(crate) owner: u32,
    pub(crate) caster_id: u32,
    pub(crate) source_object_id: Option<u32>,
    pub(crate) ability: AbilityKind,
    pub(crate) origin: (f32, f32),
    pub(crate) endpoint: (f32, f32),
    pub(crate) return_target: AbilityProjectileReturnTarget,
    pub(crate) speed_px_per_tick: f32,
    pub(crate) width_px: f32,
    pub(crate) damage: u32,
    pub(crate) created_tick: u32,
    pub(crate) expires_tick: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct AbilityProjectile {
    pub(crate) id: AbilityRuntimeObjectId,
    pub(crate) owner: u32,
    pub(crate) caster_id: u32,
    pub(crate) source_object_id: Option<u32>,
    pub(crate) ability: AbilityKind,
    pub(crate) origin: (f32, f32),
    pub(crate) endpoint: (f32, f32),
    pub(crate) return_target: AbilityProjectileReturnTarget,
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) leg: AbilityProjectileLeg,
    pub(crate) speed_px_per_tick: f32,
    pub(crate) width_px: f32,
    pub(crate) damage: u32,
    pub(crate) created_tick: u32,
    pub(crate) expires_tick: u32,
    pub(crate) distance_traveled: f32,
    pub(crate) ticks_out: u32,
    outbound_hits: BTreeSet<u32>,
    return_hits: BTreeSet<u32>,
}

impl AbilityProjectile {
    pub(super) fn can_spawn(
        active: &[AbilityProjectile],
        world_objects: &AbilityWorldObjectStore,
        spec: &AbilityProjectileSpec,
    ) -> bool {
        active.len() < MAX_ACTIVE_ABILITY_PROJECTILES
            && projectile_spec_valid(spec)
            && world_objects.can_insert(spec.origin.0, spec.origin.1)
    }

    pub(super) fn new(id: AbilityRuntimeObjectId, spec: AbilityProjectileSpec) -> Self {
        AbilityProjectile {
            id,
            owner: spec.owner,
            caster_id: spec.caster_id,
            source_object_id: spec.source_object_id,
            ability: spec.ability,
            origin: spec.origin,
            endpoint: spec.endpoint,
            return_target: spec.return_target,
            x: spec.origin.0,
            y: spec.origin.1,
            leg: AbilityProjectileLeg::Outbound,
            speed_px_per_tick: spec.speed_px_per_tick,
            width_px: spec.width_px,
            damage: spec.damage,
            created_tick: spec.created_tick,
            expires_tick: spec.expires_tick,
            distance_traveled: 0.0,
            ticks_out: 0,
            outbound_hits: BTreeSet::new(),
            return_hits: BTreeSet::new(),
        }
    }

    pub(super) fn visual_object(&self) -> AbilityWorldObject {
        AbilityWorldObject {
            id: self.id,
            owner: self.owner,
            caster_id: self.caster_id,
            ability: self.ability,
            kind: AbilityWorldObjectKind::LineProjectile,
            x: self.x,
            y: self.y,
            created_tick: self.created_tick,
            expires_tick: self.expires_tick,
            payload: AbilityObjectPayload::LineProjectile {
                distance_traveled: self.distance_traveled,
                ticks_out: self.ticks_out,
            },
        }
    }

    pub(super) fn sync_visual_object(&self, world_objects: &mut AbilityWorldObjectStore) {
        if let Some(object) = world_objects.get_mut(self.id.get()) {
            object.x = self.x;
            object.y = self.y;
            object.payload = AbilityObjectPayload::LineProjectile {
                distance_traveled: self.distance_traveled,
                ticks_out: self.ticks_out,
            };
            object.expires_tick = self.expires_tick;
        }
    }

    pub(super) fn advance(
        &mut self,
        entities: &mut EntityStore,
        teams: &TeamRelations,
        spatial: &SpatialIndex,
        tick: u32,
    ) -> bool {
        if !self.active_at(tick) || !self.caster_alive(entities) {
            return false;
        }
        let Some(target) = self.target_point(entities) else {
            return false;
        };
        let start = (self.x, self.y);
        let (next, reached_target, traveled) = step_toward(start, target, self.speed_px_per_tick);
        self.x = next.0;
        self.y = next.1;
        self.distance_traveled += traveled;
        if self.leg == AbilityProjectileLeg::Outbound {
            self.ticks_out = self.ticks_out.saturating_add(1);
        }

        apply_projectile_hits(self, start, next, entities, teams, spatial, tick);

        if reached_target {
            match self.leg {
                AbilityProjectileLeg::Outbound => {
                    self.leg = AbilityProjectileLeg::Return;
                    true
                }
                AbilityProjectileLeg::Return => false,
            }
        } else {
            true
        }
    }

    fn active_at(&self, tick: u32) -> bool {
        self.expires_tick > tick
    }

    fn caster_alive(&self, entities: &EntityStore) -> bool {
        entities
            .get(self.caster_id)
            .is_some_and(|entity| entity.owner == self.owner && entity.hp > 0)
    }

    fn target_point(&self, entities: &EntityStore) -> Option<(f32, f32)> {
        match self.leg {
            AbilityProjectileLeg::Outbound => Some(self.endpoint),
            AbilityProjectileLeg::Return => match self.return_target {
                AbilityProjectileReturnTarget::FixedPoint { x, y } => Some((x, y)),
                AbilityProjectileReturnTarget::Entity { id } => entities
                    .get(id)
                    .filter(|entity| entity.hp > 0)
                    .map(|entity| (entity.pos_x, entity.pos_y)),
            },
        }
    }

    fn hits_for_current_leg(&mut self) -> &mut BTreeSet<u32> {
        match self.leg {
            AbilityProjectileLeg::Outbound => &mut self.outbound_hits,
            AbilityProjectileLeg::Return => &mut self.return_hits,
        }
    }
}

fn projectile_spec_valid(spec: &AbilityProjectileSpec) -> bool {
    point_valid(spec.origin)
        && point_valid(spec.endpoint)
        && match spec.return_target {
            AbilityProjectileReturnTarget::FixedPoint { x, y } => point_valid((x, y)),
            AbilityProjectileReturnTarget::Entity { .. } => true,
        }
        && spec.speed_px_per_tick.is_finite()
        && spec.speed_px_per_tick > 0.0
        && spec.width_px.is_finite()
        && spec.width_px >= 0.0
        && spec.expires_tick > spec.created_tick
}

fn point_valid((x, y): (f32, f32)) -> bool {
    x.is_finite() && y.is_finite()
}

fn step_toward(start: (f32, f32), target: (f32, f32), speed: f32) -> ((f32, f32), bool, f32) {
    let dx = target.0 - start.0;
    let dy = target.1 - start.1;
    let distance = (dx * dx + dy * dy).sqrt();
    if distance <= PROJECTILE_EPSILON {
        return (target, true, 0.0);
    }
    if distance <= speed {
        return (target, true, distance);
    }
    let scale = speed / distance;
    ((start.0 + dx * scale, start.1 + dy * scale), false, speed)
}

fn apply_projectile_hits(
    projectile: &mut AbilityProjectile,
    start: (f32, f32),
    end: (f32, f32),
    entities: &mut EntityStore,
    teams: &TeamRelations,
    spatial: &SpatialIndex,
    tick: u32,
) {
    if projectile.damage == 0 {
        return;
    }
    let mut hits = Vec::new();
    for id in spatial.all_ids() {
        let Some(target) = entities.get(id) else {
            continue;
        };
        if target.hp == 0
            || target.is_node()
            || !teams.is_enemy_owner(projectile.owner, target.owner)
            || projectile.hits_for_current_leg().contains(&id)
        {
            continue;
        }
        let nearest = nearest_point_on_segment(start, end, (target.pos_x, target.pos_y));
        let hit_distance = distance((target.pos_x, target.pos_y), nearest);
        if hit_distance <= target.radius() + projectile.width_px {
            hits.push((id, distance(start, nearest)));
        }
    }
    hits.sort_by(|a, b| a.1.total_cmp(&b.1).then_with(|| a.0.cmp(&b.0)));
    for (id, _) in hits {
        projectile.hits_for_current_leg().insert(id);
        if let Some(target) = entities.get_mut(id) {
            target.apply_damage(projectile.damage, Some((projectile.owner, start, tick)));
        }
    }
}

fn nearest_point_on_segment(start: (f32, f32), end: (f32, f32), point: (f32, f32)) -> (f32, f32) {
    let dx = end.0 - start.0;
    let dy = end.1 - start.1;
    let len2 = dx * dx + dy * dy;
    if len2 <= f32::EPSILON {
        return start;
    }
    let t = (((point.0 - start.0) * dx + (point.1 - start.1) * dy) / len2).clamp(0.0, 1.0);
    (start.0 + dx * t, start.1 + dy * t)
}

fn distance(a: (f32, f32), b: (f32, f32)) -> f32 {
    ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::ability_runtime::AbilityRuntime;
    use crate::game::entity::{EntityKind, EntityStore};

    fn spawn_caster(entities: &mut EntityStore, owner: u32) -> u32 {
        entities
            .spawn_unit(owner, EntityKind::Ekat, 128.0, 128.0)
            .expect("test caster should spawn")
    }

    fn teams() -> TeamRelations {
        TeamRelations::from_player_teams([(1, 1), (2, 2), (3, 1)])
    }

    fn tick_runtime(runtime: &mut AbilityRuntime, entities: &mut EntityStore, tick: u32) {
        let spatial = SpatialIndex::build(entities, 32);
        runtime.tick_projectiles(entities, &teams(), &spatial, tick);
        runtime.tick(entities, tick);
    }

    fn projectile_spec(
        caster_id: u32,
        origin: (f32, f32),
        endpoint: (f32, f32),
    ) -> AbilityProjectileSpec {
        AbilityProjectileSpec {
            owner: 1,
            caster_id,
            source_object_id: None,
            ability: AbilityKind::EkatLineShot,
            origin,
            endpoint,
            return_target: AbilityProjectileReturnTarget::FixedPoint {
                x: origin.0,
                y: origin.1,
            },
            speed_px_per_tick: 64.0,
            width_px: 8.0,
            damage: 10,
            created_tick: 0,
            expires_tick: 60,
        }
    }

    #[test]
    fn projectile_moves_outbound_then_turns_to_return() {
        let mut entities = EntityStore::default();
        let caster = spawn_caster(&mut entities, 1);
        let mut runtime = AbilityRuntime::new();
        let projectile_id = runtime
            .spawn_projectile(projectile_spec(caster, (128.0, 128.0), (256.0, 128.0)))
            .expect("projectile id");

        tick_runtime(&mut runtime, &mut entities, 1);
        let object = runtime
            .world_objects()
            .find(|object| object.id.get() == projectile_id)
            .expect("projectile visual object");
        assert_eq!((object.x, object.y), (192.0, 128.0));

        tick_runtime(&mut runtime, &mut entities, 2);
        let object = runtime
            .world_objects()
            .find(|object| object.id.get() == projectile_id)
            .expect("projectile visual object");
        assert_eq!((object.x, object.y), (256.0, 128.0));
        assert_eq!(
            object.payload,
            AbilityObjectPayload::LineProjectile {
                distance_traveled: 128.0,
                ticks_out: 2
            }
        );
    }

    #[test]
    fn projectile_completes_fixed_point_return_and_removes_visual_object() {
        let mut entities = EntityStore::default();
        let caster = spawn_caster(&mut entities, 1);
        let mut runtime = AbilityRuntime::new();
        runtime
            .spawn_projectile(projectile_spec(caster, (128.0, 128.0), (192.0, 128.0)))
            .expect("projectile id");

        tick_runtime(&mut runtime, &mut entities, 1);
        assert_eq!(runtime.world_objects().count(), 1);
        tick_runtime(&mut runtime, &mut entities, 2);

        assert_eq!(runtime.world_objects().count(), 0);
    }

    #[test]
    fn projectile_return_leg_steers_toward_live_caster() {
        let mut entities = EntityStore::default();
        let caster = spawn_caster(&mut entities, 1);
        let mut runtime = AbilityRuntime::new();
        let mut spec = projectile_spec(caster, (128.0, 128.0), (192.0, 128.0));
        spec.return_target = AbilityProjectileReturnTarget::Entity { id: caster };
        runtime.spawn_projectile(spec).expect("projectile id");

        tick_runtime(&mut runtime, &mut entities, 1);
        if let Some(caster) = entities.get_mut(caster) {
            caster.pos_x = 192.0;
            caster.pos_y = 256.0;
        }
        tick_runtime(&mut runtime, &mut entities, 2);

        let object = runtime
            .world_objects()
            .next()
            .expect("projectile visual object");
        assert!(object.y > 128.0);
    }

    #[test]
    fn swept_projectile_hits_enemy_between_ticks_once_per_leg() {
        let mut entities = EntityStore::default();
        let caster = spawn_caster(&mut entities, 1);
        let enemy = entities
            .spawn_unit(2, EntityKind::Rifleman, 180.0, 128.0)
            .expect("enemy should spawn");
        let mut runtime = AbilityRuntime::new();
        runtime
            .spawn_projectile(projectile_spec(caster, (128.0, 128.0), (256.0, 128.0)))
            .expect("projectile id");

        tick_runtime(&mut runtime, &mut entities, 1);
        let after_first_hit = entities.get(enemy).expect("enemy exists").hp;
        tick_runtime(&mut runtime, &mut entities, 2);
        assert_eq!(
            entities.get(enemy).expect("enemy exists").hp,
            after_first_hit
        );
        tick_runtime(&mut runtime, &mut entities, 3);
        assert_eq!(
            entities.get(enemy).expect("enemy exists").hp,
            after_first_hit.saturating_sub(10)
        );
    }

    #[test]
    fn projectile_filters_allies_with_team_relations() {
        let mut entities = EntityStore::default();
        let caster = spawn_caster(&mut entities, 1);
        let ally = entities
            .spawn_unit(3, EntityKind::Rifleman, 180.0, 128.0)
            .expect("ally should spawn");
        let mut runtime = AbilityRuntime::new();
        runtime
            .spawn_projectile(projectile_spec(caster, (128.0, 128.0), (256.0, 128.0)))
            .expect("projectile id");

        tick_runtime(&mut runtime, &mut entities, 1);

        let ally_entity = entities.get(ally).expect("ally exists");
        assert_eq!(ally_entity.hp, ally_entity.max_hp);
    }

    #[test]
    fn projectile_with_stale_caster_is_removed_without_panic() {
        let mut entities = EntityStore::default();
        let caster = spawn_caster(&mut entities, 1);
        let mut runtime = AbilityRuntime::new();
        runtime
            .spawn_projectile(projectile_spec(caster, (128.0, 128.0), (256.0, 128.0)))
            .expect("projectile id");
        entities.remove(caster);

        tick_runtime(&mut runtime, &mut entities, 1);

        assert_eq!(runtime.world_objects().count(), 0);
    }

    #[test]
    fn projectile_with_missing_source_object_still_advances() {
        let mut entities = EntityStore::default();
        let caster = spawn_caster(&mut entities, 1);
        let mut runtime = AbilityRuntime::new();
        let mut spec = projectile_spec(caster, (128.0, 128.0), (192.0, 128.0));
        spec.source_object_id = Some(9999);
        runtime.spawn_projectile(spec).expect("projectile id");

        tick_runtime(&mut runtime, &mut entities, 1);
    }

    #[test]
    fn projectile_visual_object_is_fog_projectable_runtime_state() {
        let mut entities = EntityStore::default();
        let caster = spawn_caster(&mut entities, 1);
        let mut runtime = AbilityRuntime::new();
        let projectile_id = runtime
            .spawn_projectile(projectile_spec(caster, (128.0, 128.0), (256.0, 128.0)))
            .expect("projectile id");

        tick_runtime(&mut runtime, &mut entities, 1);

        let object = runtime
            .world_objects()
            .find(|object| object.id.get() == projectile_id)
            .expect("projectile visual object");
        assert_eq!((object.x, object.y), (192.0, 128.0));
        assert_eq!(
            object.payload,
            AbilityObjectPayload::LineProjectile {
                distance_traveled: 64.0,
                ticks_out: 1
            }
        );
    }
}
