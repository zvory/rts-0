use super::fixtures::empty_flat_game;
use super::*;
use crate::game::entity::{EntityKind, ProdItem};
use crate::game::services::occupancy::footprint_center;
use crate::game::upgrade::UpgradeKind;
use crate::rules;

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

fn spawn_building(game: &mut Game, owner: u32, kind: EntityKind, tile: (u32, u32)) -> u32 {
    let (x, y) = footprint_center(&game.state.map, kind, tile.0, tile.1);
    game.state
        .entities
        .spawn_building(owner, kind, x, y, true)
        .expect("building should spawn")
}

fn repeat_fixture() -> (Game, u32) {
    let mut game = empty_flat_game(&players());
    spawn_building(&mut game, 1, EntityKind::ResourceDepot, (3, 3));
    spawn_building(&mut game, 2, EntityKind::ResourceDepot, (50, 50));
    let barracks = spawn_building(&mut game, 1, EntityKind::Barracks, (8, 8));
    game.state
        .entities
        .get_mut(barracks)
        .expect("barracks")
        .set_repeat_production(Some(EntityKind::Rifleman), true);
    game.state.players[0].auto_build = AutoBuildSettings {
        paused: false,
        reserve_steel: 0,
        reserve_oil: 0,
    };
    systems::recompute_supply(&mut game.state.players, &game.state.entities);
    (game, barracks)
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

    let producer = game
        .state
        .entities
        .get_mut(barracks)
        .expect("barracks");
    assert!(producer.set_construction_progress(u32::MAX));
    assert_eq!(producer.advance_construction(), Some(true));
    game.tick();

    let queued = game
        .state
        .entities
        .get(barracks)
        .expect("completed barracks")
        .prod_queue();
    assert_eq!(queued.first().map(|item| item.unit), Some(EntityKind::Rifleman));
}

#[test]
fn depot_completes_steel_mine_on_nearest_in_range_patch() {
    let mut game = empty_flat_game(&players());
    let depot = spawn_building(&mut game, 1, EntityKind::ResourceDepot, (10, 10));
    spawn_building(&mut game, 2, EntityKind::ResourceDepot, (50, 50));
    game.state
        .entities
        .get_mut(depot)
        .expect("depot")
        .push_production(ProdItem {
            unit: EntityKind::SteelMine,
            progress: 1,
            total: 1,
            paid: true,
        });
    let near = game.state.map.tile_center(14, 10);
    let far = game.state.map.tile_center(18, 10);
    game.state
        .entities
        .spawn_node(EntityKind::Steel, far.0, far.1);
    game.state
        .entities
        .spawn_node(EntityKind::Steel, near.0, near.1);

    game.tick();

    assert!(game
        .state
        .entities
        .get(depot)
        .expect("depot")
        .prod_queue()
        .is_empty());
    let mine = game
        .state
        .entities
        .iter()
        .find(|entity| entity.kind == EntityKind::SteelMine)
        .expect("steel mine should be produced");
    assert_eq!((mine.pos_x, mine.pos_y), near);
}

#[test]
fn depot_extractor_scaffold_matches_front_queue_progress() {
    let mut game = empty_flat_game(&players());
    let depot = spawn_building(&mut game, 1, EntityKind::ResourceDepot, (10, 10));
    spawn_building(&mut game, 2, EntityKind::ResourceDepot, (50, 50));
    let patch = game.state.map.tile_center(14, 10);
    let total = config::building_stats(EntityKind::SteelMine)
        .expect("steel mine stats")
        .build_ticks;
    game.state
        .entities
        .spawn_node(EntityKind::Steel, patch.0, patch.1);
    game.state
        .entities
        .get_mut(depot)
        .expect("depot")
        .push_production(ProdItem {
            unit: EntityKind::SteelMine,
            progress: 0,
            total,
            paid: true,
        });

    game.tick();

    let queue_progress = game.state.entities.get(depot).expect("depot").prod_queue()[0].progress;
    let mine = game
        .state
        .entities
        .iter()
        .find(|entity| entity.kind == EntityKind::SteelMine)
        .expect("visible extractor scaffold");
    assert!(mine.under_construction());
    assert_eq!((mine.pos_x, mine.pos_y), patch);
    assert_eq!(mine.construction_producer_id(), Some(depot));
    assert_eq!(
        mine.build_progress_fraction(),
        Some(queue_progress as f32 / total as f32)
    );
}

#[test]
fn cancelling_legacy_extractor_scaffold_restarts_free_without_refund() {
    let mut game = empty_flat_game(&players());
    let depot = spawn_building(&mut game, 1, EntityKind::ResourceDepot, (10, 10));
    spawn_building(&mut game, 2, EntityKind::ResourceDepot, (50, 50));
    let patch = game.state.map.tile_center(14, 10);
    game.state
        .entities
        .spawn_node(EntityKind::Steel, patch.0, patch.1);
    game.state.players[0].set_resources(450, 0);
    game.state
        .entities
        .get_mut(depot)
        .expect("depot")
        .push_production(ProdItem {
            unit: EntityKind::SteelMine,
            progress: 0,
            total: 10,
            paid: true,
        });
    game.tick();
    let scaffold = game
        .state
        .entities
        .iter()
        .find(|entity| entity.construction_producer_id() == Some(depot))
        .map(|entity| entity.id)
        .expect("extractor scaffold");

    game.enqueue(
        1,
        Command::Cancel {
            building: scaffold,
            construction: true,
        },
    );
    game.tick();

    assert!(game.state.entities.get(scaffold).is_none());
    assert!(game
        .state
        .entities
        .get(depot)
        .expect("depot")
        .prod_queue()
        .is_empty());
    assert_eq!(game.state.players[0].steel, 450);
}

#[test]
fn automatic_extractor_pauses_when_saturated_and_restarts_free_after_destruction() {
    let mut game = empty_flat_game(&players());
    let depot = spawn_building(&mut game, 1, EntityKind::ResourceDepot, (10, 10));
    spawn_building(&mut game, 2, EntityKind::ResourceDepot, (50, 50));
    let patch = game.state.map.tile_center(14, 10);
    game.state
        .entities
        .spawn_node(EntityKind::Steel, patch.0, patch.1);
    let mine = game
        .state
        .entities
        .spawn_building(1, EntityKind::SteelMine, patch.0, patch.1, true)
        .expect("starting mine should spawn");
    game.state.players[0].set_resources(500, 0);

    game.tick();
    assert_eq!(game.state.players[0].steel, 500);
    assert!(game
        .state
        .entities
        .iter()
        .all(|entity| { entity.kind != EntityKind::SteelMine || !entity.under_construction() }));

    game.state.entities.remove(mine);
    game.tick();
    assert_eq!(game.state.players[0].steel, 500);
    let scaffold = game
        .state
        .entities
        .iter()
        .find(|entity| entity.kind == EntityKind::SteelMine && entity.under_construction())
        .expect("the permanent free job should restart the mine");
    assert_eq!(scaffold.construction_producer_id(), Some(depot));
    assert_eq!(
        scaffold.build_progress_fraction(),
        Some(1.0 / (config::TICK_HZ * 24) as f32)
    );
}

#[test]
fn depot_builds_free_steel_and_oil_extractors_concurrently() {
    let mut game = empty_flat_game(&players());
    let _depot = spawn_building(&mut game, 1, EntityKind::ResourceDepot, (10, 10));
    spawn_building(&mut game, 2, EntityKind::ResourceDepot, (50, 50));
    let steel_patch = game.state.map.tile_center(14, 10);
    let oil_patch = game.state.map.tile_center(16, 10);
    game.state
        .entities
        .spawn_node(EntityKind::Steel, steel_patch.0, steel_patch.1);
    game.state
        .entities
        .spawn_node(EntityKind::Oil, oil_patch.0, oil_patch.1);
    game.state.players[0].set_resources(0, 0);

    game.tick();
    let progress = |kind| {
        game.state
            .entities
            .iter()
            .find(|entity| entity.kind == kind && entity.under_construction())
            .and_then(|entity| entity.build_progress_fraction())
    };
    let expected_first_tick = 1.0 / (config::TICK_HZ * 24) as f32;
    assert_eq!(progress(EntityKind::SteelMine), Some(expected_first_tick));
    assert_eq!(progress(EntityKind::PumpJack), Some(expected_first_tick));
    assert_eq!(
        (game.state.players[0].steel, game.state.players[0].oil),
        (0, 0)
    );
    for _ in 1..config::TICK_HZ * 24 {
        game.tick();
    }
    for kind in [EntityKind::SteelMine, EntityKind::PumpJack] {
        assert!(game
            .state
            .entities
            .iter()
            .any(|entity| entity.kind == kind && !entity.under_construction()));
    }
}

#[test]
fn extractor_production_ignores_matching_patches_outside_its_depot_range() {
    let mut game = empty_flat_game(&players());
    let depot = spawn_building(&mut game, 1, EntityKind::ResourceDepot, (4, 4));
    spawn_building(&mut game, 2, EntityKind::ResourceDepot, (50, 50));
    let patch = game.state.map.tile_center(25, 25);
    game.state
        .entities
        .spawn_node(EntityKind::Oil, patch.0, patch.1);
    game.state
        .entities
        .get_mut(depot)
        .expect("depot")
        .push_production(ProdItem {
            unit: EntityKind::PumpJack,
            progress: 0,
            total: 1,
            paid: false,
        });
    game.state.players[0].set_resources(500, 0);

    game.tick();

    assert_eq!(game.state.players[0].steel, 500);
    assert!(!game.state.entities.get(depot).expect("depot").prod_queue()[0].paid);
    assert!(!game
        .state
        .entities
        .iter()
        .any(|entity| entity.kind == EntityKind::PumpJack));
}

#[test]
fn auto_build_defaults_to_running_without_resource_floors() {
    assert_eq!(
        AutoBuildSettings::default(),
        AutoBuildSettings {
            paused: false,
            reserve_steel: 0,
            reserve_oil: 0,
        }
    );
}

#[test]
fn worker_auto_build_ignores_unspent_oil_reserve() {
    let mut game = empty_flat_game(&players());
    let resource_depot = spawn_building(&mut game, 1, EntityKind::ResourceDepot, (3, 3));
    spawn_building(&mut game, 2, EntityKind::ResourceDepot, (50, 50));
    game.state
        .entities
        .get_mut(resource_depot)
        .expect("resource depot")
        .set_repeat_production(Some(EntityKind::Worker), true);
    let cost = rules::economy::resource_cost(EntityKind::Worker);
    game.state.players[0].auto_build = AutoBuildSettings::default();
    let reserve_steel = game.state.players[0].auto_build.reserve_steel;
    game.state.players[0].set_resources(cost.steel.saturating_add(reserve_steel), 0);
    systems::recompute_supply(&mut game.state.players, &game.state.entities);

    game.tick();

    let queue = game
        .state
        .entities
        .get(resource_depot)
        .expect("resource depot")
        .prod_queue();
    assert_eq!(queue.len(), 1);
    assert_eq!(queue[0].unit, EntityKind::Worker);
    assert!(queue[0].paid);
    assert_eq!(
        (game.state.players[0].steel, game.state.players[0].oil),
        (reserve_steel, 0)
    );
}

#[test]
fn auto_build_settings_pause_and_preserve_resource_floors() {
    let (mut game, barracks) = repeat_fixture();
    let cost = rules::economy::resource_cost(EntityKind::Rifleman);
    game.enqueue(
        1,
        SimCommand::SetAutoBuildSettings {
            paused: true,
            reserve_steel: 200,
            reserve_oil: 100,
        },
    );
    game.state.players[0]
        .set_resources(cost.steel.saturating_add(200), cost.oil.saturating_add(100));

    game.tick();
    assert!(game
        .state
        .entities
        .get(barracks)
        .expect("barracks")
        .prod_queue()
        .is_empty());
    assert!(
        game.snapshot_for(1)
            .auto_build
            .expect("owner settings")
            .paused
    );

    game.enqueue(
        1,
        SimCommand::SetAutoBuildSettings {
            paused: false,
            reserve_steel: 200,
            reserve_oil: 100,
        },
    );
    game.tick();

    assert_eq!(
        game.state
            .entities
            .get(barracks)
            .expect("barracks")
            .prod_queue()
            .len(),
        1
    );
    assert_eq!(game.state.players[0].steel, 200);
    assert_eq!(game.state.players[0].oil, 100);
    assert_eq!(
        game.snapshot_for(1).auto_build,
        Some(crate::protocol::AutoBuildSettingsSnapshot {
            paused: false,
            reserve_steel: 200,
            reserve_oil: 100,
        })
    );
}

#[test]
fn auto_build_waits_when_either_resource_would_cross_its_floor() {
    let (mut game, barracks) = repeat_fixture();
    let cost = rules::economy::resource_cost(EntityKind::Rifleman);
    game.state.players[0].auto_build = AutoBuildSettings {
        paused: false,
        reserve_steel: 200,
        reserve_oil: 100,
    };
    game.state.players[0]
        .set_resources(cost.steel.saturating_add(199), cost.oil.saturating_add(100));

    game.tick();

    assert!(game
        .state
        .entities
        .get(barracks)
        .expect("barracks")
        .prod_queue()
        .is_empty());
    assert_eq!(
        (game.state.players[0].steel, game.state.players[0].oil),
        (cost.steel.saturating_add(199), cost.oil.saturating_add(100))
    );
}

#[test]
fn auto_build_resource_floors_do_not_block_manual_training() {
    let (mut game, barracks) = repeat_fixture();
    let cost = rules::economy::resource_cost(EntityKind::Rifleman);
    game.state.players[0].auto_build = AutoBuildSettings::default();
    game.state.players[0].set_resources(cost.steel, cost.oil);
    game.enqueue(
        1,
        SimCommand::Train {
            building: barracks,
            unit: EntityKind::Rifleman,
        },
    );

    game.tick();

    let queue = game
        .state
        .entities
        .get(barracks)
        .expect("barracks")
        .prod_queue();
    assert_eq!(queue.len(), 1);
    assert!(queue[0].paid);
    assert_eq!(
        (game.state.players[0].steel, game.state.players[0].oil),
        (0, 0)
    );
}

#[test]
fn auto_build_settings_do_not_change_ai_repeat_production() {
    let (mut game, barracks) = repeat_fixture();
    let cost = rules::economy::resource_cost(EntityKind::Rifleman);
    game.state.players[0].is_ai = true;
    game.state.players[0].auto_build = AutoBuildSettings {
        paused: true,
        reserve_steel: 9_950,
        reserve_oil: 9_950,
    };
    game.state.players[0].set_resources(cost.steel, cost.oil);

    game.tick();

    let queue = game
        .state
        .entities
        .get(barracks)
        .expect("barracks")
        .prod_queue();
    assert_eq!(queue.len(), 1);
    assert!(queue[0].paid);
    assert_eq!(
        (game.state.players[0].steel, game.state.players[0].oil),
        (0, 0)
    );
}

#[test]
fn repeat_production_retries_then_charges_and_reserves_once() {
    let (mut game, barracks) = repeat_fixture();
    let cost = rules::economy::resource_cost(EntityKind::Rifleman);
    let supply = rules::economy::supply_cost(EntityKind::Rifleman);
    game.state.players[0].set_resources(cost.steel.saturating_sub(1), cost.oil);

    game.tick();
    assert!(game
        .state
        .entities
        .get(barracks)
        .expect("barracks")
        .prod_queue()
        .is_empty());
    assert_eq!(game.state.players[0].supply_used, 0);

    game.state.players[0].set_resources(cost.steel, cost.oil);
    game.tick();
    let queued = game
        .state
        .entities
        .get(barracks)
        .expect("barracks")
        .prod_queue();
    assert_eq!(queued.len(), 1);
    assert_eq!(queued[0].unit, EntityKind::Rifleman);
    assert_eq!(queued[0].progress, 1);
    assert_eq!(game.state.players[0].steel, 0);
    assert_eq!(game.state.players[0].oil, 0);
    assert_eq!(game.state.players[0].supply_used, supply);

    game.tick();
    assert_eq!(
        game.state
            .entities
            .get(barracks)
            .expect("barracks")
            .prod_queue()
            .len(),
        1
    );
    assert_eq!(game.state.players[0].supply_used, supply);
}

#[test]
fn repeat_production_revalidates_producer_compatibility() {
    let (mut game, barracks) = repeat_fixture();
    spawn_building(&mut game, 1, EntityKind::Factory, (14, 8));
    game.state
        .entities
        .get_mut(barracks)
        .expect("barracks")
        .set_repeat_production(None, false);
    game.state
        .entities
        .get_mut(barracks)
        .expect("barracks")
        .set_repeat_production(Some(EntityKind::Tank), true);
    game.state.players[0]
        .upgrades
        .insert(UpgradeKind::TankUnlock);
    let cost = rules::economy::resource_cost(EntityKind::Tank);
    game.state.players[0].set_resources(cost.steel, cost.oil);
    systems::recompute_supply(&mut game.state.players, &game.state.entities);

    game.tick();

    assert!(game
        .state
        .entities
        .get(barracks)
        .expect("barracks")
        .prod_queue()
        .is_empty());
    assert_eq!(game.state.players[0].steel, cost.steel);
    assert_eq!(game.state.players[0].oil, cost.oil);
    assert_eq!(game.state.players[0].supply_used, 0);
}

#[test]
fn repeat_production_alternates_enabled_units() {
    let (mut game, barracks) = repeat_fixture();
    spawn_building(&mut game, 1, EntityKind::TrainingCentre, (14, 8));
    game.state
        .entities
        .get_mut(barracks)
        .expect("barracks")
        .set_repeat_production(Some(EntityKind::MachineGunner), true);
    game.state.players[0].set_resources(10_000, 10_000);
    systems::recompute_supply(&mut game.state.players, &game.state.entities);

    for (index, expected) in [
        EntityKind::Rifleman,
        EntityKind::MachineGunner,
        EntityKind::Rifleman,
    ]
    .into_iter()
    .enumerate()
    {
        game.tick();
        if index == 0 {
            let repeat_kinds = game
                .snapshot_for(1)
                .entities
                .into_iter()
                .find(|entity| entity.id == barracks)
                .expect("barracks projection")
                .prod_repeat_kinds;
            assert_eq!(
                repeat_kinds,
                vec!["rifleman".to_string(), "machine_gunner".to_string()]
            );
        }
        assert_eq!(
            game.state
                .entities
                .get(barracks)
                .expect("barracks")
                .prod_queue()[0]
                .unit,
            expected
        );
        game.state
            .entities
            .get_mut(barracks)
            .expect("barracks")
            .remove_front_production();
    }
}

#[test]
fn disabling_repeat_units_preserves_the_next_unit() {
    let (mut game, barracks) = repeat_fixture();
    let producer = game.state.entities.get_mut(barracks).expect("barracks");
    producer.set_repeat_production(Some(EntityKind::MachineGunner), true);
    producer.set_repeat_production(Some(EntityKind::Rifleman), true);

    producer.set_repeat_production(None, true);
    assert_eq!(
        producer.repeat_production(),
        Some(EntityKind::MachineGunner)
    );

    producer.set_repeat_production(Some(EntityKind::Rifleman), false);
    assert_eq!(
        producer.repeat_production(),
        Some(EntityKind::MachineGunner),
        "removing a later unit must not move the cursor"
    );

    producer.set_repeat_production(Some(EntityKind::Rifleman), true);
    producer.set_repeat_production(Some(EntityKind::Worker), false);
    assert_eq!(
        producer.repeat_production(),
        Some(EntityKind::MachineGunner),
        "removing an earlier unit must preserve the cursor's semantic target"
    );

    producer.set_repeat_production(Some(EntityKind::MachineGunner), false);
    producer.set_repeat_production(Some(EntityKind::Rifleman), true);
    assert_eq!(
        &producer
            .production
            .as_ref()
            .expect("production")
            .repeat_units,
        &[EntityKind::Rifleman],
        "removing the current unit must select its successor without adding duplicates"
    );
    assert_eq!(producer.repeat_production(), Some(EntityKind::Rifleman));
}
