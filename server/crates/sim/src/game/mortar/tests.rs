use super::*;
use crate::game::map::Map;
use crate::protocol::terrain;

fn open_map(size: u32) -> Map {
    Map {
        size,
        terrain: vec![terrain::GRASS; (size * size) as usize],
        starts: vec![(4, 4), (size - 5, size - 5)],
        base_sites: Vec::new(),
    }
}

fn visible_team_fog(map: &Map, entities: &EntityStore) -> Fog {
    let mut fog = Fog::new(map.size);
    fog.recompute(&[1, 2, 3], entities, map);
    fog
}

fn has_under_attack_notice(events: &HashMap<u32, Vec<Event>>, player: u32) -> bool {
    events.get(&player).is_some_and(|player_events| {
        player_events
            .iter()
            .any(|event| matches!(event, Event::Notice { msg, .. } if msg == "alert:under_attack"))
    })
}

#[test]
fn half_turn_completes_in_two_hundred_ms() {
    assert_eq!(
        HALF_TURN_TICKS * 1000 / config::TICK_HZ,
        200,
        "mortar half-turn timing should stay at 200 ms"
    );

    let mut entities = EntityStore::new();
    let mortar_id = entities
        .spawn_unit(1, EntityKind::MortarTeam, 100.0, 100.0)
        .expect("mortar should spawn");
    if let Some(mortar) = entities.get_mut(mortar_id) {
        mortar.set_facing(0.0);
        mortar.set_weapon_facing(0.0);
    }

    let target_angle = std::f32::consts::PI;
    for tick in 1..=HALF_TURN_TICKS {
        let ready = {
            let mortar = entities.get_mut(mortar_id).expect("mortar should exist");
            rotate_mortar_for_fire(mortar, target_angle)
        };
        if tick < HALF_TURN_TICKS {
            assert!(
                !ready,
                "mortar should still be rotating on half-turn tick {tick}"
            );
        } else {
            assert!(ready, "mortar should complete a 180-degree turn in 200 ms");
        }
    }

    let mortar = entities.get(mortar_id).expect("mortar should exist");
    assert!(
        angle_delta(mortar.facing(), target_angle).abs() <= FIRE_TOLERANCE_RAD + 0.001,
        "mortar should finish the half-turn aligned with the target, got {:.4}",
        mortar.facing()
    );
}

#[test]
fn mortar_under_attack_notice_goes_to_victim_owner_not_teammate() {
    let map = open_map(20);
    let mut entities = EntityStore::new();
    entities
        .spawn_unit(2, EntityKind::Worker, 160.0, 160.0)
        .expect("victim should spawn");
    entities
        .spawn_unit(3, EntityKind::Worker, 176.0, 160.0)
        .expect("victim ally should spawn");
    let fog = visible_team_fog(&map, &entities);
    let teams = TeamRelations::from_player_teams([(1, 1), (2, 7), (3, 7)]);
    let mut events = HashMap::from([(1, Vec::new()), (2, Vec::new()), (3, Vec::new())]);

    push_under_attack_notice(&mut events, &teams, &fog, 1, 2, 160.0, 160.0);

    assert!(has_under_attack_notice(&events, 2));
    assert!(!has_under_attack_notice(&events, 3));
    assert!(!has_under_attack_notice(&events, 1));
}
