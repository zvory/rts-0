use super::*;

const TS: f32 = 32.0;

fn zone_at(x_tiles: f32, y_tiles: f32) -> Zone {
    Zone {
        center: (x_tiles * TS, y_tiles * TS),
        radius: REACH_TILES * TS,
        tank: Some(7),
    }
}

fn tiles(x: f32, y: f32) -> (f32, f32) {
    (x * TS, y * TS)
}

#[test]
fn an_order_into_a_parked_tanks_reach_stops_just_outside_it() {
    let zones = [zone_at(40.0, 10.0)];
    let vision = Vision::everywhere();
    let Clip::StopAt { point, zone, .. } =
        clip_path(&zones, &vision, tiles(10.0, 10.0), tiles(38.0, 10.0), TS)
    else {
        panic!("the order runs into the reach");
    };
    assert_eq!(zone, 0);
    let distance_tiles = (40.0 * TS - point.0) / TS;
    assert!(
        (REACH_TILES..=REACH_TILES + 1.5).contains(&distance_tiles),
        "stops just outside the reach: {distance_tiles} tiles"
    );
    assert!((point.1 - 10.0 * TS).abs() < 1e-3);
}

#[test]
fn orders_that_pass_beside_or_back_out_of_a_reach_are_left_alone() {
    let zones = [zone_at(40.0, 10.0)];
    let vision = Vision::everywhere();
    let beside = clip_path(&zones, &vision, tiles(10.0, 40.0), tiles(60.0, 40.0), TS);
    assert_eq!(beside, Clip::Clear);
    // Inside the reach, an order away from the Tank is a retreat and stays.
    let back_out = clip_path(&zones, &vision, tiles(30.0, 10.0), tiles(20.0, 10.0), TS);
    assert_eq!(back_out, Clip::Clear);
    // Inside the reach, an order deeper in becomes standing still.
    let deeper = clip_path(&zones, &vision, tiles(30.0, 10.0), tiles(35.0, 10.0), TS);
    assert_eq!(deeper, Clip::Stand { zone: 0 });
}

#[test]
fn the_earliest_reach_on_the_way_decides_the_stop() {
    let zones = [zone_at(70.0, 10.0), zone_at(40.0, 10.0)];
    let vision = Vision::everywhere();
    let Clip::StopAt { zone, .. } =
        clip_path(&zones, &vision, tiles(0.0, 10.0), tiles(80.0, 10.0), TS)
    else {
        panic!("the order runs into both reaches");
    };
    assert_eq!(zone, 1);
}

#[test]
fn a_reach_the_enemy_cannot_see_does_not_stop_the_tank() {
    let zones = [zone_at(40.0, 10.0)];
    // Only the parked Tank's own 9-tile sight: it cannot see a Tank stopping 12 tiles away.
    let blind = Vision::around(vec![(tiles(40.0, 10.0), 9.0 * TS)]);
    let Clip::StopAt { point, .. } =
        clip_path(&zones, &blind, tiles(10.0, 10.0), tiles(38.0, 10.0), TS)
    else {
        panic!("the order still runs into the seen part of the reach");
    };
    let distance_tiles = (40.0 * TS - point.0) / TS;
    assert!(
        (9.0..=10.5).contains(&distance_tiles),
        "stops only where the Tank's own sight begins: {distance_tiles} tiles"
    );
    let to_twelve = clip_path(&zones, &blind, tiles(10.0, 10.0), tiles(28.0, 10.0), TS);
    assert_eq!(to_twelve, Clip::Clear);
}

fn summary(id: u32, owner: u32, kind: EntityKind, at: (f32, f32), hp: u32) -> AiEntitySummary {
    AiEntitySummary {
        id,
        owner,
        kind,
        x: at.0 * TS,
        y: at.1 * TS,
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

fn observation(
    tick: u32,
    owned: Vec<AiEntitySummary>,
    visible_enemies: Vec<AiEntitySummary>,
) -> AiObservation {
    use crate::ai_core::observation::{AiEconomy, AiPlayerSummary};
    AiObservation {
        player_id: 1,
        tick,
        map: AiMapSummary {
            width: 128,
            height: 128,
            tile_size: TS as u32,
        },
        economy: AiEconomy {
            steel: 0,
            oil: 0,
            supply_used: 0,
            supply_cap: 100,
        },
        own_start_tile: (5, 50),
        players: vec![
            AiPlayerSummary {
                id: 1,
                team_id: 1,
                start_tile: (5, 50),
                is_ai: true,
                is_alive: true,
            },
            AiPlayerSummary {
                id: 2,
                team_id: 2,
                start_tile: (120, 50),
                is_ai: false,
                is_alive: true,
            },
        ],
        owned,
        resources: Vec::new(),
        visible_allies: Vec::new(),
        visible_enemies,
        ability_states: Vec::new(),
        smokes: Vec::new(),
        visible_tank_traps: Vec::new(),
        pending_builds: Vec::new(),
        upgrades: Vec::new(),
    }
}

fn move_targets(commands: &[Command], unit: u32) -> Vec<(f32, f32)> {
    commands
        .iter()
        .filter_map(|command| match command {
            Command::Move { units, x, y, .. } | Command::AttackMove { units, x, y, .. }
                if units.contains(&unit) =>
            {
                Some((*x / TS, *y / TS))
            }
            _ => None,
        })
        .collect()
}

fn parked_pair_with_spotter() -> Vec<AiEntitySummary> {
    vec![
        summary(101, 2, EntityKind::Tank, (40.0, 50.0), 292),
        summary(102, 2, EntityKind::Tank, (40.0, 52.0), 292),
        summary(103, 2, EntityKind::Rifleman, (36.0, 50.0), 45),
    ]
}

#[test]
fn a_lone_tank_ordered_into_a_spotted_parked_line_stops_where_it_would_be_seen() {
    let mut micro = TankMicro::default();
    let owned = vec![summary(1, 1, EntityKind::Tank, (10.0, 50.0), 292)];
    let obs = observation(300, owned, parked_pair_with_spotter());
    observe(&obs, &mut micro);
    let mut commands = vec![Command::AttackMove {
        units: vec![1],
        x: 38.0 * TS,
        y: 50.0 * TS,
        queued: false,
    }];
    let mut intents = Vec::new();
    govern(
        &obs,
        None,
        None,
        &BTreeSet::new(),
        &mut micro,
        &mut commands,
        &mut intents,
    );
    let targets = move_targets(&commands, 1);
    assert_eq!(targets.len(), 1, "one order: {commands:?}");
    let (x, _) = targets[0];
    // The spotter sees 11 tiles from x = 36 and the Tanks reach 15 from x = 40, so danger starts
    // at x = 25.
    assert!((22.0..25.0).contains(&x), "stops before x = 25, got {x}");
}

#[test]
fn a_larger_force_may_drive_into_a_smaller_parked_group() {
    let mut micro = TankMicro::default();
    let owned = vec![
        summary(1, 1, EntityKind::Tank, (10.0, 49.0), 292),
        summary(2, 1, EntityKind::Tank, (10.0, 51.0), 292),
        summary(3, 1, EntityKind::Tank, (11.0, 50.0), 292),
    ];
    let enemy = vec![
        summary(101, 2, EntityKind::Tank, (40.0, 50.0), 292),
        summary(103, 2, EntityKind::Rifleman, (36.0, 50.0), 45),
    ];
    let obs = observation(300, owned, enemy);
    observe(&obs, &mut micro);
    let mut commands = vec![Command::AttackMove {
        units: vec![1, 2, 3],
        x: 38.0 * TS,
        y: 50.0 * TS,
        queued: false,
    }];
    let mut intents = Vec::new();
    govern(
        &obs,
        None,
        None,
        &BTreeSet::new(),
        &mut micro,
        &mut commands,
        &mut intents,
    );
    for tank in [1, 2, 3] {
        assert_eq!(move_targets(&commands, tank), vec![(38.0, 50.0)]);
    }
}

#[test]
fn a_tank_hit_from_fog_backs_out_with_its_neighbour_keeping_its_front_forward() {
    let mut micro = TankMicro::default();
    let before = vec![
        summary(1, 1, EntityKind::Tank, (60.0, 50.0), 292),
        summary(2, 1, EntityKind::Tank, (60.0, 53.0), 292),
    ];
    observe(&observation(300, before, Vec::new()), &mut micro);
    let after = vec![
        summary(1, 1, EntityKind::Tank, (60.0, 50.0), 232),
        summary(2, 1, EntityKind::Tank, (60.0, 53.0), 292),
    ];
    let obs = observation(309, after, Vec::new());
    observe(&obs, &mut micro);
    let mut commands = Vec::new();
    let mut intents = Vec::new();
    govern(
        &obs,
        None,
        None,
        &BTreeSet::new(),
        &mut micro,
        &mut commands,
        &mut intents,
    );
    for (tank, y) in [(1, 50.0), (2, 53.0)] {
        let targets = move_targets(&commands, tank);
        assert_eq!(targets.len(), 1, "tank {tank} backs out: {commands:?}");
        let (x, ty) = targets[0];
        // Away from the enemy start (to the east), at most 3 tiles so the hull reverses.
        assert!(x < 60.0, "tank {tank} backs west, got {x}");
        let hop = ((60.0 - x).powi(2) + (y - ty).powi(2)).sqrt();
        assert!(hop <= 3.0, "tank {tank} hop {hop} tiles");
    }
    assert_eq!(micro.ghosts.len(), 1, "the unseen shooter is estimated");
}

#[test]
fn a_staged_tank_the_governor_cuts_short_is_staged_again_once_released() {
    let mut micro = TankMicro::default();
    let owned = vec![summary(1, 1, EntityKind::Tank, (10.0, 50.0), 292)];
    let obs = observation(300, owned, parked_pair_with_spotter());
    observe(&obs, &mut micro);
    let mut commands = vec![Command::Move {
        units: vec![1],
        x: 38.0 * TS,
        y: 50.0 * TS,
        queued: false,
    }];
    let mut intents = vec![AiIntent::Stage { units: vec![1] }];
    govern(
        &obs,
        None,
        None,
        &BTreeSet::new(),
        &mut micro,
        &mut commands,
        &mut intents,
    );
    // The live controller sends a stage order once; assembling clears that, so the stage order
    // reaches the Tank again when the reach no longer cuts it.
    assert!(
        intents
            .iter()
            .any(|intent| matches!(intent, AiIntent::Assemble { units } if units == &[1])),
        "{intents:?}"
    );
}

#[test]
fn a_group_that_clearly_outnumbers_a_visible_attacker_keeps_its_orders() {
    let mut micro = TankMicro::default();
    let at = [(60.0, 50.0), (60.0, 52.0), (61.0, 51.0), (59.0, 51.0)];
    let tanks = |first_hp: u32| {
        at.iter()
            .enumerate()
            .map(|(index, point)| {
                let hp = if index == 0 { first_hp } else { 292 };
                summary(index as u32 + 1, 1, EntityKind::Tank, *point, hp)
            })
            .collect::<Vec<_>>()
    };
    let mut enemy = summary(101, 2, EntityKind::Tank, (70.0, 50.0), 292);
    enemy.state = AiEntityState::Move;
    observe(
        &observation(300, tanks(292), vec![enemy.clone()]),
        &mut micro,
    );
    let obs = observation(309, tanks(232), vec![enemy]);
    observe(&obs, &mut micro);
    let mut commands = vec![Command::AttackMove {
        units: vec![1, 2, 3, 4],
        x: 80.0 * TS,
        y: 50.0 * TS,
        queued: false,
    }];
    let mut intents = Vec::new();
    govern(
        &obs,
        None,
        None,
        &BTreeSet::new(),
        &mut micro,
        &mut commands,
        &mut intents,
    );
    assert!(
        !commands
            .iter()
            .any(|command| matches!(command, Command::HoldPosition { .. })),
        "{commands:?}"
    );
    for tank in 1..=4 {
        assert_eq!(move_targets(&commands, tank), vec![(80.0, 50.0)]);
    }
}

fn marching_pair_with_scout(x: f32) -> Vec<AiEntitySummary> {
    let mut units = vec![
        summary(1, 1, EntityKind::Tank, (x, 50.0), 292),
        summary(2, 1, EntityKind::Tank, (x, 52.0), 292),
        summary(9, 1, EntityKind::ScoutCar, (x - 2.0, 51.0), 100),
    ];
    for unit in &mut units {
        unit.state = AiEntityState::Move;
    }
    units
}

#[test]
fn the_push_scout_car_leads_a_march_but_keeps_the_push_smoke_cast() {
    let mut micro = TankMicro::default();
    observe(
        &observation(300, marching_pair_with_scout(36.0), Vec::new()),
        &mut micro,
    );
    let obs = observation(309, marching_pair_with_scout(40.0), Vec::new());
    observe(&obs, &mut micro);

    // Moving with the push: the governor takes over its movement, 4 tiles ahead of the front.
    let mut commands = vec![Command::Move {
        units: vec![9],
        x: 30.0 * TS,
        y: 51.0 * TS,
        queued: false,
    }];
    let mut intents = Vec::new();
    govern(
        &obs,
        None,
        Some(9),
        &BTreeSet::new(),
        &mut micro.clone(),
        &mut commands,
        &mut intents,
    );
    let targets = move_targets(&commands, 9);
    assert_eq!(targets.len(), 1, "{commands:?}");
    assert!(targets[0].0 > 40.0, "leads ahead of the Tanks: {targets:?}");

    // Casting for the push: left alone.
    let cast = Command::UseAbility {
        ability: rts_rules::faction::AbilityKind::Smoke,
        units: vec![9],
        x: Some(50.0 * TS),
        y: Some(51.0 * TS),
        queued: false,
    };
    let mut commands = vec![cast.clone()];
    govern(
        &obs,
        None,
        Some(9),
        &BTreeSet::new(),
        &mut micro,
        &mut commands,
        &mut intents,
    );
    assert_eq!(commands, vec![cast]);
}

/// Our Tanks lined up 20 tiles from two parked enemy Tanks, ordered at them, with a smoke-ready
/// Scout Car and smoke researched: returns whether the governor starts a smoke assault.
fn assault_starts_with(our_tanks: u32) -> bool {
    use crate::ai_core::observation::AiAbilitySummary;
    use rts_rules::faction::AbilityKind;
    let mut micro = TankMicro::default();
    let mut owned: Vec<AiEntitySummary> = (0..our_tanks)
        .map(|index| {
            summary(
                1 + index,
                1,
                EntityKind::Tank,
                (20.0, 44.0 + index as f32 * 1.5),
                292,
            )
        })
        .collect();
    owned.push(summary(50, 1, EntityKind::ScoutCar, (17.0, 50.0), 100));
    owned.push(summary(
        60,
        1,
        EntityKind::EngineeringComplex,
        (6.0, 50.0),
        1000,
    ));
    let enemy = vec![
        summary(101, 2, EntityKind::Tank, (40.0, 49.0), 292),
        summary(102, 2, EntityKind::Tank, (40.0, 51.0), 292),
    ];
    // Seen standing still twice: parked.
    observe(&observation(300, owned.clone(), enemy.clone()), &mut micro);
    let mut obs = observation(309, owned, enemy);
    obs.ability_states.push(AiAbilitySummary {
        entity_id: 50,
        kind: AbilityKind::Smoke,
        cooldown_left: 0,
        remaining_uses: None,
        available_tick: None,
        lockout_until_tick: None,
        charge_recharge_left: None,
    });
    observe(&obs, &mut micro);
    let mut commands = vec![Command::AttackMove {
        units: (1..=our_tanks).collect(),
        x: 39.0 * TS,
        y: 50.0 * TS,
        queued: false,
    }];
    govern(
        &obs,
        None,
        None,
        &BTreeSet::new(),
        &mut micro,
        &mut commands,
        &mut Vec::new(),
    );
    micro.assault.is_some()
}

#[test]
fn a_force_more_than_twice_a_parked_groups_size_drives_in_without_smoke() {
    assert!(
        assault_starts_with(3),
        "3 against 2 parked Tanks: smoke first"
    );
    assert!(
        !assault_starts_with(5),
        "5 against 2: an assault would only split the push"
    );
}

#[test]
fn a_cannon_ordered_onto_infantry_or_a_fresh_tank_goes_on_the_tank_that_dies_soonest() {
    let mut micro = TankMicro::default();
    let owned = vec![summary(1, 1, EntityKind::Tank, (10.0, 50.0), 292)];
    let enemy = vec![
        summary(101, 2, EntityKind::Tank, (18.0, 50.0), 292),
        summary(102, 2, EntityKind::Tank, (19.0, 52.0), 100),
        summary(103, 2, EntityKind::Rifleman, (15.0, 50.0), 45),
        summary(104, 2, EntityKind::Tank, (30.0, 50.0), 40),
    ];
    // Standing three seconds: the cannon reaches 14 tiles.
    observe(&observation(100, owned.clone(), enemy.clone()), &mut micro);
    let obs = observation(200, owned, enemy);
    observe(&obs, &mut micro);
    let tanks: BTreeMap<u32, &AiEntitySummary> =
        obs.owned.iter().map(|unit| (unit.id, unit)).collect();
    let attack = |target: u32| Command::Attack {
        units: vec![1],
        target,
        queued: false,
    };
    // 102 dies in two shots, 101 in five.
    for ordered in [103, 101] {
        let mut commands = vec![attack(ordered)];
        engage::aim(&obs, &mut micro, &tanks, &BTreeSet::new(), &mut commands);
        assert_eq!(commands, vec![attack(102)], "ordered onto {ordered}");
    }
    // 104 dies in one shot but is out of reach.
    let mut commands = vec![attack(102)];
    engage::aim(&obs, &mut micro, &tanks, &BTreeSet::new(), &mut commands);
    assert_eq!(commands, vec![attack(102)]);
}

/// Our Tank 1 parked 12 tiles from enemy Tank 101, which stands and shoots at it, plus `others`.
/// Both sides are seen standing for four seconds, so Tank 1 is charged and Tank 101 counts as
/// standing.
fn one_on_one_fight(micro: &mut TankMicro, others: &[(u32, (f32, f32))]) -> AiObservation {
    let mut owned = vec![summary(1, 1, EntityKind::Tank, (40.0, 50.0), 292)];
    owned.extend(
        others
            .iter()
            .map(|(id, at)| summary(*id, 1, EntityKind::Tank, *at, 292)),
    );
    let enemy = vec![AiEntitySummary {
        target_id: Some(1),
        state: AiEntityState::Attack,
        ..summary(101, 2, EntityKind::Tank, (52.0, 50.0), 292)
    }];
    observe(&observation(300, owned.clone(), enemy.clone()), micro);
    let obs = observation(420, owned, enemy);
    observe(&obs, micro);
    obs
}

fn concentrate_on(obs: &AiObservation, micro: &mut TankMicro) -> (BTreeSet<u32>, Vec<Command>) {
    let tanks: BTreeMap<u32, &AiEntitySummary> = obs
        .owned
        .iter()
        .filter(|unit| unit.kind == EntityKind::Tank)
        .map(|unit| (unit.id, unit))
        .collect();
    let mut extra = Vec::new();
    let joined = engage::concentrate(obs, None, micro, &tanks, &BTreeSet::new(), &[], &mut extra);
    (joined, extra)
}

#[test]
fn a_second_tank_comes_up_to_make_a_duel_two_on_one() {
    let mut micro = TankMicro::default();
    // Tank 2 is 21 tiles off, out of reach.
    let obs = one_on_one_fight(&mut micro, &[(2, (35.0, 62.0))]);
    let (joined, extra) = concentrate_on(&obs, &mut micro);
    assert!(joined.contains(&2), "{:?}", micro.notes);
    assert!(!joined.contains(&1), "the Tank already firing keeps on");
    let posts = move_targets(&extra, 2);
    assert_eq!(posts.len(), 1, "{extra:?}");
    let (x, y) = posts[0];
    let range = ((x - 52.0).powi(2) + (y - 50.0).powi(2)).sqrt();
    assert!(range <= 13.9, "fires from within reach: {range} tiles");
    assert!(x < 52.0 && y > 50.0, "on its own side: ({x}, {y})");
}

#[test]
fn a_fight_already_won_two_to_one_draws_no_more_tanks() {
    let mut micro = TankMicro::default();
    // Tank 3 already reaches Tank 101 too.
    let obs = one_on_one_fight(&mut micro, &[(2, (35.0, 62.0)), (3, (41.0, 46.0))]);
    let (joined, extra) = concentrate_on(&obs, &mut micro);
    assert!(joined.is_empty(), "{:?}", micro.notes);
    assert!(extra.is_empty());
}

#[test]
fn a_push_tank_is_not_drawn_to_a_fight_away_from_the_push() {
    // Tank 2 is the nearest Tank of a push whose others are 45 tiles from the fight.
    let mut micro = TankMicro {
        push_tanks: [2, 4, 5].into_iter().collect(),
        ..TankMicro::default()
    };
    let obs = one_on_one_fight(
        &mut micro,
        &[(2, (35.0, 62.0)), (4, (10.0, 90.0)), (5, (12.0, 92.0))],
    );
    let (joined, _) = concentrate_on(&obs, &mut micro);
    assert!(!joined.contains(&2), "{:?}", micro.notes);
}
