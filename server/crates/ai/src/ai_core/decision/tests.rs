use super::defense::main_steel_cluster_center;
use super::frontal::FrontalWaveBlocker;
use super::geometry::{building_center, normalized_direction, tile_center};
use super::*;

use crate::ai_core::observation::{
    AiEconomy, AiEntityState, AiEntitySummary, AiMapSummary, AiObservation, AiPlayerSummary,
    AiResourceSummary,
};
use crate::ai_core::profiles::{AiProfile, JEFFS_AI};
use rts_sim::game::command::SimCommand as Command;

mod economy_manager_tests;
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
fn jeffs_ai_waits_for_five_ready_tanks_and_a_scout_car() {
    let mut observation = observation(
        AiEconomy {
            steel: 1_000,
            oil: 1_000,
            supply_used: 20,
            supply_cap: 100,
        },
        vec![
            combat(1, EntityKind::Tank),
            combat(2, EntityKind::Tank),
            combat(3, EntityKind::Tank),
            combat(4, EntityKind::Tank),
            combat(5, EntityKind::ScoutCar),
            combat(6, EntityKind::Rifleman),
            combat(7, EntityKind::Rifleman),
        ],
    );
    observation.upgrades.push(UpgradeKind::Methamphetamines);
    let attack = JEFFS_AI.tech_transition.expect("armored transition").attack;
    let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);

    let waiting = plan_frontal_wave(
        &observation,
        attack,
        &mut memory,
        &JEFFS_AI,
        &BTreeSet::new(),
    );

    assert!(!waiting.required_units_ready);
    assert!(waiting
        .blockers
        .contains(&FrontalWaveBlocker::WaitingForTank));
    assert!(!waiting.should_attack());

    observation.owned.push(combat(8, EntityKind::Tank));
    let ready = plan_frontal_wave(
        &observation,
        attack,
        &mut memory,
        &JEFFS_AI,
        &BTreeSet::new(),
    );

    assert!(ready.required_units_ready);
    assert!(!ready.blockers.contains(&FrontalWaveBlocker::WaitingForTank));
    assert!(ready.should_attack());
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
            building(1, EntityKind::CityCentre, Some(0)),
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
