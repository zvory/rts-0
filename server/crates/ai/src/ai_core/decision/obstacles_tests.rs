use super::*;
use crate::ai_core::profiles::JEFFS_AI;
use rts_sim::game::command::SimCommand;

fn unit(id: u32, owner: u32, kind: EntityKind, tile: (f32, f32)) -> AiEntitySummary {
    AiEntitySummary {
        id,
        owner,
        kind,
        x: tile.0 * 32.0,
        y: tile.1 * 32.0,
        hp: 60,
        state: AiEntityState::Idle,
        is_complete: true,
        production_queue_len: None,
        production_kind: None,
        latched_node: None,
        target_id: None,
        free_for_combat: true,
    }
}

fn trap(id: u32, tile: (f32, f32)) -> AiEntitySummary {
    unit(id, 0, EntityKind::TankTrap, tile)
}

fn classic(player: u32) -> (AiObservation, AiMapAnalysis) {
    use rts_sim::game::map::Map;
    use rts_sim::game::{Game, PlayerInit};
    let players: Vec<_> = (1..=2)
        .map(|id| PlayerInit {
            id,
            team_id: id,
            faction_id: "kriegsia".into(),
            name: format!("P{id}"),
            color: "#ffffff".into(),
            is_ai: true,
        })
        .collect();
    let map = Map::load_for_players("Classic", &[(1, 1), (2, 2)], 0x1234_5678).unwrap();
    let game = Game::new_with_random_ai_profiles_and_map_metadata(
        &players,
        0x1234_5678,
        map,
        Map::metadata_for_name("Classic").unwrap(),
    );
    let start = game.start_payload();
    let observation = AiObservation::from_snapshot_with_alive(
        &start,
        &game.snapshot_for(player),
        player,
        [],
        None,
    )
    .unwrap();
    (observation, AiMapAnalysis::analyze(&start))
}

#[test]
fn a_push_clears_a_trap_across_its_way_only_while_nothing_hostile_is_near() {
    let (mut observation, _) = classic(1);
    // Jeff's HQ in the west, the enemy's in the east; the push is on the enemy's side.
    observation.own_start_tile = (10, 41);
    let enemy_base = (120.5 * 32.0, 41.5 * 32.0);
    let tanks = [901, 902, 903];
    for (index, id) in tanks.iter().enumerate() {
        observation
            .owned
            .push(unit(*id, 1, EntityKind::Tank, (80.0, 40.0 + index as f32)));
    }
    let destination = (110.0 * 32.0, 41.0 * 32.0);
    observation.visible_tank_traps = vec![trap(500, (86.0, 41.0))];
    assert_eq!(
        trap_across_push(&observation, &tanks, destination, enemy_base),
        Some(500)
    );

    // Behind the push, or off to the side of its way: left alone.
    observation.visible_tank_traps = vec![trap(501, (74.0, 41.0)), trap(502, (86.0, 49.0))];
    assert_eq!(
        trap_across_push(&observation, &tanks, destination, enemy_base),
        None
    );

    // An enemy Rifleman near the trap makes it unsafe.
    observation.visible_tank_traps = vec![trap(500, (86.0, 41.0))];
    observation
        .visible_enemies
        .push(unit(700, 2, EntityKind::Rifleman, (92.0, 41.0)));
    assert_eq!(
        trap_across_push(&observation, &tanks, destination, enemy_base),
        None
    );
}

#[test]
fn a_push_leaves_traps_on_jeffs_own_side_of_the_map() {
    let (mut observation, _) = classic(1);
    observation.own_start_tile = (10, 41);
    let enemy_base = (120.5 * 32.0, 41.5 * 32.0);
    let tanks = [901, 902, 903];
    for (index, id) in tanks.iter().enumerate() {
        observation
            .owned
            .push(unit(*id, 1, EntityKind::Tank, (40.0, 40.0 + index as f32)));
    }
    // Straight across the push's way, but nearer Jeff's HQ than the enemy's: it may be keeping
    // enemy armor out, so the push goes round it.
    observation.visible_tank_traps = vec![trap(500, (46.0, 41.0))];
    assert_eq!(
        trap_across_push(&observation, &tanks, (70.0 * 32.0, 41.0 * 32.0), enemy_base),
        None
    );
}

#[test]
fn home_tanks_clear_a_trap_on_the_mains_way_out_while_the_base_is_quiet() {
    let (mut observation, analysis) = classic(1);
    let hq = tile_center(observation.own_start_tile, observation.map.tile_size);
    let route = analysis.base_route_tiles(1).unwrap();
    // The route tile about ten tiles out from the HQ.
    let out = route
        .iter()
        .min_by(|left, right| {
            let from_hq = |tile: &&crate::ai_core::map_analysis::AiTile| {
                let point = tile_center((tile.x, tile.y), 32);
                (dist2(point.0, point.1, hq.0, hq.1).sqrt() / 32.0 - 10.0).abs()
            };
            from_hq(left).total_cmp(&from_hq(right))
        })
        .unwrap();
    let hq_tile = (hq.0 / 32.0, hq.1 / 32.0);
    for (index, id) in [901u32, 902, 903].iter().enumerate() {
        observation.owned.push(unit(
            *id,
            1,
            EntityKind::Tank,
            (hq_tile.0 + index as f32, hq_tile.1 + 3.0),
        ));
    }
    observation.visible_tank_traps = vec![trap(500, (out.x as f32 + 0.5, out.y as f32 + 0.5))];

    let facts = AiFacts::from_observation(&observation);
    let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);
    let mut actions = AiActionContext::new(&facts, SpendBudget::new(0, 0, 0, 100));
    let sent = clear_route_traps(
        &mut actions,
        &observation,
        &mut memory,
        Some(&analysis),
        &BTreeSet::new(),
    )
    .unwrap();
    assert_eq!(sent.len(), 2, "two Tanks go, the rest stay home");
    assert!(actions
        .into_commands()
        .iter()
        .any(|command| matches!(command, SimCommand::ClearObstacleArea { target: 500, .. })));

    // The order stands; it is not given again straight away.
    let mut actions = AiActionContext::new(&facts, SpendBudget::new(0, 0, 0, 100));
    assert!(clear_route_traps(
        &mut actions,
        &observation,
        &mut memory,
        Some(&analysis),
        &BTreeSet::new()
    )
    .is_none());

    // An enemy Tank near the trap: nobody goes.
    let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);
    observation.visible_enemies.push(unit(
        700,
        2,
        EntityKind::Tank,
        (out.x as f32 + 6.0, out.y as f32),
    ));
    let mut actions = AiActionContext::new(&facts, SpendBudget::new(0, 0, 0, 100));
    assert!(clear_route_traps(
        &mut actions,
        &observation,
        &mut memory,
        Some(&analysis),
        &BTreeSet::new()
    )
    .is_none());
}
