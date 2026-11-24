use super::*;
use crate::protocol::terrain;

fn fixture() -> (Map, EntityStore, u32, u32) {
    let map = Map {
        width: 40,
        height: 40,
        terrain: vec![terrain::GRASS; 1600],
        ..Default::default()
    };
    let mut entities = EntityStore::new();
    let attacker = entities
        .spawn_unit(1, EntityKind::WindAdept, 410.0, 600.0)
        .unwrap();
    let target = entities
        .spawn_unit(2, EntityKind::ScoutCar, 500.0, 600.0)
        .unwrap();
    entities
        .get_mut(target)
        .unwrap()
        .set_movement_delta(2.35, 0.0);
    let unit = entities.get_mut(attacker).unwrap();
    unit.set_order(Order::attack(target));
    unit.set_path(vec![(420.0, 600.0)]);
    unit.set_path_goal(Some((420.0, 600.0)));
    unit.set_last_repath_tick(10);
    (map, entities, attacker, target)
}

#[test]
fn terminal_tracking_updates_rotated_hull_goals_without_search_budget() {
    for angle in [0.0, std::f32::consts::FRAC_PI_2, 0.73] {
        let (map, mut entities, attacker, target) = fixture();
        entities.get_mut(target).unwrap().set_facing(angle);
        let occ = Occupancy::build(&map, &entities);
        let mut pathing = PathingService::new(8192, 256);
        let mut coordinator = MoveCoordinator::new(&mut pathing, &map, &occ, 11);
        let budget = coordinator.budget;
        assert!(coordinator.request_direct_attack_path(
            &mut entities,
            attacker,
            target,
            0.0,
            54.0
        ));
        assert_eq!(
            coordinator.budget, budget,
            "local tracking must not consume tile searches"
        );
        let unit = entities.get(attacker).unwrap();
        let point = primary_combat_target_point(
            &map,
            unit.default_weapon_kind().unwrap(),
            (410.0, 600.0),
            entities.get(target).unwrap(),
        );
        let expected =
            direct_attack_staging_goal(&map, (410.0, 600.0), point, 0.0, 54.0).unwrap();
        assert_eq!(unit.path_goal(), Some(expected));
        assert_eq!(unit.next_waypoint(), Some(expected));
        assert_eq!(
            unit.last_repath_tick(),
            10,
            "tracking is not a new tile search"
        );
    }
}

#[test]
fn terminal_tracking_preserves_stationary_approaching_detour_and_travel_paths() {
    for case in [
        "stationary",
        "approaching",
        "detour",
        "travel",
        "minimum-range",
        "blocked",
        "large-jump",
    ] {
        let (mut map, mut entities, attacker, target) = fixture();
        match case {
            "stationary" => entities
                .get_mut(target)
                .unwrap()
                .set_movement_delta(0.0, 0.0),
            "approaching" => entities
                .get_mut(target)
                .unwrap()
                .set_movement_delta(-2.35, 0.0),
            "detour" => entities
                .get_mut(attacker)
                .unwrap()
                .set_path(vec![(420.0, 600.0), (410.0, 632.0)]),
            "travel" => {
                let unit = entities.get_mut(attacker).unwrap();
                unit.set_order(Order::attack_move_to(420.0, 600.0));
                unit.set_target_id(Some(target));
            }
            "blocked" => {
                let tile = (600 / 32 * 40 + 13) as usize;
                map.terrain[tile] = terrain::WATER;
            }
            "large-jump" => entities.get_mut(target).unwrap().set_position(700.0, 600.0),
            _ => {}
        }
        let before = entities
            .get(attacker)
            .unwrap()
            .movement
            .as_ref()
            .unwrap()
            .path
            .clone();
        let occ = Occupancy::build(&map, &entities);
        let mut pathing = PathingService::new(8192, 256);
        let coordinator = MoveCoordinator::new(&mut pathing, &map, &occ, 11);
        let min = if case == "minimum-range" { 10.0 } else { 0.0 };
        let goal = if case == "large-jump" {
            (630.0, 600.0)
        } else {
            (428.6, 600.0)
        };
        assert!(
            !coordinator.track_pursuit_endpoint(
                &mut entities,
                attacker,
                target,
                goal,
                min,
                54.0
            ),
            "{case}"
        );
        assert_eq!(
            entities
                .get(attacker)
                .unwrap()
                .movement
                .as_ref()
                .unwrap()
                .path,
            before,
            "{case}"
        );
    }
}
