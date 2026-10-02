use super::*;
use crate::ai_core::observation::{AiEconomy, AiMapSummary, AiPlayerSummary};

const TILE: f32 = 32.0;

fn rifleman(id: u32, owner: u32, tile_x: f32, tile_y: f32, hp: u32) -> AiEntitySummary {
    AiEntitySummary {
        id,
        owner,
        kind: SQUAD_KIND,
        x: tile_x * TILE,
        y: tile_y * TILE,
        hp,
        state: AiEntityState::Idle,
        is_complete: true,
        production_queue_len: None,
        production_kind: None,
        latched_node: None,
        target_id: None,
        free_for_combat: true,
    }
}

fn observation(owned: Vec<AiEntitySummary>, enemies: Vec<AiEntitySummary>) -> AiObservation {
    AiObservation {
        player_id: 1,
        tick: 90,
        map: AiMapSummary {
            width: 64,
            height: 64,
            tile_size: TILE as u32,
        },
        economy: AiEconomy {
            steel: 0,
            oil: 0,
            supply_used: 4,
            supply_cap: 20,
        },
        own_start_tile: (4, 4),
        players: vec![
            AiPlayerSummary {
                id: 1,
                team_id: 1,
                start_tile: (4, 4),
                is_ai: true,
                is_alive: true,
            },
            AiPlayerSummary {
                id: 2,
                team_id: 2,
                start_tile: (60, 60),
                is_ai: true,
                is_alive: true,
            },
        ],
        owned,
        resources: Vec::new(),
        visible_allies: Vec::new(),
        visible_enemies: enemies,
        ability_states: Vec::new(),
        smokes: Vec::new(),
        visible_tank_traps: Vec::new(),
        pending_builds: Vec::new(),
        upgrades: Vec::new(),
    }
}

fn squad_ids(observation: &AiObservation) -> Vec<u32> {
    observation.owned.iter().map(|unit| unit.id).collect()
}

fn plan(observation: &AiObservation, params: &RifleSquadParams) -> Vec<SquadOrder> {
    plan_rifle_squad(
        observation,
        &squad_ids(observation),
        (60.0 * TILE, 60.0 * TILE),
        params,
        &mut SquadMicroMemory::default(),
    )
}

fn attack_targets(orders: &[SquadOrder]) -> BTreeMap<u32, u32> {
    let mut by_unit = BTreeMap::new();
    for order in orders {
        if let SquadOrder::Attack { units, target } = order {
            for unit in units {
                by_unit.insert(*unit, *target);
            }
        }
    }
    by_unit
}

#[test]
fn params_round_trip_through_their_text_form() {
    let params = RifleSquadParams::parse("naive,focus=weakest,retreat_hp=15,hold=1").unwrap();
    assert_eq!(params.focus, FocusMode::Weakest);
    assert_eq!(params.retreat_hp, 15);
    assert!(params.hold);
    assert!(
        !params.overkill_guard,
        "naive preset keeps its own defaults"
    );
    assert_eq!(
        RifleSquadParams::parse(&params.to_string()).unwrap(),
        params
    );
    assert_eq!(
        RifleSquadParams::parse("").unwrap(),
        RifleSquadParams::micro()
    );
}

#[test]
fn params_reject_unknown_keys_and_out_of_range_values() {
    assert!(RifleSquadParams::parse("speed=3").is_err());
    assert!(RifleSquadParams::parse("focus=closest").is_err());
    assert!(RifleSquadParams::parse("retreat_tiles=-1").is_err());
    assert!(RifleSquadParams::parse("regroup_tiles=NaN").is_err());
    assert!(RifleSquadParams::parse("hold=maybe").is_err());
}

#[test]
fn advancing_squad_attack_moves_on_the_enemy_start_once() {
    let owned = vec![
        rifleman(1, 1, 10.0, 10.0, 45),
        rifleman(2, 1, 11.0, 10.0, 45),
    ];
    let observation = observation(owned, Vec::new());
    let params = RifleSquadParams::naive();
    let mut memory = SquadMicroMemory::default();
    let objective = (60.5 * TILE, 60.5 * TILE);
    let orders = plan_rifle_squad(&observation, &[1, 2], objective, &params, &mut memory);
    assert_eq!(
        orders,
        vec![SquadOrder::AttackMove {
            units: vec![1, 2],
            x: objective.0,
            y: objective.1,
        }]
    );
    let mut moving = observation.clone();
    for unit in &mut moving.owned {
        unit.state = AiEntityState::Move;
    }
    assert!(
        plan_rifle_squad(&moving, &[1, 2], objective, &params, &mut memory).is_empty(),
        "an unchanged advance is not reissued every decision"
    );
}

#[test]
fn strung_out_squad_regroups_before_advancing() {
    let owned = vec![
        rifleman(1, 1, 10.0, 10.0, 45),
        rifleman(2, 1, 20.0, 10.0, 45),
    ];
    let observation = observation(owned, Vec::new());
    let orders = plan(&observation, &RifleSquadParams::micro());
    assert_eq!(
        orders,
        vec![SquadOrder::AttackMove {
            units: vec![1, 2],
            x: 15.0 * TILE,
            y: 10.0 * TILE,
        }]
    );
}

#[test]
fn weakest_focus_prefers_the_lowest_hp_enemy_in_range() {
    let owned = vec![
        rifleman(1, 1, 10.0, 10.0, 45),
        rifleman(2, 1, 10.0, 11.0, 45),
    ];
    let enemies = vec![
        rifleman(10, 2, 13.0, 10.0, 45),
        rifleman(11, 2, 14.0, 11.0, 30),
        rifleman(12, 2, 30.0, 30.0, 5),
    ];
    let observation = observation(owned, enemies);
    let params = RifleSquadParams {
        overkill_guard: false,
        ..RifleSquadParams::micro()
    };
    let targets = attack_targets(&plan(&observation, &params));
    assert_eq!(
        targets.get(&1),
        Some(&11),
        "out-of-range 5 HP enemy is not chased"
    );
    assert_eq!(targets.get(&2), Some(&11));
}

#[test]
fn overkill_guard_spills_extra_shooters_to_the_next_target() {
    let owned = vec![
        rifleman(1, 1, 10.0, 10.0, 45),
        rifleman(2, 1, 10.0, 11.0, 45),
        rifleman(3, 1, 10.0, 12.0, 45),
    ];
    let enemies = vec![
        rifleman(10, 2, 13.0, 11.0, 5),
        rifleman(11, 2, 13.0, 12.0, 45),
    ];
    let observation = observation(owned, enemies);
    let targets = attack_targets(&plan(&observation, &RifleSquadParams::micro()));
    let on_wounded = targets.values().filter(|target| **target == 10).count();
    assert_eq!(
        on_wounded, 1,
        "one 5-damage shot covers a 5 HP target: {targets:?}"
    );
    assert_eq!(targets.values().filter(|target| **target == 11).count(), 2);
}

#[test]
fn squad_nearest_without_guard_matches_the_ai_2_1_single_target_wave() {
    let owned = vec![
        rifleman(1, 1, 10.0, 10.0, 45),
        rifleman(2, 1, 10.0, 12.0, 45),
    ];
    let enemies = vec![
        rifleman(10, 2, 14.0, 11.0, 45),
        rifleman(11, 2, 20.0, 11.0, 5),
    ];
    let observation = observation(owned, enemies);
    let params = RifleSquadParams {
        focus: FocusMode::SquadNearest,
        overkill_guard: false,
        ..RifleSquadParams::micro()
    };
    assert_eq!(
        plan(&observation, &params),
        vec![SquadOrder::Attack {
            units: vec![1, 2],
            target: 10,
        }]
    );
}

#[test]
fn engaged_rifleman_is_not_reissued_its_current_target() {
    let mut shooter = rifleman(1, 1, 10.0, 10.0, 45);
    shooter.state = AiEntityState::Attack;
    shooter.target_id = Some(10);
    let observation = observation(vec![shooter], vec![rifleman(10, 2, 13.0, 10.0, 20)]);
    assert!(plan(&observation, &RifleSquadParams::micro()).is_empty());
}

#[test]
fn targeted_wounded_rifleman_steps_back_from_its_attacker() {
    let owned = vec![
        rifleman(1, 1, 10.0, 10.0, 10),
        rifleman(2, 1, 10.0, 11.0, 45),
    ];
    let mut attacker = rifleman(10, 2, 14.0, 10.0, 45);
    attacker.target_id = Some(1);
    let observation = observation(owned, vec![attacker]);
    let params = RifleSquadParams {
        retreat_hp: 15,
        retreat_tiles: 2.0,
        ..RifleSquadParams::micro()
    };
    let mut memory = SquadMicroMemory::default();
    let orders = plan_rifle_squad(
        &observation,
        &[1, 2],
        (60.0 * TILE, 60.0 * TILE),
        &params,
        &mut memory,
    );
    assert!(orders.contains(&SquadOrder::Move {
        units: vec![1],
        x: 8.0 * TILE,
        y: 10.0 * TILE,
    }));
    assert_eq!(attack_targets(&orders).get(&2), Some(&10));
    assert!(!attack_targets(&orders).contains_key(&1));
}

#[test]
fn last_healthy_riflemen_never_retreat() {
    let owned = vec![rifleman(1, 1, 10.0, 10.0, 10)];
    let mut attacker = rifleman(10, 2, 14.0, 10.0, 45);
    attacker.target_id = Some(1);
    let observation = observation(owned, vec![attacker]);
    let params = RifleSquadParams {
        retreat_hp: 15,
        ..RifleSquadParams::micro()
    };
    let orders = plan(&observation, &params);
    assert_eq!(attack_targets(&orders).get(&1), Some(&10));
}

#[test]
fn holding_squad_waits_until_an_enemy_reaches_contact_range() {
    let owned = vec![
        rifleman(1, 1, 10.0, 10.0, 45),
        rifleman(2, 1, 10.0, 11.0, 45),
    ];
    let far = vec![rifleman(10, 2, 18.0, 10.0, 45)];
    let params = RifleSquadParams {
        hold: true,
        ..RifleSquadParams::micro()
    };
    let mut memory = SquadMicroMemory::default();
    let objective = (60.0 * TILE, 60.0 * TILE);
    let waiting = observation(owned.clone(), far);
    assert_eq!(
        plan_rifle_squad(&waiting, &[1, 2], objective, &params, &mut memory),
        vec![SquadOrder::Hold { units: vec![1, 2] }]
    );
    assert!(plan_rifle_squad(&waiting, &[1, 2], objective, &params, &mut memory).is_empty());
    let contact = observation(owned, vec![rifleman(10, 2, 14.0, 10.0, 45)]);
    let orders = plan_rifle_squad(&contact, &[1, 2], objective, &params, &mut memory);
    assert_eq!(attack_targets(&orders).len(), 2);
}

#[test]
fn nearest_focus_leaves_targeting_to_the_simulation() {
    let owned = vec![rifleman(1, 1, 10.0, 10.0, 45)];
    let enemies = vec![
        rifleman(10, 2, 13.0, 10.0, 45),
        rifleman(11, 2, 13.0, 12.0, 45),
    ];
    let observation = observation(owned, enemies);
    let orders = plan(&observation, &RifleSquadParams::naive());
    assert_eq!(
        orders,
        vec![SquadOrder::AttackMove {
            units: vec![1],
            x: 13.0 * TILE,
            y: 11.0 * TILE,
        }]
    );
}

#[test]
fn enemy_workers_rank_behind_combat_units() {
    let owned = vec![rifleman(1, 1, 10.0, 10.0, 45)];
    let mut worker = rifleman(10, 2, 12.0, 10.0, 5);
    worker.kind = EntityKind::Worker;
    let enemies = vec![worker, rifleman(11, 2, 14.0, 10.0, 45)];
    let observation = observation(owned, enemies);
    let targets = attack_targets(&plan(&observation, &RifleSquadParams::micro()));
    assert_eq!(targets.get(&1), Some(&11));
}

#[test]
fn strategy_objective_is_the_enemy_start_tile_centre() {
    let observation = observation(Vec::new(), Vec::new());
    assert_eq!(
        enemy_start_center(&observation),
        Some((60.5 * TILE, 60.5 * TILE))
    );
}

#[test]
fn weapon_reach_matches_the_simulation_firing_distance() {
    // Rifles fire at 5 tiles + 9 px radius + 4 px slack = 173 px centre to centre, so an enemy
    // 5.35 tiles (171.2 px) away is both a legal focus target and contact for a holding squad.
    let owned = vec![rifleman(1, 1, 10.0, 10.0, 45)];
    let enemies = vec![
        rifleman(10, 2, 15.35, 10.0, 20),
        rifleman(11, 2, 12.0, 10.0, 45),
    ];
    let observation = observation(owned, enemies);
    let params = RifleSquadParams {
        hold: true,
        contact_margin_tiles: 0.0,
        ..RifleSquadParams::micro()
    };
    let targets = attack_targets(&plan(&observation, &params));
    assert_eq!(targets.get(&1), Some(&10));
    assert!((RifleWeapon::current().reach_px(TILE) - 173.0).abs() < f32::EPSILON);
}
