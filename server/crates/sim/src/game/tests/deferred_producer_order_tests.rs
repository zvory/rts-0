use super::fixtures::empty_flat_game;
use super::*;
use crate::game::entity::EntityKind;
use crate::game::services::occupancy::footprint_center;

fn players() -> [PlayerInit; 2] {
    [
        PlayerInit {
            id: 1,
            team_id: 1,
            faction_id: "kriegsia".to_string(),
            name: "One".to_string(),
            color: "#fff".to_string(),
            is_ai: false,
        },
        PlayerInit {
            id: 2,
            team_id: 2,
            faction_id: "kriegsia".to_string(),
            name: "Two".to_string(),
            color: "#000".to_string(),
            is_ai: false,
        },
    ]
}

fn spawn_building(game: &mut Game, owner: u32, kind: EntityKind, tile: (u32, u32)) {
    let (x, y) = footprint_center(&game.state.map, kind, tile.0, tile.1);
    game.state
        .entities
        .spawn_building(owner, kind, x, y, true)
        .expect("building should spawn");
}

#[test]
fn unfinished_producer_repeat_begins_after_completion() {
    let mut game = empty_flat_game(&players());
    spawn_building(&mut game, 1, EntityKind::ResourceDepot, (3, 3));
    spawn_building(&mut game, 2, EntityKind::ResourceDepot, (50, 50));
    let (x, y) = footprint_center(&game.state.map, EntityKind::Barracks, 8, 8);
    let barracks = game
        .state
        .entities
        .spawn_building(1, EntityKind::Barracks, x, y, false)
        .expect("unfinished barracks should spawn");
    game.state.players[0].set_resources(1_000, 1_000);

    game.enqueue(
        1,
        Command::AdjustProductionRepeat {
            buildings: vec![barracks],
            unit: EntityKind::Rifleman,
            delta: 1,
        },
    );
    game.tick();

    let producer = game.state.entities.get(barracks).expect("barracks");
    assert!(producer.under_construction());
    assert_eq!(producer.repeat_production(), Some(EntityKind::Rifleman));
    assert!(producer.prod_queue().is_empty());

    let producer = game.state.entities.get_mut(barracks).expect("barracks");
    assert!(producer.set_construction_progress(u32::MAX));
    assert_eq!(producer.advance_construction(), Some(true));
    game.tick();

    let queued = game
        .state
        .entities
        .get(barracks)
        .expect("completed barracks")
        .prod_queue();
    assert_eq!(
        queued.first().map(|item| item.unit),
        Some(EntityKind::Rifleman)
    );
}
