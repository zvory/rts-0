use super::fixtures::*;
use super::*;
use crate::game::entity::BuildPhase;

fn setup(faction: &str, steel: u32, oil: u32) -> (Game, u32) {
    let mut game = empty_flat_game(&[PlayerInit {
        id: 1,
        team_id: 1,
        faction_id: faction.into(),
        name: "Builder".into(),
        color: "#fff".into(),
        is_ai: false,
    }]);
    game.state.players[0].set_resources(steel, oil);
    let (x, y) = game.state.map.tile_center(9, 10);
    let worker = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Worker, x, y)
        .unwrap();
    game.rebuild_final_spatial();
    game.state
        .fog
        .recompute(&[1], &game.state.entities, &game.state.map);
    game.enqueue(
        1,
        Command::Build {
            units: vec![worker],
            building: EntityKind::Shop,
            tile_x: 10,
            tile_y: 10,
            queued: false,
        },
    );
    (game, worker)
}

fn start(game: &mut Game, worker: u32) -> u32 {
    for _ in 0..120 {
        game.tick();
        if let Some(BuildPhase::Constructing { site }) =
            game.state.entities.get(worker).unwrap().build_phase()
        {
            return site;
        }
    }
    panic!("Serf must start the Shop");
}

#[test]
fn shop_costs_both_resources_and_finishes_after_1200_work_ticks() {
    let (mut game, worker) = setup("cultivators", 200, 200);
    let site = start(&mut game, worker);
    assert_eq!(
        (game.state.players[0].steel, game.state.players[0].oil),
        (0, 0)
    );
    let building = game.state.entities.get(site).unwrap();
    assert_eq!(building.max_hp, 1000);
    assert_eq!(building.construction.as_ref().unwrap().total, 1200);
    let progress = building.construction.as_ref().unwrap().progress;
    for _ in progress..1199 {
        game.tick();
    }
    assert!(game.state.entities.get(site).unwrap().under_construction());
    game.tick();
    let building = game.state.entities.get(site).unwrap();
    assert!(!building.under_construction());
    assert_eq!(building.hp, 1000);
    assert!(building.production.is_none());
    assert!(building.combat.is_none());
    let restored = Game::restore_checkpoint_payload_text_for_test(
        &game.checkpoint_payload_text_for_test().unwrap(),
        game.state.map.clone(),
        game.state.map_metadata.clone(),
    )
    .unwrap();
    assert_eq!(
        restored.state.entities.get(site).unwrap().kind,
        EntityKind::Shop
    );
    game.state
        .entities
        .get_mut(site)
        .unwrap()
        .apply_damage(u32::MAX, None);
    game.tick();
    assert!(game.state.entities.get(site).is_none());
}

#[test]
fn shop_waits_for_either_missing_resource_and_rejects_other_factions() {
    for (faction, steel, oil) in [
        ("cultivators", 199, 200),
        ("cultivators", 200, 199),
        ("kriegsia", 1000, 1000),
    ] {
        let (mut game, _) = setup(faction, steel, oil);
        for _ in 0..120 {
            game.tick();
        }
        assert!(!game
            .state
            .entities
            .iter()
            .any(|e| e.kind == EntityKind::Shop));
        assert_eq!(
            (game.state.players[0].steel, game.state.players[0].oil),
            (steel, oil)
        );
    }
}

#[test]
fn shop_cancellation_uses_normal_construction_refund() {
    let (mut game, worker) = setup("cultivators", 200, 200);
    let site = start(&mut game, worker);
    game.enqueue(
        1,
        Command::Cancel {
            building: site,
            construction: true,
        },
    );
    game.tick();
    assert!(game.state.entities.get(site).is_none());
    assert_eq!(
        (game.state.players[0].steel, game.state.players[0].oil),
        (200, 200)
    );
}
