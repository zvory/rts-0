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
    govern(&obs, None, &mut micro, &mut commands, &mut intents);
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
    govern(&obs, None, &mut micro, &mut commands, &mut intents);
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
    govern(&obs, None, &mut micro, &mut commands, &mut intents);
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
