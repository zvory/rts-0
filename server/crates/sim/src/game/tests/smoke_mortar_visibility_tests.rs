use super::fixtures::*;
use super::*;

fn mortar_launch_impact(
    events: &[(u32, Vec<Event>)],
    player_id: u32,
    mortar: u32,
) -> Option<(f32, f32)> {
    events
        .iter()
        .find(|(id, _)| *id == player_id)
        .and_then(|(_, player_events)| {
            player_events.iter().find_map(|event| match event {
                Event::MortarLaunch {
                    from, to_x, to_y, ..
                } if *from == mortar => Some((*to_x, *to_y)),
                _ => None,
            })
        })
}

#[test]
fn hidden_mortar_launch_is_not_sent_but_impact_reveals_attacker_to_victim() {
    let players = [
        PlayerInit {
            id: 1,
            team_id: 1,
            faction_id: "kriegsia".to_string(),
            name: "One".into(),
            color: "#fff".into(),
            is_ai: false,
        },
        PlayerInit {
            id: 2,
            team_id: 2,
            faction_id: "kriegsia".to_string(),
            name: "Two".into(),
            color: "#000".into(),
            is_ai: false,
        },
    ];
    let mut game = empty_flat_game(&players);
    let mortar_pos = game.state.map.tile_center(8, 8);
    let rifleman_sight = config::unit_stats(EntityKind::Rifleman)
        .expect("rifleman should have stats")
        .sight_tiles;
    let tank_sight = config::unit_stats(EntityKind::Tank)
        .expect("tank should have stats")
        .sight_tiles;
    let target_tile = 8 + rifleman_sight.max(tank_sight) + 1;
    let target_pos = game.state.map.tile_center(target_tile, 8);
    let mortar = game
        .state
        .entities
        .spawn_unit(1, EntityKind::MortarTeam, mortar_pos.0, mortar_pos.1)
        .expect("mortar should spawn");
    game.state
        .entities
        .get_mut(mortar)
        .expect("mortar should exist")
        .set_weapon_setup(WeaponSetup::Deployed);
    let target = game
        .state
        .entities
        .spawn_unit(2, EntityKind::Rifleman, target_pos.0, target_pos.1)
        .expect("target should spawn");
    let counter = game
        .state
        .entities
        .spawn_unit(
            2,
            EntityKind::Tank,
            target_pos.0,
            target_pos.1 + config::TILE_SIZE as f32 * 8.0,
        )
        .expect("counter tank should spawn");
    systems::recompute_supply(&mut game.state.players, &game.state.entities);
    game.rebuild_final_spatial();
    let ids: Vec<u32> = game.state.players.iter().map(|p| p.id).collect();
    game.state
        .fog
        .recompute(&ids, &game.state.entities, &game.state.map);
    assert!(
        !game
            .state
            .fog
            .is_visible_world(2, mortar_pos.0, mortar_pos.1),
        "test setup requires the mortar to be hidden before it fires"
    );

    game.enqueue(
        1,
        Command::UseAbility {
            ability: ability::AbilityKind::MortarFire,
            units: vec![mortar],
            x: Some(target_pos.0),
            y: Some(target_pos.1),
            queued: false,
        },
    );
    let accepted_events = game.tick();
    let impact_pos = mortar_launch_impact(&accepted_events, 1, mortar)
        .expect("owner should receive hidden mortar launch impact");
    let enemy_events = accepted_events
        .iter()
        .find(|(player_id, _)| *player_id == 2)
        .map(|(_, events)| events.as_slice())
        .unwrap_or(&[]);
    assert!(
        enemy_events
            .iter()
            .all(|event| !matches!(event, Event::MortarLaunch { .. })),
        "hidden mortar launch should not leak dust/recoil/shell data: {enemy_events:?}"
    );

    game.state
        .entities
        .get_mut(target)
        .expect("target should still exist")
        .set_position(impact_pos.0, impact_pos.1);
    let hp_before_impact = game
        .state
        .entities
        .get(target)
        .expect("target should still exist")
        .hp;
    let mut impact_events = Vec::new();
    for _ in 0..config::MORTAR_SHELL_DELAY_TICKS {
        impact_events = game.tick();
    }
    assert!(
        game.state
            .entities
            .get(target)
            .is_none_or(|target_after| target_after.hp < hp_before_impact),
        "test setup requires the mortar impact to damage the victim"
    );
    let enemy_events = impact_events
        .iter()
        .find(|(player_id, _)| *player_id == 2)
        .map(|(_, events)| events.as_slice())
        .unwrap_or(&[]);
    assert!(
        enemy_events.iter().any(|event| matches!(
            event,
            Event::MortarImpact {
                from: Some(from),
                reveal: Some(reveal),
                x,
                y,
                ..
            } if *from == mortar
                && reveal.kind == kinds::MORTAR_TEAM
                && (*x - impact_pos.0).abs() < 0.001
                && (*y - impact_pos.1).abs() < 0.001
        )),
        "victim should receive a mortar impact reveal after being hit: {enemy_events:?}"
    );
    let snapshot = game.snapshot_for(2);
    let view = snapshot
        .entities
        .iter()
        .find(|entity| entity.id == mortar)
        .expect("impact-revealed mortar should be projected to the victim");
    assert!(
        !view.vision_only,
        "mortar firing reveal should be actionable live fog"
    );

    game.enqueue(
        2,
        Command::Attack {
            units: vec![counter],
            target: mortar,
            queued: false,
        },
    );
    game.tick();

    assert_eq!(
        game.state
            .entities
            .get(counter)
            .expect("counter tank should exist")
            .order()
            .attack_target(),
        Some(mortar),
        "victim should be able to target the impact-revealed mortar"
    );
}
