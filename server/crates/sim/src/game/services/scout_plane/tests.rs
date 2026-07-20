use super::*;
use crate::game::entity::{EntityKind, ScoutPlaneState};
use crate::game::map::Map;
use crate::protocol::terrain;
use crate::rules::{
    combat,
    defs::{self, TechRequirement, WeaponClass},
};

const EPS: f32 = 0.01;

fn test_map(size: u32) -> Map {
    Map {
        size,
        terrain: vec![terrain::GRASS; (size * size) as usize],
        starts: vec![(4, 4), (size.saturating_sub(5), size.saturating_sub(5))],
        base_sites: Vec::new(),
    }
}

fn spawn_plane(entities: &mut EntityStore, owner: u32, x: f32, y: f32) -> u32 {
    entities
        .spawn_unit(owner, EntityKind::ScoutPlane, x, y)
        .expect("scout plane should spawn")
}

fn plane_state(entities: &EntityStore, id: u32) -> ScoutPlaneState {
    *entities
        .get(id)
        .expect("plane should exist")
        .scout_plane_state()
        .expect("plane state should exist")
}

#[test]
fn scout_plane_requirement_numbers_and_non_combat_contract_are_stable() {
    let def = defs::unit_def(EntityKind::ScoutPlane).expect("Scout Plane def");
    assert_eq!(def.trained_at, None);
    assert!(
        matches!(def.train_requirement, TechRequirement::All(requirements) if requirements.is_empty())
    );
    assert_eq!(def.weapon, WeaponClass::None);
    assert_eq!(combat::default_weapon_kind(EntityKind::ScoutPlane), None);
    assert_eq!(def.stats.hp, 40);
    assert_eq!(def.stats.dmg, 0);
    assert_eq!(def.stats.range_tiles, 0);
    assert_eq!(def.stats.cooldown, 0);
    assert_eq!(def.stats.sight_tiles, 16);
    assert_eq!(def.stats.cost_steel, 50);
    assert_eq!(def.stats.cost_oil, 75);
    assert_eq!(def.stats.supply, 0);
    assert_eq!(def.stats.build_ticks, 0);
    assert_eq!(config::SCOUT_PLANE_ORBIT_RADIUS_TILES, 2);
    assert_eq!(config::SCOUT_PLANE_LIFETIME_TICKS, 600);
    assert_eq!(config::SCOUT_PLANE_ABILITY_COOLDOWN_TICKS, 900);
}

#[test]
fn scout_plane_launches_from_caster_without_a_city_centre() {
    let map = test_map(32);
    let mut entities = EntityStore::new();
    let source_command_car = entities
        .spawn_unit(1, EntityKind::CommandCar, 320.0, 448.0)
        .expect("source Command Car should spawn");

    let plane = launch_ability(&map, &mut entities, 1, source_command_car, 790.0, 790.0)
        .expect("launch succeeds");
    let plane_entity = entities.get(plane).expect("plane exists");
    assert_eq!(plane_entity.owner, 1);
    assert_eq!(plane_entity.pos_x, 320.0);
    assert_eq!(plane_entity.pos_y, 448.0);

    let state = plane_state(&entities, plane);
    assert_eq!(state.source_command_car, Some(source_command_car));
    assert_eq!(state.orbit_center, (790.0, 790.0));
    let second = launch_ability(&map, &mut entities, 1, source_command_car, 128.0, 128.0)
        .expect("an existing sortie should not invalidate another launch");
    assert_ne!(plane, second);
    assert_eq!(
        launch_ability(&map, &mut entities, 3, 77, 128.0, 128.0),
        Err(ScoutPlaneLaunchError::InvalidLaunch)
    );
}

#[test]
fn scout_plane_lifetime_expires_during_transit() {
    let map = test_map(32);
    let mut entities = EntityStore::new();
    let plane = spawn_plane(&mut entities, 1, 256.0, 256.0);
    if let Some(state) = entities
        .get_mut(plane)
        .and_then(|plane| plane.scout_plane_state_mut())
    {
        *state = ScoutPlaneState::launched_at(790.0, 790.0);
        state.lifetime_ticks_remaining = 2;
    }

    advance_scout_planes(&map, &mut entities);
    let state = plane_state(&entities, plane);
    assert!(
        !state.orbiting,
        "distant target should keep the plane in transit"
    );
    assert_eq!(
        state.lifetime_ticks_remaining, 1,
        "transit should consume sortie lifetime"
    );
    advance_scout_planes(&map, &mut entities);
    assert!(
        entities.get(plane).is_none(),
        "plane should disappear when its total lifetime expires before arrival"
    );
}

#[test]
fn scout_plane_arrival_uses_only_remaining_lifetime_for_orbit() {
    let map = test_map(32);
    let mut entities = EntityStore::new();
    let plane = spawn_plane(&mut entities, 1, 256.0, 256.0);
    if let Some(state) = entities
        .get_mut(plane)
        .and_then(|plane| plane.scout_plane_state_mut())
    {
        *state = ScoutPlaneState::launched_at(256.0, 256.0);
        state.lifetime_ticks_remaining = 2;
    }

    advance_scout_planes(&map, &mut entities);
    let state = plane_state(&entities, plane);
    assert!(state.orbiting, "arrival should establish orbit");
    assert_eq!(
        state.lifetime_ticks_remaining, 1,
        "arrival should not reset the lifetime consumed by the sortie"
    );
    advance_scout_planes(&map, &mut entities);
    assert!(
        entities.get(plane).is_none(),
        "plane should orbit only for its remaining lifetime"
    );
}

#[test]
fn multiple_scout_planes_survive_independent_mission_processing() {
    let map = test_map(32);
    let mut entities = EntityStore::new();
    let first = spawn_plane(&mut entities, 1, 128.0, 128.0);
    let second = spawn_plane(&mut entities, 1, 160.0, 160.0);

    advance_scout_planes(&map, &mut entities);

    assert!(entities.get(first).is_some());
    assert!(entities.get(second).is_some());
}

#[test]
fn scout_planes_from_different_command_cars_survive_mission_processing() {
    let map = test_map(32);
    let mut entities = EntityStore::new();
    let first = spawn_plane(&mut entities, 1, 128.0, 128.0);
    let second = spawn_plane(&mut entities, 1, 160.0, 160.0);
    for (plane, command_car) in [(first, 10), (second, 11)] {
        if let Some(state) = entities
            .get_mut(plane)
            .and_then(|plane| plane.scout_plane_state_mut())
        {
            *state = ScoutPlaneState::launched_from_command_car(command_car, 512.0, 512.0);
        }
    }

    advance_scout_planes(&map, &mut entities);

    assert!(entities.get(first).is_some());
    assert!(entities.get(second).is_some());
}

#[test]
fn scout_plane_travels_to_orbit_ring_without_overshooting() {
    let speed = config::SCOUT_PLANE_SPEED_PX_PER_TICK;
    let orbit_radius = config::SCOUT_PLANE_ORBIT_RADIUS_TILES as f32 * config::TILE_SIZE as f32;
    let snapshot = ScoutPlaneSnapshot {
        x: 0.0,
        y: 0.0,
        center: (orbit_radius + speed * 2.0, 0.0),
        phase: 0.0,
        orbiting: false,
    };

    let step = advance_one(snapshot, speed, orbit_radius, 2_048.0);
    assert!((step.x - speed).abs() <= EPS);
    assert!(step.y.abs() <= EPS);
    assert!(!step.orbiting);
}
