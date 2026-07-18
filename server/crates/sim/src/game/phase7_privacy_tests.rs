use super::*;
use crate::game::command::SimCommand as Command;
use crate::game::entity::{
    EntityKind, Order, ProdItem, RallyIntent, RallyKind, ResearchItem, WeaponSetup,
};
use crate::game::{systems, SmokeCloudStore};
use crate::protocol::{kinds, terrain, Event};

fn empty_flat_game(players: &[PlayerInit]) -> Game {
    let mut game = Game::new_for_replay(players, 0x1234_5678);
    for tile in &mut game.state.map.terrain {
        *tile = terrain::GRASS;
    }
    for id in game.state.entities.ids() {
        game.state.entities.remove(id);
    }
    game.state.smokes = SmokeCloudStore::new();
    game.state.mortar_shells = MortarShellStore::default();
    game.state.artillery_shells = artillery::ArtilleryShellStore::default();
    refresh_world(&mut game);
    game
}

fn refresh_world(game: &mut Game) {
    systems::recompute_supply(&mut game.state.players, &game.state.entities);
    game.rebuild_final_spatial();
    let ids: Vec<u32> = game.state.players.iter().map(|p| p.id).collect();
    game.state.fog.recompute_with_smoke(
        &ids,
        &game.state.entities,
        &game.state.map,
        &game.state.smokes,
    );
}

fn phase7_players() -> [PlayerInit; 3] {
    [
        PlayerInit {
            id: 1,
            team_id: 7,
            faction_id: "kriegsia".to_string(),
            name: "One".into(),
            color: "#fff".into(),
            is_ai: false,
        },
        PlayerInit {
            id: 2,
            team_id: 7,
            faction_id: "kriegsia".to_string(),
            name: "Two".into(),
            color: "#bbb".into(),
            is_ai: false,
        },
        PlayerInit {
            id: 3,
            team_id: 3,
            faction_id: "kriegsia".to_string(),
            name: "Three".into(),
            color: "#000".into(),
            is_ai: false,
        },
    ]
}

fn deploy_artillery_toward(game: &mut Game, artillery: u32, target: (f32, f32)) {
    let unit = game
        .state
        .entities
        .get_mut(artillery)
        .expect("artillery should exist");
    let facing = (target.1 - unit.pos_y).atan2(target.0 - unit.pos_x);
    unit.set_weapon_setup(WeaponSetup::Deployed);
    unit.set_emplacement_facing(Some(facing));
    unit.set_weapon_facing(facing);
}

#[test]
fn allied_snapshot_exposes_read_only_details_but_not_private_controls() {
    let mut game = empty_flat_game(&phase7_players());
    game.state
        .players
        .iter_mut()
        .find(|player| player.id == 2)
        .expect("ally player should exist")
        .upgrades
        .insert(upgrade::UpgradeKind::Methamphetamines);
    for (owner, tile) in [(1, (2, 2)), (2, (5, 2)), (3, (55, 55))] {
        let pos = game.state.map.tile_center(tile.0, tile.1);
        game.state
            .entities
            .spawn_building(owner, EntityKind::CityCentre, pos.0, pos.1, true)
            .expect("city centre should spawn");
    }

    let ally_city_pos = game.state.map.tile_center(15, 2);
    let ally_city_centre = game
        .state
        .entities
        .spawn_building(
            2,
            EntityKind::CityCentre,
            ally_city_pos.0,
            ally_city_pos.1,
            true,
        )
        .expect("ally producing city centre should spawn");
    {
        let building = game
            .state
            .entities
            .get_mut(ally_city_centre)
            .expect("city centre exists");
        building.push_production(ProdItem {
            unit: EntityKind::Worker,
            progress: 30,
            total: 120,
            paid: true,
        });
        building.push_production(ProdItem {
            unit: EntityKind::ScoutPlane,
            progress: 0,
            total: 600,
            paid: true,
        });
    }

    let barracks_pos = game.state.map.tile_center(7, 2);
    let barracks = game
        .state
        .entities
        .spawn_building(
            2,
            EntityKind::Barracks,
            barracks_pos.0,
            barracks_pos.1,
            true,
        )
        .expect("ally barracks should spawn");
    {
        let building = game
            .state
            .entities
            .get_mut(barracks)
            .expect("barracks exists");
        building.push_production(ProdItem {
            unit: EntityKind::Rifleman,
            progress: 30,
            total: 120,
            paid: true,
        });
        building.push_production(ProdItem {
            unit: EntityKind::MachineGunner,
            progress: 0,
            total: 180,
            paid: true,
        });
        building.set_rally_point(Some(RallyIntent::new(
            RallyKind::AttackMove,
            barracks_pos.0 + 64.0,
            barracks_pos.1,
        )));
    }

    let research_pos = game.state.map.tile_center(9, 2);
    let research = game
        .state
        .entities
        .spawn_building(
            2,
            EntityKind::ResearchComplex,
            research_pos.0,
            research_pos.1,
            true,
        )
        .expect("ally research complex should spawn");
    game.state
        .entities
        .get_mut(research)
        .expect("research complex exists")
        .push_research(ResearchItem {
            upgrade: upgrade::UpgradeKind::TankUnlock,
            progress: 60,
            total: 600,
            paid: true,
        });

    let scaffold_pos = game.state.map.tile_center(11, 2);
    let scaffold = game
        .state
        .entities
        .spawn_building(2, EntityKind::Depot, scaffold_pos.0, scaffold_pos.1, false)
        .expect("ally scaffold should spawn");
    game.state
        .entities
        .get_mut(scaffold)
        .expect("scaffold exists")
        .set_construction_progress(10);

    let mortar_pos = game.state.map.tile_center(13, 2);
    let target_pos = game.state.map.tile_center(55, 50);
    let mortar = game
        .state
        .entities
        .spawn_unit(2, EntityKind::MortarTeam, mortar_pos.0, mortar_pos.1)
        .expect("ally mortar should spawn");
    let hidden_enemy = game
        .state
        .entities
        .spawn_unit(3, EntityKind::Rifleman, target_pos.0, target_pos.1)
        .expect("hidden enemy should spawn");
    {
        let unit = game.state.entities.get_mut(mortar).expect("mortar exists");
        unit.set_weapon_setup(WeaponSetup::Deployed);
        unit.set_order(Order::attack(hidden_enemy));
        unit.set_target_id(Some(hidden_enemy));
        unit.set_weapon_facing(1.3);
    }

    refresh_world(&mut game);
    let snapshot = game.snapshot_for(1);
    let barracks_view = snapshot
        .entities
        .iter()
        .find(|entity| entity.id == barracks)
        .expect("ally barracks should be visible");
    assert_eq!(barracks_view.prod_kind.as_deref(), Some(kinds::RIFLEMAN));
    assert_eq!(barracks_view.prod_queue, Some(2));
    assert!(barracks_view.prod_progress.is_some());
    assert_eq!(barracks_view.rally, None);
    assert!(barracks_view.rally_plan.is_empty());
    assert!(barracks_view.order_plan.is_empty());

    let city_centre_view = snapshot
        .entities
        .iter()
        .find(|entity| entity.id == ally_city_centre)
        .expect("ally city centre should be visible");
    assert_eq!(city_centre_view.prod_kind.as_deref(), Some(kinds::WORKER));
    assert_eq!(city_centre_view.prod_queue, Some(2));
    assert!(
        city_centre_view.prod_scout_plane_queued,
        "allied production details include hidden Scout Plane queue presence"
    );

    let research_view = snapshot
        .entities
        .iter()
        .find(|entity| entity.id == research)
        .expect("ally research complex should be visible");
    assert_eq!(
        research_view.prod_upgrade.as_deref(),
        Some(upgrade::UpgradeKind::TankUnlock.to_protocol_str())
    );
    assert_eq!(research_view.prod_queue, Some(1));

    let scaffold_view = snapshot
        .entities
        .iter()
        .find(|entity| entity.id == scaffold)
        .expect("ally scaffold should be visible");
    assert!(scaffold_view.build_progress.is_some());

    let mortar_view = snapshot
        .entities
        .iter()
        .find(|entity| entity.id == mortar)
        .expect("ally mortar should be visible");
    assert_eq!(mortar_view.setup_state.as_deref(), Some("deployed"));
    assert_eq!(mortar_view.target_id, None);
    assert_eq!(mortar_view.weapon_facing, None);
    assert!(mortar_view.abilities.is_empty());
    assert!(snapshot
        .upgrades
        .iter()
        .all(|upgrade| upgrade != upgrade::UpgradeKind::Methamphetamines.to_protocol_str()));
    assert_eq!(snapshot.player_resources.len(), 0);
}

#[test]
fn full_world_snapshot_exposes_private_planning_for_all_projected_owners() {
    let mut game = empty_flat_game(&phase7_players());
    let observer_pos = game.state.map.tile_center(10, 10);
    game.state
        .entities
        .spawn_unit(1, EntityKind::Worker, observer_pos.0, observer_pos.1)
        .expect("observer worker should spawn");

    let barracks_pos = game.state.map.tile_center(12, 10);
    let barracks = game
        .state
        .entities
        .spawn_building(
            3,
            EntityKind::Barracks,
            barracks_pos.0,
            barracks_pos.1,
            true,
        )
        .expect("enemy barracks should spawn");
    let rally = game.state.map.tile_center(16, 10);
    game.state
        .entities
        .get_mut(barracks)
        .expect("enemy barracks should exist")
        .set_rally_point(Some(RallyIntent::new(RallyKind::Move, rally.0, rally.1)));

    let rifle_pos = game.state.map.tile_center(22, 12);
    let rifle = game
        .state
        .entities
        .spawn_unit(3, EntityKind::Rifleman, rifle_pos.0, rifle_pos.1)
        .expect("enemy rifleman should spawn");
    let move_goal = game.state.map.tile_center(16, 12);
    game.state
        .entities
        .get_mut(rifle)
        .expect("enemy rifleman should exist")
        .set_order(Order::move_to(move_goal.0, move_goal.1));

    let gun_pos = game.state.map.tile_center(24, 14);
    let gun = game
        .state
        .entities
        .spawn_unit(3, EntityKind::AntiTankGun, gun_pos.0, gun_pos.1)
        .expect("enemy anti-tank gun should spawn");
    let gun_facing = 1.125;
    {
        let gun = game
            .state
            .entities
            .get_mut(gun)
            .expect("enemy anti-tank gun should exist");
        gun.set_weapon_setup(WeaponSetup::Deployed);
        gun.set_emplacement_facing(Some(gun_facing));
        gun.set_weapon_facing(gun_facing);
    }

    refresh_world(&mut game);

    let normal = game.snapshot_for(1);
    let normal_barracks = normal
        .entities
        .iter()
        .find(|entity| entity.id == barracks)
        .expect("visible enemy barracks should project");
    assert!(normal_barracks.rally.is_none());
    assert!(normal_barracks.rally_plan.is_empty());
    let full_world = game.snapshot_full_for(1);
    let selected_owner = game.snapshot_for_observer(&super::ObserverView::Players(vec![3]));
    let selected_barracks = selected_owner
        .entities
        .iter()
        .find(|entity| entity.id == barracks)
        .expect("selected owner barracks should project");
    assert_eq!(selected_barracks.rally, Some([rally.0, rally.1]));
    assert_eq!(selected_owner.player_resources.len(), 1);
    assert_eq!(selected_owner.player_resources[0].id, 3);
    let full_barracks = full_world
        .entities
        .iter()
        .find(|entity| entity.id == barracks)
        .expect("full-world enemy barracks should project");
    assert_eq!(full_barracks.rally, Some([rally.0, rally.1]));
    assert_eq!(full_barracks.rally_plan.len(), 1);
    assert_eq!(full_barracks.rally_plan[0].kind, "move");
    assert_eq!(full_barracks.rally_plan[0].x, rally.0);
    assert_eq!(full_barracks.rally_plan[0].y, rally.1);

    let full_rifle = full_world
        .entities
        .iter()
        .find(|entity| entity.id == rifle)
        .expect("full-world enemy rifleman should project");
    assert_eq!(full_rifle.order_plan.len(), 1);
    assert_eq!(full_rifle.order_plan[0].kind, "move");
    assert_eq!(full_rifle.order_plan[0].x, move_goal.0);
    assert_eq!(full_rifle.order_plan[0].y, move_goal.1);

    let full_gun = full_world
        .entities
        .iter()
        .find(|entity| entity.id == gun)
        .expect("full-world enemy anti-tank gun should project");
    let projected_facing = full_gun
        .setup_facing
        .expect("full-world enemy setup facing should project");
    assert!((projected_facing - gun_facing).abs() < 0.0001);
    assert!(full_world.visible_tiles.iter().all(|visible| *visible == 1));
    assert!(full_world
        .explored_tiles
        .iter()
        .all(|explored| *explored == 1));
    assert!(selected_owner
        .explored_tiles
        .iter()
        .any(|explored| *explored == 0));
    assert_eq!(full_world.player_resources.len(), phase7_players().len());
}

#[test]
fn artillery_target_marker_is_visible_to_allies_not_hidden_enemies() {
    let mut game = empty_flat_game(&phase7_players());
    let pos = game.state.map.tile_center(10, 10);
    let target = game.state.map.tile_center(26, 10);
    let artillery = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Artillery, pos.0, pos.1)
        .expect("artillery should spawn");
    deploy_artillery_toward(&mut game, artillery, target);

    game.enqueue(
        1,
        Command::UseAbility {
            ability: ability::AbilityKind::PointFire,
            units: vec![artillery],
            x: Some(target.0),
            y: Some(target.1),
            queued: false,
        },
    );
    let events = game.tick();
    let ally_events = events
        .iter()
        .find(|(player_id, _)| *player_id == 2)
        .map(|(_, events)| events.as_slice())
        .unwrap_or(&[]);
    let enemy_events = events
        .iter()
        .find(|(player_id, _)| *player_id == 3)
        .map(|(_, events)| events.as_slice())
        .unwrap_or(&[]);

    assert!(ally_events
        .iter()
        .any(|event| matches!(event, Event::ArtilleryTarget { from, .. } if *from == artillery)));
    assert!(enemy_events
        .iter()
        .all(|event| !matches!(event, Event::ArtilleryTarget { .. } | Event::Attack { .. })));
}

#[test]
fn manual_mortar_launch_and_impact_markers_are_visible_to_allies() {
    let mut game = empty_flat_game(&phase7_players());
    let mortar_pos = game.state.map.tile_center(8, 8);
    let target_pos = game.state.map.tile_center(17, 8);
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
    game.state
        .entities
        .spawn_unit(3, EntityKind::Rifleman, target_pos.0, target_pos.1)
        .expect("target should spawn");
    refresh_world(&mut game);

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
    let launch_events = game.tick();
    let ally_launch = launch_events
        .iter()
        .find(|(player_id, _)| *player_id == 2)
        .map(|(_, events)| events.as_slice())
        .unwrap_or(&[]);
    let enemy_launch = launch_events
        .iter()
        .find(|(player_id, _)| *player_id == 3)
        .map(|(_, events)| events.as_slice())
        .unwrap_or(&[]);
    assert!(ally_launch
        .iter()
        .any(|event| matches!(event, Event::MortarLaunch { from, .. } if *from == mortar)));
    let launch_impact = ally_launch
        .iter()
        .find_map(|event| match event {
            Event::MortarLaunch {
                from, to_x, to_y, ..
            } if *from == mortar => Some((*to_x, *to_y)),
            _ => None,
        })
        .expect("ally should receive mortar launch impact");
    assert!(enemy_launch
        .iter()
        .all(|event| !matches!(event, Event::MortarLaunch { .. })));

    let mut impact_events = Vec::new();
    for _ in 0..config::MORTAR_SHELL_DELAY_TICKS {
        impact_events = game.tick();
    }
    let ally_impact = impact_events
        .iter()
        .find(|(player_id, _)| *player_id == 2)
        .map(|(_, events)| events.as_slice())
        .unwrap_or(&[]);
    assert!(ally_impact
        .iter()
        .any(|event| matches!(event, Event::MortarImpact { x, y, .. }
            if (*x - launch_impact.0).abs() < 0.001 && (*y - launch_impact.1).abs() < 0.001)));
}
