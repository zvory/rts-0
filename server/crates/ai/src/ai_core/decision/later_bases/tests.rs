use super::*;
use crate::ai_core::observation::{AiEconomy, AiResourceSummary};

const TS: f32 = config::TILE_SIZE as f32;

fn observation(tick: u32) -> AiObservation {
    AiObservation {
        player_id: 1,
        tick,
        map: AiMapSummary {
            width: 128,
            height: 128,
            tile_size: config::TILE_SIZE,
        },
        economy: AiEconomy {
            steel: 0,
            oil: 0,
            supply_used: 0,
            supply_cap: 100,
        },
        own_start_tile: (10, 10),
        players: Vec::new(),
        owned: Vec::new(),
        resources: Vec::new(),
        visible_allies: Vec::new(),
        visible_enemies: Vec::new(),
        ability_states: Vec::new(),
        smokes: Vec::new(),
        visible_tank_traps: Vec::new(),
        pending_builds: Vec::new(),
        upgrades: Vec::new(),
    }
}

fn entity(id: u32, kind: EntityKind, tile: (f32, f32)) -> AiEntitySummary {
    AiEntitySummary {
        id,
        owner: 1,
        kind,
        x: tile.0 * TS,
        y: tile.1 * TS,
        hp: 100,
        state: AiEntityState::Idle,
        is_complete: true,
        production_queue_len: None,
        production_kind: None,
        latched_node: None,
        target_id: None,
        free_for_combat: true,
    }
}

fn well(id: u32, tile: (f32, f32), remaining: u32) -> AiResourceSummary {
    AiResourceSummary {
        id,
        kind: EntityKind::Oil,
        x: tile.0 * TS,
        y: tile.1 * TS,
        remaining,
    }
}

/// A main Depot with one Pump Jack on a well, and a natural Depot far away.
fn pumped_base(tick: u32, remaining: u32) -> AiObservation {
    let mut observation = observation(tick);
    observation.owned = vec![
        entity(1, EntityKind::ResourceDepot, (10.0, 10.0)),
        entity(2, EntityKind::ResourceDepot, (60.0, 60.0)),
        entity(3, EntityKind::PumpJack, (15.0, 10.0)),
    ];
    observation.resources = vec![well(50, (15.0, 10.0), remaining)];
    observation
}

#[test]
fn only_three_bases_are_due_before_any_well_runs_dry() {
    let state = LaterBases::default();
    assert_eq!(state.target_bases(), 3);
}

#[test]
fn a_well_running_dry_unlocks_one_base_for_the_depot_that_mines_it() {
    let mut state = LaterBases::default();
    let mut remaining = 60;
    let mut tick = 1_000;
    while remaining > DRY_WELL_WARNING_REMAINING - 4 {
        note_dry_wells(&pumped_base(tick, remaining), &mut state);
        remaining -= 2;
        tick += 40;
    }
    assert_eq!(state.unlocking_depots, BTreeSet::from([1]));
    assert_eq!(state.target_bases(), 4);

    // Draining further does not unlock the same base twice.
    note_dry_wells(&pumped_base(tick, 2), &mut state);
    assert_eq!(state.target_bases(), 4);
}

#[test]
fn an_out_of_sight_placeholder_reading_does_not_count_as_a_dry_well() {
    let mut state = LaterBases::default();
    note_dry_wells(&pumped_base(1_000, 3_000), &mut state);
    // A well that drops out of sight reports 1: a far larger drop than pumping can cause.
    note_dry_wells(&pumped_base(1_009, 1), &mut state);
    assert!(state.unlocking_depots.is_empty());
    // A first reading that is already low is not trusted either.
    let mut fresh = LaterBases::default();
    note_dry_wells(&pumped_base(1_000, 1), &mut fresh);
    assert!(fresh.unlocking_depots.is_empty());
}

#[test]
fn a_rejected_site_rules_out_its_neighbours_but_not_other_bases() {
    let rejected = BTreeSet::from([(65, 23)]);
    assert!(site_is_rejected(&rejected, (65, 23)));
    assert!(site_is_rejected(&rejected, (65, 24)));
    assert!(site_is_rejected(&rejected, (69, 26)));
    assert!(!site_is_rejected(&rejected, (12, 27)));
}

#[test]
fn a_site_is_rejected_after_repeated_failures() {
    let mut state = LaterBases::default();
    assert!(!note_site_failure(&mut state, (40, 40)));
    assert!(note_site_failure(&mut state, (40, 40)));
    assert!(!note_site_failure(&mut state, (80, 80)));
}

#[test]
fn a_destroyed_depot_puts_the_next_base_on_a_cooldown_and_releases_the_guards() {
    let site = (40, 40);
    let center = building_center(site, EntityKind::ResourceDepot, config::TILE_SIZE).unwrap();
    let mut memory = AiDecisionMemory::default();
    memory.later_bases.site = Some(site);
    memory.later_bases.guard_site = Some(site);
    memory.later_bases.guards = BTreeSet::from([7, 8]);

    let mut built = observation(5_000);
    let mut depot = entity(20, EntityKind::ResourceDepot, (0.0, 0.0));
    depot.x = center.0;
    depot.y = center.1;
    depot.is_complete = false;
    built.owned.push(depot);
    refresh_guard_site(&built, &mut memory, 5_000);
    assert_eq!(memory.later_bases.site, None, "the foundation ends staging");
    assert_eq!(
        memory.later_bases.guard_site,
        Some(site),
        "guards cover construction"
    );

    refresh_guard_site(&observation(5_300), &mut memory, 5_300);
    let state = &memory.later_bases;
    assert_eq!(state.guard_site, None);
    assert!(state.guards.is_empty());
    assert_eq!(state.retry_after_tick, 5_300 + SITE_RETRY_COOLDOWN_TICKS);
    assert_eq!(state.site_failures.get(&site), Some(&1));
    assert!(!site_is_rejected(&state.rejected_sites, site));
}

#[test]
fn guards_are_held_once_on_arrival_rather_than_every_decision() {
    let slot = (30.0 * TS, 30.0 * TS);
    let mut memory = AiDecisionMemory::default();
    let mut observation = observation(100);
    observation
        .owned
        .push(entity(7, EntityKind::Tank, (30.0, 30.0)));
    let facts = AiFacts::from_observation(&observation);
    let posts = [(7, slot)];

    let mut actions = AiActionContext::new(&facts, SpendBudget::new(0, 0, 0, 100));
    let first = issue_guard_orders(
        &mut actions,
        &observation,
        &mut memory,
        &posts,
        &BTreeSet::new(),
    );
    assert_eq!(first, vec![7]);

    let mut actions = AiActionContext::new(&facts, SpendBudget::new(0, 0, 0, 100));
    let second = issue_guard_orders(
        &mut actions,
        &observation,
        &mut memory,
        &posts,
        &BTreeSet::new(),
    );
    assert!(second.is_empty(), "a held guard keeps its target");
}

#[test]
fn a_dead_guard_counts_as_lost_but_one_reassigned_home_does_not() {
    let mut observation = observation(100);
    observation.owned = vec![
        entity(7, EntityKind::Tank, (30.0, 30.0)),
        entity(8, EntityKind::Tank, (31.0, 30.0)),
    ];
    let mut memory = AiDecisionMemory::default();
    memory.later_bases.guards = BTreeSet::from([7, 8]);
    memory.home_defensive_tank = Some(8);
    assert!(!drop_lost_guards(&observation, &mut memory));
    assert_eq!(memory.later_bases.guards, BTreeSet::from([7]));

    observation.owned.retain(|unit| unit.id != 7);
    assert!(drop_lost_guards(&observation, &mut memory));
    assert!(memory.later_bases.guards.is_empty());
}

#[test]
fn guards_are_the_nearest_tanks_outside_the_push_and_home_defense() {
    let mut observation = observation(100);
    observation.owned = vec![
        entity(1, EntityKind::Tank, (40.0, 40.0)),
        entity(2, EntityKind::Tank, (41.0, 40.0)),
        entity(3, EntityKind::Tank, (20.0, 20.0)),
        entity(4, EntityKind::Tank, (12.0, 12.0)),
        entity(5, EntityKind::Tank, (42.0, 40.0)),
    ];
    let mut memory = AiDecisionMemory::default();
    memory.home_defensive_tank = Some(1);
    memory.containment.active_tanks = BTreeSet::from([2]);
    select_guards(&observation, &mut memory, (40.0 * TS, 40.0 * TS));
    assert_eq!(memory.later_bases.guards, BTreeSet::from([3, 5]));
}

#[test]
fn an_abandoned_site_is_only_ruled_out_when_rejected() {
    let mut state = LaterBases {
        site: Some((40, 40)),
        guard_site: Some((40, 40)),
        guards: BTreeSet::from([7, 8]),
        ..LaterBases::default()
    };
    abandon_site(&mut state, (40, 40), 1_000, false);
    assert!(state.guards.is_empty());
    assert_eq!(state.site, None);
    assert_eq!(state.retry_after_tick, 1_000 + SITE_RETRY_COOLDOWN_TICKS);
    assert!(!site_is_rejected(&state.rejected_sites, (40, 40)));

    abandon_site(&mut state, (40, 40), 2_000, true);
    assert!(site_is_rejected(&state.rejected_sites, (40, 40)));
}

#[test]
fn a_later_base_on_the_enemys_route_in_is_marked_down() {
    use crate::ai_core::map_analysis::AiMapAnalysis;
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
    let map = Map::load_for_players("Classic", &[(1, 1), (2, 2)], 0).unwrap();
    let game = Game::new_with_random_ai_profiles_and_map_metadata(
        &players,
        0,
        map,
        Map::metadata_for_name("Classic").unwrap(),
    );
    let start = game.start_payload();
    let analysis = AiMapAnalysis::analyze(&start);
    let observation =
        AiObservation::from_snapshot_with_alive(&start, &game.snapshot_for(1), 1, [], None)
            .unwrap();
    assert_eq!(observation.own_start_tile, (9, 9));
    let route = analysis.base_route_tiles(1);
    // The base at (39, 43) lies on the diagonal the enemy walks in on; (68, 27) and the natural at
    // (39, 14) do not.
    assert_eq!(
        route_exposure_penalty(&observation, route, (38, 42)),
        -ROUTE_EXPOSURE_PENALTY
    );
    assert_eq!(route_exposure_penalty(&observation, route, (67, 26)), 0);
    assert_eq!(route_exposure_penalty(&observation, route, (38, 13)), 0);
    // Without map analysis nothing is marked down.
    assert_eq!(route_exposure_penalty(&observation, None, (38, 42)), 0);
}
