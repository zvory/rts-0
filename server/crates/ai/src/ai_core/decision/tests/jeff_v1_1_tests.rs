use super::jeff_v1_1::base_line::{self, allocate, base_lines, shares};
use super::jeff_v1_1::{barracks_batches, gun_cap, rifle_line};
use super::*;
use crate::ai_core::facts::ProductionBuildingFact;
use crate::ai_core::profiles::{JEFFS_AI, JEFFS_AI_V1_1, JEFFS_AI_V1_1_ID};

fn barracks(id: u32) -> ProductionBuildingFact {
    ProductionBuildingFact {
        id,
        kind: EntityKind::Barracks,
        queue_len: 0,
    }
}

fn economy() -> AiEconomy {
    AiEconomy {
        steel: 500,
        oil: 100,
        supply_used: 20,
        supply_cap: 80,
    }
}

#[test]
fn v1_1_trains_guns_from_its_oldest_barracks_and_no_guns_from_the_others() {
    let buildings = [barracks(9), barracks(4), barracks(12)];
    let priorities = [
        EntityKind::Tank,
        EntityKind::MachineGunner,
        EntityKind::Rifleman,
    ];
    let caps = [(EntityKind::MachineGunner, 10)];
    let short = [(EntityKind::MachineGunner, 3)];
    let batches = barracks_batches(
        JEFFS_AI_V1_1_ID,
        EntityKind::Barracks,
        &buildings,
        &priorities,
        (&short, &caps),
    );
    assert_eq!(
        batches,
        vec![
            (vec![barracks(4)], vec![EntityKind::MachineGunner]),
            (
                vec![barracks(9), barracks(12)],
                vec![EntityKind::Tank, EntityKind::Rifleman]
            ),
        ]
    );
    // With every gun post filled the oldest Barracks trains like the others.
    let full = [(EntityKind::MachineGunner, 10)];
    let batches = barracks_batches(
        JEFFS_AI_V1_1_ID,
        EntityKind::Barracks,
        &buildings,
        &priorities,
        (&full, &caps),
    );
    assert_eq!(batches[0].1, vec![EntityKind::Tank, EntityKind::Rifleman]);
    // V1.0, and V1.1 with one Barracks, train from every Barracks as before.
    for (profile_id, buildings) in [
        (JEFFS_AI_ID, &buildings[..]),
        (JEFFS_AI_V1_1_ID, &buildings[..1]),
    ] {
        let batches = barracks_batches(
            profile_id,
            EntityKind::Barracks,
            buildings,
            &priorities,
            (&short, &caps),
        );
        assert_eq!(batches, vec![(buildings.to_vec(), priorities.to_vec())]);
    }
}

#[test]
fn v1_1_builds_guns_for_the_home_pocket_each_bases_line_and_a_home_reserve() {
    // HQ and natural up, five guns out: six home, four at the natural and four held home.
    let mut owned = vec![
        building(1, EntityKind::ResourceDepot, Some(0)),
        building(2, EntityKind::ResourceDepot, Some(0)),
    ];
    owned.extend((10..15).map(|id| combat(id, EntityKind::MachineGunner)));
    let obs = observation(economy(), owned);
    let facts = AiFacts::from_observation(&obs);
    let mut caps = vec![(EntityKind::MachineGunner, 6)];
    gun_cap(&JEFFS_AI_V1_1, &facts, None, &[], &mut caps);
    assert_eq!(caps, vec![(EntityKind::MachineGunner, 14)]);
    let mut caps = vec![(EntityKind::MachineGunner, 6)];
    gun_cap(&JEFFS_AI, &facts, None, &[], &mut caps);
    assert_eq!(caps, vec![(EntityKind::MachineGunner, 6)]);
}

#[test]
fn v1_1_keeps_two_riflemen_for_every_tank_and_four_at_home() {
    let mut owned: Vec<_> = (10..13).map(|id| combat(id, EntityKind::Tank)).collect();
    owned.extend((20..25).map(|id| combat(id, EntityKind::Rifleman)));
    let obs = observation(economy(), owned);
    let facts = AiFacts::from_observation(&obs);
    let mut priorities = vec![EntityKind::Tank];
    assert_eq!(
        rifle_line(JEFFS_AI_V1_1_ID, &facts, &mut priorities),
        Some(10)
    );
    assert_eq!(priorities, vec![EntityKind::Tank, EntityKind::Rifleman]);
    let mut priorities = vec![EntityKind::Tank];
    assert_eq!(rifle_line(JEFFS_AI_ID, &facts, &mut priorities), None);
    assert_eq!(priorities, vec![EntityKind::Tank]);
}

/// Main Depot at the start (8, 8) with mines four tiles out along each side of the line to the
/// enemy start at (48, 48).
fn base_line_observation(owned: Vec<AiEntitySummary>) -> AiObservation {
    let ts = config::TILE_SIZE as f32;
    let mut all = vec![
        building_at(1, EntityKind::ResourceDepot, Some(0), 8.5 * ts, 8.5 * ts),
        building_at(2, EntityKind::SteelMine, None, 12.5 * ts, 9.5 * ts),
        building_at(3, EntityKind::SteelMine, None, 9.5 * ts, 12.5 * ts),
    ];
    all.extend(owned);
    observation(economy(), all)
}

/// How far `point` stands in front of the main Depot toward the enemy, and to its side, in tiles.
fn line_coordinates(point: (f32, f32)) -> (f32, f32) {
    let ts = config::TILE_SIZE as f32;
    let (dx, dy) = (point.0 / ts - 8.5, point.1 / ts - 8.5);
    let half = std::f32::consts::FRAC_1_SQRT_2;
    ((dx + dy) * half, (dy - dx) * half)
}

#[test]
fn v1_1_holds_its_line_two_tiles_past_the_forward_most_mine_with_guns_on_the_wings() {
    let ts = config::TILE_SIZE as f32;
    let obs = base_line_observation(Vec::new());
    let lines = base_lines(&obs, None);
    assert_eq!(lines.len(), 1);
    assert!(lines[0].main);
    // Two mines; without a map there is no route, so they are not doubled.
    assert_eq!(lines[0].exposure, 2);
    // Both mines stand 3.5 tiles toward the enemy and 2.1 tiles either side.
    let forward = 5.0 * std::f32::consts::FRAC_1_SQRT_2;
    let side = 3.0 * std::f32::consts::FRAC_1_SQRT_2 + 1.0;
    let rifles: Vec<(f32, f32)> = lines[0]
        .rifle_points(3, ts)
        .into_iter()
        .map(line_coordinates)
        .collect();
    for ((ahead, across), wanted) in rifles.iter().zip([0.0, 1.75, -1.75]) {
        assert!((ahead - (forward + 2.0)).abs() < 0.01, "{rifles:?}");
        assert!((across - wanted).abs() < 0.01, "{rifles:?}");
    }
    let guns: Vec<(f32, f32)> = lines[0]
        .gun_points(2, ts)
        .into_iter()
        .map(line_coordinates)
        .collect();
    for ((ahead, across), wanted) in guns.iter().zip([side, -side]) {
        assert!((ahead - (forward + 1.5)).abs() < 0.01, "{guns:?}");
        assert!((across - wanted).abs() < 0.01, "{guns:?}");
    }
    // From four Riflemen one stands picket seven tiles past the line.
    let picket = line_coordinates(*lines[0].rifle_points(4, ts).last().unwrap());
    assert!((picket.0 - (forward + 9.0)).abs() < 0.01, "{picket:?}");
}

#[test]
fn v1_1_splits_line_units_by_exposure_without_walking_units_off_a_base() {
    assert_eq!(shares(&[12, 4, 2], 7), vec![5, 1, 1]);
    assert_eq!(shares(&[0, 0], 3), vec![0, 0]);
    let ts = config::TILE_SIZE as f32;
    let mut obs = base_line_observation(Vec::new());
    obs.owned.push(building_at(
        4,
        EntityKind::ResourceDepot,
        Some(0),
        30.5 * ts,
        20.5 * ts,
    ));
    let lines = base_lines(&obs, None);
    assert_eq!(
        lines.iter().map(|line| line.depot).collect::<Vec<_>>(),
        vec![1, 4]
    );
    let weights = [6, 2];
    // Four Riflemen held the main; the natural's arrival takes the next one, not one of them.
    let mut units: Vec<(u32, (f32, f32), Option<u32>)> =
        (10..14).map(|id| (id, (0.0, 0.0), Some(1))).collect();
    let on_post = BTreeSet::new();
    assert!(allocate(&units, &lines, &weights, &on_post)
        .iter()
        .all(|(_, line)| *line == 0));
    units.push((14, (0.0, 0.0), None));
    assert_eq!(
        allocate(&units, &lines, &weights, &on_post),
        vec![(10, 0), (11, 0), (12, 0), (13, 0), (14, 1)]
    );
    // More than one over its share, the base gives up the units not on a post.
    let crowded: Vec<(u32, (f32, f32), Option<u32>)> =
        (10..14).map(|id| (id, (0.0, 0.0), Some(4))).collect();
    let on_post: BTreeSet<u32> = [12, 13].into_iter().collect();
    assert_eq!(
        allocate(&crowded, &lines, &weights, &on_post),
        vec![(10, 0), (11, 0), (12, 1), (13, 1)]
    );
}

#[test]
fn v1_1_posts_riflemen_on_the_line_and_leaves_fighting_ones_alone() {
    let ts = config::TILE_SIZE as f32;
    let riflemen = (20..23)
        .map(|id| combat_at(id, EntityKind::Rifleman, 6.5 * ts, 6.5 * ts))
        .collect();
    let mut obs = base_line_observation(riflemen);
    let facts = AiFacts::from_observation(&obs);
    let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI_V1_1);
    let held = BTreeSet::new();
    let mut actions = AiActionContext::new(&facts, SpendBudget::new(1000, 1000, 20, 80));
    let mut intents = Vec::new();
    let live = (&mut actions, &obs, None, &mut intents);
    base_line::hold(live, &mut memory, &JEFFS_AI_V1_1, &[], &held);
    let posted: Vec<(Vec<u32>, (f32, f32))> = actions
        .into_commands()
        .into_iter()
        .filter_map(|command| match command {
            Command::AttackMove { units, x, y, .. } => Some((units, line_coordinates((x, y)))),
            _ => None,
        })
        .collect();
    assert_eq!(posted.len(), 3, "{posted:?}");
    let forward = 5.0 * std::f32::consts::FRAC_1_SQRT_2 + 2.0;
    for (_, (ahead, _)) in &posted {
        assert!((ahead - forward).abs() < 1.5, "{posted:?}");
    }
    assert_eq!(
        intents,
        vec![AiIntent::Assemble {
            units: vec![20, 21, 22]
        }]
    );
    // Walking to their posts, they are not sent again; V1.0 posts none of its Riflemen here.
    obs.tick += 30;
    for unit in &mut obs.owned {
        if unit.kind == EntityKind::Rifleman {
            unit.state = AiEntityState::Move;
        }
    }
    for profile in [&JEFFS_AI_V1_1, &JEFFS_AI] {
        let mut actions = AiActionContext::new(&facts, SpendBudget::new(1000, 1000, 20, 80));
        let mut intents = Vec::new();
        let live = (&mut actions, &obs, None, &mut intents);
        base_line::hold(live, &mut memory, profile, &[], &held);
        assert!(actions.into_commands().is_empty());
    }
    // An enemy in sight nearby: the Riflemen fight where they stand.
    obs.tick += 600;
    let mut raider = combat_at(90, EntityKind::Rifleman, 9.5 * ts, 6.5 * ts);
    raider.owner = 2;
    obs.visible_enemies.push(raider);
    let mut actions = AiActionContext::new(&facts, SpendBudget::new(1000, 1000, 20, 80));
    let mut intents = Vec::new();
    let live = (&mut actions, &obs, None, &mut intents);
    base_line::hold(live, &mut memory, &JEFFS_AI_V1_1, &[], &held);
    assert!(actions.into_commands().is_empty());
}
