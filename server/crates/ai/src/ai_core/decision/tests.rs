use super::defense::main_steel_cluster_center;
use super::geometry::{building_center, dist2, normalized_direction, tile_center};
use super::*;

use crate::ai_core::observation::{
    AiBuildIntent, AiEconomy, AiEntityState, AiEntitySummary, AiMapSummary, AiObservation,
    AiPlayerSummary, AiResourceSummary,
};
use crate::ai_core::profiles::AiProfile;
use rts_sim::game::command::SimCommand as Command;

mod economy_manager_tests;
mod expansion_security_tests;
mod steel_line_tests;
mod turtle_tests;

fn worker(id: u32, state: AiEntityState) -> AiEntitySummary {
    worker_at(id, state, id as f32, 0.0)
}

fn worker_at(id: u32, state: AiEntityState, x: f32, y: f32) -> AiEntitySummary {
    AiEntitySummary {
        id,
        owner: 1,
        kind: EntityKind::Worker,
        x,
        y,
        hp: 100,
        state,
        is_complete: true,
        production_queue_len: None,
        production_kind: None,
        latched_node: None,
        target_id: None,
        free_for_combat: false,
    }
}

fn steel_worker(id: u32, node: u32) -> AiEntitySummary {
    let mut worker = worker(id, AiEntityState::Gather);
    worker.latched_node = Some(node);
    worker
}

fn resource(id: u32, kind: EntityKind, x: f32, y: f32) -> AiResourceSummary {
    AiResourceSummary {
        id,
        kind,
        x,
        y,
        remaining: 1_000,
    }
}

fn building(id: u32, kind: EntityKind, queue_len: Option<usize>) -> AiEntitySummary {
    building_at(id, kind, queue_len, 0.0, 0.0)
}

fn building_at(
    id: u32,
    kind: EntityKind,
    queue_len: Option<usize>,
    x: f32,
    y: f32,
) -> AiEntitySummary {
    AiEntitySummary {
        id,
        owner: 1,
        kind,
        x,
        y,
        hp: 100,
        state: queue_len
            .filter(|queue| *queue > 0)
            .map(|_| AiEntityState::Train)
            .unwrap_or(AiEntityState::Idle),
        is_complete: true,
        production_queue_len: queue_len,
        production_kind: None,
        latched_node: None,
        target_id: None,
        free_for_combat: false,
    }
}

fn combat(id: u32, kind: EntityKind) -> AiEntitySummary {
    combat_at(id, kind, 0.0, 0.0)
}

fn combat_at(id: u32, kind: EntityKind, x: f32, y: f32) -> AiEntitySummary {
    AiEntitySummary {
        id,
        owner: 1,
        kind,
        x,
        y,
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

fn observation(economy: AiEconomy, owned: Vec<AiEntitySummary>) -> AiObservation {
    let tile_size = config::TILE_SIZE;
    let ts = tile_size as f32;
    let mut resources = Vec::new();
    for i in 0..18 {
        resources.push(resource(
            100 + i,
            EntityKind::Steel,
            (8.5 + (i % 6) as f32) * ts,
            (8.5 + (i / 6) as f32) * ts,
        ));
    }
    for i in 0..3 {
        resources.push(resource(
            200 + i,
            EntityKind::Oil,
            (10.5 + i as f32) * ts,
            12.5 * ts,
        ));
    }
    AiObservation {
        player_id: 1,
        tick: 90,
        map: AiMapSummary {
            width: 64,
            height: 64,
            tile_size,
        },
        economy,
        own_start_tile: (8, 8),
        players: vec![
            AiPlayerSummary {
                id: 1,
                team_id: 1,
                start_tile: (8, 8),
                is_ai: true,
                is_alive: true,
            },
            AiPlayerSummary {
                id: 2,
                team_id: 2,
                start_tile: (48, 48),
                is_ai: false,
                is_alive: true,
            },
        ],
        owned,
        resources,
        visible_allies: Vec::new(),
        visible_enemies: Vec::new(),
        ability_states: Vec::new(),
        smokes: Vec::new(),
        pending_builds: Vec::new(),
        upgrades: Vec::new(),
    }
}

fn with_expansion_resources(mut observation: AiObservation) -> AiObservation {
    let ts = observation.map.tile_size as f32;
    for i in 0..18 {
        observation.resources.push(resource(
            300 + i,
            EntityKind::Steel,
            (21.5 + (i % 6) as f32) * ts,
            (31.5 + (i / 6) as f32) * ts,
        ));
    }
    for i in 0..3 {
        observation.resources.push(resource(
            400 + i,
            EntityKind::Oil,
            (16.5 + i as f32) * ts,
            38.5 * ts,
        ));
    }
    observation.resources.sort_by_key(|resource| resource.id);
    observation
}

fn base_site_resources(first_id: u32, site: (u32, u32), map_size: u32) -> Vec<AiResourceSummary> {
    let ts = config::TILE_SIZE as f32;
    let (hx, hy) = (site.0 as f32 + 0.5, site.1 as f32 + 0.5);
    let map_center = map_size as f32 * 0.5;
    let base_angle = (map_center - hy).atan2(map_center - hx);
    let (perp_x, perp_y) = (-base_angle.sin(), base_angle.cos());
    let mut resources = Vec::new();
    let mut steel_index = 0;
    for (side, field_patches) in [
        (1.0, config::STEEL_PATCHES_PER_BASE.div_ceil(2)),
        (-1.0, config::STEEL_PATCHES_PER_BASE / 2),
    ] {
        let block_cx = hx + side * config::STEEL_BLOCK_DIST_TILES * base_angle.cos();
        let block_cy = hy + side * config::STEEL_BLOCK_DIST_TILES * base_angle.sin();
        let row_center = field_patches.div_ceil(6).saturating_sub(1) as f32 / 2.0;
        for i in 0..field_patches {
            let (off_x, off_y) = ((i % 6) as f32 - 2.5, (i / 6) as f32 - row_center);
            resources.push(resource(
                first_id + steel_index,
                EntityKind::Steel,
                (block_cx + off_x * perp_x + off_y * base_angle.cos()) * ts,
                (block_cy + off_x * perp_y + off_y * base_angle.sin()) * ts,
            ));
            steel_index += 1;
        }
    }
    resources
}

fn decide(
    observation: &AiObservation,
    profile: &'static AiProfile,
    memory: &mut AiDecisionMemory,
) -> AiDecision {
    let width = observation.map.width;
    let height = observation.map.height;
    decide_profile_without_static_map_for_tests(
        observation,
        profile,
        memory,
        ai_shared::BuildSearch {
            min_radius: 0,
            max_radius: 0,
            prefer_away_from_center: false,
            prefer_toward_center: false,
        },
        |_, tx, ty| tx < width && ty < height,
    )
}

#[test]
fn defensive_incident_search_expires_at_its_bounded_end_condition() {
    const SEARCH_TICKS: u32 = config::TICK_HZ * 2;
    let mut memory = AiDecisionMemory::for_profile(&crate::ai_core::profiles::JEFFS_AI);
    memory.note_defensive_contact(100, (320.4, 640.6), 100, false);

    let active = memory
        .defensive_incident(100 + SEARCH_TICKS, SEARCH_TICKS)
        .expect("incident remains active through the search window");
    assert_eq!(active.position, (320.0, 641.0));
    assert!(memory
        .defensive_incident(101 + SEARCH_TICKS, SEARCH_TICKS)
        .is_none());
}

#[test]
fn defensive_interceptors_move_unentrenched_reserves_first() {
    let ts = config::TILE_SIZE as f32;
    let mut observation = observation(
        AiEconomy {
            steel: 0,
            oil: 0,
            supply_used: 3,
            supply_cap: 10,
        },
        vec![
            combat_at(10, EntityKind::Rifleman, 10.5 * ts, 10.5 * ts),
            combat_at(20, EntityKind::Rifleman, 11.5 * ts, 10.5 * ts),
            combat_at(30, EntityKind::Rifleman, 12.5 * ts, 10.5 * ts),
        ],
    );
    observation.upgrades.push(UpgradeKind::Entrenchment);
    observation.tick = 0;
    let mut memory = AiDecisionMemory::for_profile(&crate::ai_core::profiles::JEFFS_AI);
    memory.sync_defender_posture(&observation);
    observation.tick = rts_rules::balance::ENTRENCHMENT_DIG_IN_TICKS;
    memory.sync_defender_posture(&observation);

    observation.tick += 9;
    observation.owned[0].x += ts;
    observation.owned[0].state = AiEntityState::Move;
    memory.sync_defender_posture(&observation);
    let selected = select_defensive_interceptors(
        &observation,
        &memory,
        vec![20, 30, 10],
        (20.5 * ts, 10.5 * ts),
        10,
        false,
    );

    assert_eq!(selected, vec![10]);
    assert_eq!(
        memory.estimated_entrenchment_ticks(&observation, 20),
        rts_rules::balance::ENTRENCHMENT_DIG_IN_TICKS
    );
}

#[test]
fn defensive_interceptors_prioritize_anti_armor_and_refuse_rifle_only_sacrifices() {
    let ts = config::TILE_SIZE as f32;
    let observation = observation(
        AiEconomy {
            steel: 0,
            oil: 0,
            supply_used: 4,
            supply_cap: 10,
        },
        vec![
            combat_at(10, EntityKind::Rifleman, 10.5 * ts, 10.5 * ts),
            combat_at(20, EntityKind::Rifleman, 11.5 * ts, 10.5 * ts),
            combat_at(30, EntityKind::Tank, 12.5 * ts, 10.5 * ts),
        ],
    );
    let memory = AiDecisionMemory::for_profile(&crate::ai_core::profiles::JEFFS_AI);

    let with_tank = select_defensive_interceptors(
        &observation,
        &memory,
        vec![10, 20, 30],
        (20.5 * ts, 10.5 * ts),
        600,
        true,
    );
    let rifles_only = select_defensive_interceptors(
        &observation,
        &memory,
        vec![10, 20],
        (20.5 * ts, 10.5 * ts),
        600,
        true,
    );

    assert_eq!(with_tank.first(), Some(&30));
    assert!(rifles_only.is_empty());
}

#[test]
fn river_natural_reinforcement_is_symmetric_and_map_bounded() {
    let ts = config::TILE_SIZE as f32;
    let candidates = vec![10, 20, 30, 40];
    for (start, site) in [((9, 9), (15, 30)), ((116, 116), (108, 93))] {
        let mut observation = observation(
            AiEconomy {
                steel: 0,
                oil: 0,
                supply_used: 4,
                supply_cap: 10,
            },
            vec![
                combat_at(10, EntityKind::Rifleman, 10.5 * ts, 10.5 * ts),
                combat_at(20, EntityKind::Rifleman, 11.5 * ts, 10.5 * ts),
                combat_at(30, EntityKind::Tank, 12.5 * ts, 10.5 * ts),
                combat_at(40, EntityKind::Tank, 13.5 * ts, 10.5 * ts),
            ],
        );
        observation.map = AiMapSummary {
            width: 126,
            height: 126,
            tile_size: config::TILE_SIZE,
        };
        observation.own_start_tile = start;
        observation.pending_builds.push(AiBuildIntent::to_site(
            99,
            EntityKind::ResourceDepot,
            site.0,
            site.1,
        ));
        let memory = AiDecisionMemory::for_profile(&crate::ai_core::profiles::JEFFS_AI);

        let selected = select_defensive_interceptors(
            &observation,
            &memory,
            candidates.clone(),
            (20.5 * ts, 10.5 * ts),
            1,
            true,
        );
        let selected_kind_count = |kind| {
            selected
                .iter()
                .filter(|id| {
                    observation
                        .owned
                        .iter()
                        .any(|unit| unit.id == **id && unit.kind == kind)
                })
                .count()
        };
        assert_eq!(selected_kind_count(EntityKind::Tank), 2, "start {start:?}");
        assert_eq!(
            selected_kind_count(EntityKind::Rifleman),
            2,
            "start {start:?}"
        );
    }

    let observation = observation(
        AiEconomy {
            steel: 0,
            oil: 0,
            supply_used: 4,
            supply_cap: 10,
        },
        vec![
            combat_at(10, EntityKind::Rifleman, 10.5 * ts, 10.5 * ts),
            combat_at(20, EntityKind::Rifleman, 11.5 * ts, 10.5 * ts),
            combat_at(30, EntityKind::Tank, 12.5 * ts, 10.5 * ts),
            combat_at(40, EntityKind::Tank, 13.5 * ts, 10.5 * ts),
        ],
    );
    let memory = AiDecisionMemory::for_profile(&crate::ai_core::profiles::JEFFS_AI);
    let selected = select_defensive_interceptors(
        &observation,
        &memory,
        candidates,
        (20.5 * ts, 10.5 * ts),
        1,
        true,
    );
    assert_eq!(selected.len(), 1);
}

#[test]
fn canonical_profiles_never_schedule_disabled_supply_depots() {
    let observation = observation(
        AiEconomy {
            steel: 1_000,
            oil: 1_000,
            supply_used: 48,
            supply_cap: 50,
        },
        vec![
            building(1, EntityKind::ResourceDepot, Some(0)),
            building(2, EntityKind::Barracks, Some(0)),
            worker(3, AiEntityState::Idle),
        ],
    );

    for profile in crate::ai_core::profiles::required_profiles() {
        let decision = decide(
            &observation,
            profile,
            &mut AiDecisionMemory::for_profile(profile),
        );
        assert!(
            !decision.intents.iter().any(|intent| matches!(
                intent,
                AiIntent::Build {
                    kind: EntityKind::Depot
                }
            )),
            "{} must not plan a disabled Supply Depot",
            profile.id,
        );
        assert!(
            !decision.commands.iter().any(|command| matches!(
                command,
                Command::Build {
                    building: EntityKind::Depot,
                    ..
                }
            )),
            "{} must not issue a disabled Supply Depot build command",
            profile.id,
        );
    }
}

#[test]
fn expansion_search_skips_occupied_natural_and_chooses_next_resource_site() {
    let mut obs = observation(
        AiEconomy {
            steel: 1300,
            oil: 300,
            supply_used: 60,
            supply_cap: 120,
        },
        vec![building_at(
            1,
            EntityKind::ResourceDepot,
            None,
            8.5 * config::TILE_SIZE as f32,
            8.5 * config::TILE_SIZE as f32,
        )],
    );
    obs.map.width = 128;
    obs.map.height = 128;
    obs.resources = base_site_resources(300, (30, 30), 128);
    let profile = &crate::ai_core::profiles::AI_2_1;
    let policy = profile.production_expansion.unwrap();
    let first = expansion::expansion_resource_depot_site(
        &obs,
        policy,
        EntityKind::ResourceDepot,
        profile.id,
        &mut |_, _, _| true,
    )
    .expect("natural site");
    let center = building_center(first, EntityKind::ResourceDepot, obs.map.tile_size).unwrap();
    obs.owned.push(building_at(
        2,
        EntityKind::ResourceDepot,
        None,
        center.0,
        center.1,
    ));
    assert!(
        expansion::expansion_resource_depot_site(
            &obs,
            policy,
            EntityKind::ResourceDepot,
            profile.id,
            &mut |_, _, _| true,
        )
        .is_none(),
        "the occupied natural must not count as another expansion"
    );
    let extra: Vec<_> = obs
        .resources
        .iter()
        .filter(|r| r.id >= 300)
        .cloned()
        .map(|mut r| {
            r.id += 1000;
            r.x += 45.0 * obs.map.tile_size as f32;
            r.y += 30.0 * obs.map.tile_size as f32;
            r
        })
        .collect();
    obs.resources.extend(extra);
    let next = expansion::expansion_resource_depot_site(
        &obs,
        policy,
        EntityKind::ResourceDepot,
        profile.id,
        &mut |_, _, _| true,
    )
    .expect("next unoccupied resource site");
    let next_center = building_center(next, EntityKind::ResourceDepot, obs.map.tile_size).unwrap();
    assert!(
        dist2(center.0, center.1, next_center.0, next_center.1)
            > (10.0 * obs.map.tile_size as f32).powi(2)
    );
}

#[test]
fn expansion_spacing_counts_scaffolds_visible_depots_and_pending_sites() {
    let site = (30, 30);
    let center = building_center(site, EntityKind::ResourceDepot, config::TILE_SIZE).unwrap();
    for state in 0..4 {
        let mut obs = observation(
            AiEconomy {
                steel: 1300,
                oil: 300,
                supply_used: 60,
                supply_cap: 120,
            },
            vec![],
        );
        let mut depot = building_at(
            2,
            EntityKind::ResourceDepot,
            None,
            center.0 + 10.0 * config::TILE_SIZE as f32,
            center.1,
        );
        match state {
            0 => {
                depot.is_complete = false;
                obs.owned.push(depot);
            }
            1 => {
                depot.owner = 2;
                obs.visible_enemies.push(depot);
            }
            2 => {
                depot.owner = 2;
                obs.visible_allies.push(depot);
            }
            _ => obs.pending_builds.push(AiBuildIntent::to_site(
                9,
                EntityKind::ResourceDepot,
                40,
                30,
            )),
        }
        let r = resource(1000, EntityKind::Steel, center.0, center.1);
        assert!(
            expansion::expansion_site_candidate(
                &obs,
                EntityKind::ResourceDepot,
                site.0,
                site.1,
                &[&r]
            )
            .is_none(),
            "state {state} at exactly ten tiles"
        );
    }
    let obs = observation(
        AiEconomy {
            steel: 1300,
            oil: 300,
            supply_used: 60,
            supply_cap: 120,
        },
        vec![building_at(
            2,
            EntityKind::ResourceDepot,
            None,
            center.0 + 10.0 * config::TILE_SIZE as f32 + 1.0,
            center.1,
        )],
    );
    let r = resource(1000, EntityKind::Steel, center.0, center.1);
    assert!(expansion::expansion_site_candidate(
        &obs,
        EntityKind::ResourceDepot,
        site.0,
        site.1,
        &[&r]
    )
    .is_some());
}

#[test]
fn ai_2_1_classic_expands_to_three_separate_bases() {
    if crate::skip_unless_full_ai("ai_2_1_classic_expands_to_three_separate_bases") {
        return;
    }
    assert_classic_three_separate_bases(false);
}

#[test]
fn ai_2_1_classic_under_pressure_reaches_three_separate_bases() {
    if crate::skip_unless_full_ai("ai_2_1_classic_under_pressure_reaches_three_separate_bases") {
        return;
    }
    assert_classic_three_separate_bases(true);
}

fn assert_classic_three_separate_bases(under_pressure: bool) {
    use crate::live::{AiAlivePolicy, AiController, CanonicalAiTickDriver};
    use rts_sim::game::{map::Map, Game, PlayerInit};
    for player_id in [1, 2] {
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
        let mut game = Game::new_with_random_ai_profiles_and_map_metadata(
            &players,
            0,
            map,
            Map::metadata_for_name("Classic").unwrap(),
        );
        let start = game.start_payload();
        let mut controllers = if under_pressure {
            vec![
                AiController::with_profile_id(
                    1,
                    if player_id == 1 {
                        "ai_2_1"
                    } else {
                        "ai_2_1_pre_third_base"
                    },
                ),
                AiController::with_profile_id(
                    2,
                    if player_id == 2 {
                        "ai_2_1"
                    } else {
                        "ai_2_1_pre_third_base"
                    },
                ),
            ]
        } else {
            vec![AiController::with_profile_id(player_id, "ai_2_1")]
        };
        let mut completed = false;
        for tick in 0..15000 {
            CanonicalAiTickDriver::run(
                &mut game,
                &mut controllers,
                AiAlivePolicy::StartingPrimaryBase,
            );
            game.tick();
            if tick % 30 != 0 {
                continue;
            }
            let obs = AiObservation::from_snapshot_with_alive(
                &start,
                &game.snapshot_for(player_id),
                player_id,
                [],
                None,
            )
            .unwrap();
            let depots: Vec<_> = obs
                .owned
                .iter()
                .filter(|e| e.kind == EntityKind::ResourceDepot)
                .collect();
            for (i, depot) in depots.iter().enumerate() {
                for other in &depots[i + 1..] {
                    assert!(
                        dist2(depot.x, depot.y, other.x, other.y)
                            > (10.0 * obs.map.tile_size as f32).powi(2),
                        "player {player_id}, tick {tick}: duplicate base location"
                    );
                }
            }
            if depots.iter().filter(|d| d.is_complete).count() >= 3 {
                println!("pressure={under_pressure}, player {player_id}: third separate base completed at tick {} ({:.1}s), positions={:?}", game.tick_count(), game.tick_count() as f32 / 30.0, depots.iter().map(|d| (d.x, d.y)).collect::<Vec<_>>());
                completed = true;
                break;
            }
        }
        assert!(
            completed,
            "player {player_id}: no completed third base by tick 15000"
        );
    }
}
