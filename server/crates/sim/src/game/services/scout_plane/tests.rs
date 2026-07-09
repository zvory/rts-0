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
        expansion_sites: Vec::new(),
    }
}

fn spawn_city_centre(
    entities: &mut EntityStore,
    owner: u32,
    x: f32,
    y: f32,
) -> u32 {
    entities
        .spawn_building(owner, EntityKind::CityCentre, x, y, true)
        .expect("city centre should spawn")
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
    assert!(matches!(def.train_requirement, TechRequirement::All(requirements) if requirements.is_empty()));
    assert_eq!(def.weapon, WeaponClass::None);
    assert_eq!(combat::default_weapon_kind(EntityKind::ScoutPlane), None);
    assert_eq!(def.stats.hp, 40);
    assert_eq!(def.stats.dmg, 0);
    assert_eq!(def.stats.range_tiles, 0);
    assert_eq!(def.stats.cooldown, 0);
    assert_eq!(def.stats.sight_tiles, 12);
    assert_eq!(def.stats.cost_steel, 50);
    assert_eq!(def.stats.cost_oil, 50);
    assert_eq!(def.stats.supply, 0);
    assert_eq!(def.stats.build_ticks, 0);
    assert_eq!(config::SCOUT_PLANE_ORBIT_RADIUS_TILES, 4);
    assert_eq!(config::SCOUT_PLANE_ORBIT_DURATION_TICKS, 300);
    assert_eq!(config::SCOUT_PLANE_ABILITY_COOLDOWN_TICKS, 900);
}

#[test]
fn scout_plane_launches_from_nearest_owned_completed_city_centre_to_target() {
    let map = test_map(32);
    let mut entities = EntityStore::new();
    let far = spawn_city_centre(&mut entities, 1, 96.0, 96.0);
    let near = spawn_city_centre(&mut entities, 1, 768.0, 768.0);
    let enemy = spawn_city_centre(&mut entities, 2, 800.0, 800.0);

    let plane = launch_ability(&map, &mut entities, 1, 790.0, 790.0).expect("launch succeeds");
    let plane_entity = entities.get(plane).expect("plane exists");
    assert_eq!(plane_entity.owner, 1);
    assert_eq!(plane_entity.pos_x, 768.0);
    assert_eq!(plane_entity.pos_y, 768.0);

    let state = plane_state(&entities, plane);
    assert_eq!(state.home_city_centre, Some(near));
    assert_eq!(state.orbit_center, (790.0, 790.0));
    assert_eq!(
        launch_ability(&map, &mut entities, 1, 128.0, 128.0),
        Err(ScoutPlaneLaunchError::Active)
    );
    assert_eq!(
        launch_ability(&map, &mut entities, 3, 128.0, 128.0),
        Err(ScoutPlaneLaunchError::NoCityCentre)
    );
    assert_eq!(entities.get(far).expect("far cc").owner, 1);
    assert_eq!(entities.get(enemy).expect("enemy cc").owner, 2);
}

#[test]
fn scout_plane_orbit_timer_starts_after_arrival_then_returns_and_despawns() {
    let map = test_map(32);
    let mut entities = EntityStore::new();
    let home = spawn_city_centre(&mut entities, 1, 256.0, 256.0);
    let plane = spawn_plane(&mut entities, 1, 256.0, 256.0);
    if let Some(state) = entities
        .get_mut(plane)
        .and_then(|plane| plane.scout_plane_state_mut())
    {
        *state = ScoutPlaneState::launched_from(home, 256.0, 256.0);
        state.station_ticks_remaining = 1;
    }

    advance_scout_planes(&map, &mut entities);
    let state = plane_state(&entities, plane);
    assert!(state.orbiting, "arrival should establish orbit");
    assert_eq!(
        state.station_ticks_remaining, 1,
        "station timer should not decrement on the arrival tick"
    );
    assert!(!state.returning);

    advance_scout_planes(&map, &mut entities);
    assert!(
        plane_state(&entities, plane).returning,
        "expired station time should start return-to-base"
    );

    for _ in 0..512 {
        if entities.get(plane).is_none() {
            return;
        }
        advance_scout_planes(&map, &mut entities);
    }
    panic!("returning Scout Plane should despawn on reaching home");
}

#[test]
fn scout_plane_disappears_after_orbit_if_home_city_centre_died() {
    let map = test_map(32);
    let mut entities = EntityStore::new();
    let home = spawn_city_centre(&mut entities, 1, 256.0, 256.0);
    let plane = spawn_plane(&mut entities, 1, 256.0, 256.0);
    if let Some(state) = entities
        .get_mut(plane)
        .and_then(|plane| plane.scout_plane_state_mut())
    {
        *state = ScoutPlaneState::launched_from(home, 256.0, 256.0);
        state.station_ticks_remaining = 1;
    }

    advance_scout_planes(&map, &mut entities);
    let _ = entities.remove(home);
    advance_scout_planes(&map, &mut entities);
    assert!(
        entities.get(plane).is_none(),
        "plane should disappear when station time ends without its launch City Centre"
    );
}

#[test]
fn duplicate_scout_planes_are_cleaned_up_before_extra_mission_processing() {
    let map = test_map(32);
    let mut entities = EntityStore::new();
    let first = spawn_plane(&mut entities, 1, 128.0, 128.0);
    let second = spawn_plane(&mut entities, 1, 160.0, 160.0);

    advance_scout_planes(&map, &mut entities);

    assert!(entities.get(first).is_some());
    assert!(entities.get(second).is_none());
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
