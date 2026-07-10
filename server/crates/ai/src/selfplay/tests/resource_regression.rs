use std::collections::{BTreeMap, BTreeSet};

use super::super::player_view::{is_complete, kind_of, PlayerView};
use super::super::scripts::{ProfileBackedScript, ScriptedPlayer};
use crate::config;
use rts_sim::game::command::SimCommand as Command;
use rts_sim::game::entity::EntityKind;
use rts_sim::game::{Game, PlayerInit};
use rts_sim::protocol::{EntityView, Snapshot, StartPayload};

#[derive(Default)]
struct ResourceRegressionEvidence {
    pre_expansion_steel_gather_tick: Option<u32>,
    first_pump_jack_build_tick: Option<u32>,
    first_mineable_oil_tick: Option<u32>,
    first_second_completed_city_centre_tick: Option<u32>,
}

const POINT_IN_RECT_EPS_PX: f32 = 0.001;

fn profile_players() -> Vec<PlayerInit> {
    vec![
        PlayerInit {
            id: 1,
            team_id: 1,
            faction_id: "kriegsia".to_string(),
            name: "AI Resource Regression".into(),
            color: "#4cc9f0".into(),
            is_ai: true,
        },
        PlayerInit {
            id: 2,
            team_id: 2,
            faction_id: "kriegsia".to_string(),
            name: "AI Mirror".into(),
            color: "#f72585".into(),
            is_ai: true,
        },
    ]
}

fn gather_node_kind(start: &StartPayload, node: u32) -> Option<EntityKind> {
    start
        .map
        .resources
        .iter()
        .find(|resource| resource.id == node)
        .and_then(|resource| resource.kind.parse().ok())
}

fn completed_city_centres(snapshot: &Snapshot, player_id: u32) -> Vec<&EntityView> {
    snapshot
        .entities
        .iter()
        .filter(|entity| entity.owner == player_id)
        .filter(|entity| kind_of(entity) == Some(EntityKind::CityCentre))
        .filter(|entity| is_complete(entity))
        .collect()
}

fn resource_remaining(start: &StartPayload, snapshot: &Snapshot, node: u32) -> u32 {
    snapshot
        .resource_deltas
        .iter()
        .find(|delta| delta.id == node)
        .map(|delta| delta.remaining)
        .unwrap_or_else(|| {
            if start
                .map
                .resources
                .iter()
                .any(|resource| resource.id == node)
            {
                1
            } else {
                0
            }
        })
}

fn resource_mineable_by_completed_city_centre(
    start: &StartPayload,
    snapshot: &Snapshot,
    player_id: u32,
    node: u32,
) -> bool {
    let Some(resource) = start
        .map
        .resources
        .iter()
        .find(|resource| resource.id == node)
    else {
        return false;
    };
    if resource_remaining(start, snapshot, node) == 0 {
        return false;
    }
    let range_px = config::MINING_CC_RANGE_TILES * start.map.tile_size as f32;
    let range2 = range_px * range_px + 0.01;
    completed_city_centres(snapshot, player_id)
        .iter()
        .any(|cc| {
            let dx = cc.x - resource.x;
            let dy = cc.y - resource.y;
            dx * dx + dy * dy <= range2
        })
}

fn has_free_mineable_resource(
    start: &StartPayload,
    snapshot: &Snapshot,
    player_id: u32,
    kind: EntityKind,
) -> bool {
    let mut occupied_nodes: BTreeSet<u32> = snapshot
        .entities
        .iter()
        .filter(|entity| entity.owner == player_id)
        .filter(|entity| kind_of(entity) == Some(EntityKind::Worker))
        .filter_map(|entity| entity.latched_node)
        .collect();
    for pump_jack in snapshot
        .entities
        .iter()
        .filter(|entity| entity.owner == player_id)
        .filter(|entity| kind_of(entity) == Some(EntityKind::PumpJack))
    {
        occupied_nodes.extend(oil_nodes_overlapping_pump_jack_entity(
            start, snapshot, pump_jack,
        ));
    }
    start.map.resources.iter().any(|resource| {
        resource.kind.parse::<EntityKind>().ok() == Some(kind)
            && !occupied_nodes.contains(&resource.id)
            && resource_mineable_by_completed_city_centre(start, snapshot, player_id, resource.id)
    })
}

fn oil_nodes_overlapping_pump_jack_entity(
    start: &StartPayload,
    snapshot: &Snapshot,
    pump_jack: &EntityView,
) -> Vec<u32> {
    let Some(stats) = config::building_stats(EntityKind::PumpJack) else {
        return Vec::new();
    };
    let tile_size = start.map.tile_size as f32;
    let half_w = stats.foot_w as f32 * tile_size * 0.5;
    let half_h = stats.foot_h as f32 * tile_size * 0.5;
    start
        .map
        .resources
        .iter()
        .filter(|resource| resource.kind.parse::<EntityKind>().ok() == Some(EntityKind::Oil))
        .filter(|resource| resource_remaining(start, snapshot, resource.id) > 0)
        .filter(|resource| {
            resource.x >= pump_jack.x - half_w - POINT_IN_RECT_EPS_PX
                && resource.x <= pump_jack.x + half_w + POINT_IN_RECT_EPS_PX
                && resource.y >= pump_jack.y - half_h - POINT_IN_RECT_EPS_PX
                && resource.y <= pump_jack.y + half_h + POINT_IN_RECT_EPS_PX
        })
        .map(|resource| resource.id)
        .collect()
}

fn pump_jack_build_target_oil_node(
    start: &StartPayload,
    snapshot: &Snapshot,
    tile_x: u32,
    tile_y: u32,
) -> Option<u32> {
    let stats = config::building_stats(EntityKind::PumpJack)?;
    let tile_size = start.map.tile_size as f32;
    let min_x = tile_x as f32 * tile_size;
    let min_y = tile_y as f32 * tile_size;
    let max_x = tile_x.saturating_add(stats.foot_w) as f32 * tile_size;
    let max_y = tile_y.saturating_add(stats.foot_h) as f32 * tile_size;
    start
        .map
        .resources
        .iter()
        .filter(|resource| resource.kind.parse::<EntityKind>().ok() == Some(EntityKind::Oil))
        .filter(|resource| resource_remaining(start, snapshot, resource.id) > 0)
        .find(|resource| {
            resource.x >= min_x - POINT_IN_RECT_EPS_PX
                && resource.x <= max_x + POINT_IN_RECT_EPS_PX
                && resource.y >= min_y - POINT_IN_RECT_EPS_PX
                && resource.y <= max_y + POINT_IN_RECT_EPS_PX
        })
        .map(|resource| resource.id)
}

fn run_resource_regression_profile(max_ticks: u32) -> ResourceRegressionEvidence {
    let players = profile_players();
    let mut game = Game::new_without_ai_controllers(&players, 0x4100_0004);
    let start = game.start_payload();
    let mut scripts: Vec<Box<dyn ScriptedPlayer>> = vec![
        Box::new(ProfileBackedScript::economy_only(1)),
        Box::new(ProfileBackedScript::economy_only(2)),
    ];
    let mut evidence = ResourceRegressionEvidence::default();

    for tick in 0..max_ticks {
        let alive_player_ids = game.alive_players();
        let snapshots: BTreeMap<u32, Snapshot> = players
            .iter()
            .map(|player| (player.id, game.snapshot_for(player.id)))
            .collect();
        let player_one_snapshot = &snapshots[&1];
        if evidence.first_second_completed_city_centre_tick.is_none()
            && completed_city_centres(player_one_snapshot, 1).len() >= 2
        {
            evidence.first_second_completed_city_centre_tick = Some(tick);
        }
        if evidence.first_mineable_oil_tick.is_none()
            && has_free_mineable_resource(&start, player_one_snapshot, 1, EntityKind::Oil)
        {
            evidence.first_mineable_oil_tick = Some(tick);
        }

        let mut commands = Vec::new();
        for script in &mut scripts {
            let pid = script.player_id();
            let Some(snapshot) = snapshots.get(&pid) else {
                continue;
            };
            let view = PlayerView {
                player_id: pid,
                tick,
                start: &start,
                snapshot,
                alive_player_ids: &alive_player_ids,
            };
            commands.extend(
                script
                    .commands(view)
                    .into_iter()
                    .map(|command| (pid, command)),
            );
        }

        for (player_id, command) in commands {
            if player_id == 1 {
                if let Command::Gather { node, .. } = &command {
                    let kind = gather_node_kind(&start, *node);
                    let has_free_steel = has_free_mineable_resource(
                        &start,
                        player_one_snapshot,
                        1,
                        EntityKind::Steel,
                    );
                    let has_free_oil =
                        has_free_mineable_resource(&start, player_one_snapshot, 1, EntityKind::Oil);
                    if player_one_snapshot.supply_used >= 20
                        && player_one_snapshot.supply_used <= 25
                        && has_free_steel
                        && !has_free_oil
                    {
                        assert_eq!(
                            kind,
                            Some(EntityKind::Steel),
                            "pre-expansion gather at tick {tick} targeted {kind:?} while only steel was mineable"
                        );
                        evidence.pre_expansion_steel_gather_tick.get_or_insert(tick);
                    }
                    assert_ne!(
                        kind,
                        Some(EntityKind::Oil),
                        "oil at tick {tick} should use Pump Jack construction, not direct gather"
                    );
                }
                if let Command::Build {
                    building: EntityKind::PumpJack,
                    tile_x,
                    tile_y,
                    ..
                } = &command
                {
                    let Some(node) = pump_jack_build_target_oil_node(
                        &start,
                        player_one_snapshot,
                        *tile_x,
                        *tile_y,
                    ) else {
                        panic!("Pump Jack build at tick {tick} did not overlap a live oil patch");
                    };
                    assert!(
                        resource_mineable_by_completed_city_centre(
                            &start,
                            player_one_snapshot,
                            1,
                            node
                        ),
                        "Pump Jack build at tick {tick} targeted a known but non-mineable oil node"
                    );
                    evidence.first_pump_jack_build_tick.get_or_insert(tick);
                }
            }
            game.enqueue(player_id, command);
        }

        game.tick();
    }

    evidence
}

#[test]
fn profile_backed_ai_prefers_mineable_steel_over_known_non_mineable_oil() {
    if crate::skip_unless_full_ai(
        "profile_backed_ai_prefers_mineable_steel_over_known_non_mineable_oil",
    ) {
        return;
    }
    let evidence = run_resource_regression_profile(6_000);

    assert!(
        evidence.pre_expansion_steel_gather_tick.is_some(),
        "expected a low-to-mid supply pre-expansion steel gather while oil was known but not mineable"
    );
}

#[test]
fn profile_backed_ai_assigns_oil_after_expansion_city_centre_completes() {
    if crate::skip_unless_full_ai(
        "profile_backed_ai_assigns_oil_after_expansion_city_centre_completes",
    ) {
        return;
    }
    let evidence = run_resource_regression_profile(9_000);

    assert!(
        evidence.first_second_completed_city_centre_tick.is_some(),
        "expected AI 1.0 economy progression to complete an expansion City Centre"
    );
    assert!(
        evidence.first_mineable_oil_tick.is_some(),
        "expected expansion completion to make at least one oil node mineable"
    );
    assert!(
        evidence.first_pump_jack_build_tick.is_some(),
        "expected profile-backed economy to assign a worker to build a Pump Jack after expansion"
    );
    assert!(
        evidence.first_pump_jack_build_tick >= evidence.first_mineable_oil_tick,
        "Pump Jack build should not precede the first mineable-oil tick"
    );
}
