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
    first_oil_gather_tick: Option<u32>,
    first_mineable_oil_tick: Option<u32>,
    first_second_completed_city_centre_tick: Option<u32>,
}

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
            start
                .map
                .resources
                .iter()
                .any(|resource| resource.id == node)
                .then_some(1)
                .unwrap_or(0)
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
    let occupied_nodes: BTreeSet<u32> = snapshot
        .entities
        .iter()
        .filter(|entity| entity.owner == player_id)
        .filter(|entity| kind_of(entity) == Some(EntityKind::Worker))
        .filter_map(|entity| entity.latched_node)
        .collect();
    start.map.resources.iter().any(|resource| {
        resource.kind.parse::<EntityKind>().ok() == Some(kind)
            && !occupied_nodes.contains(&resource.id)
            && resource_mineable_by_completed_city_centre(start, snapshot, player_id, resource.id)
    })
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
                    if kind == Some(EntityKind::Oil) {
                        assert!(
                            resource_mineable_by_completed_city_centre(
                                &start,
                                player_one_snapshot,
                                1,
                                *node
                            ),
                            "oil gather at tick {tick} targeted a known but non-mineable node"
                        );
                        evidence.first_oil_gather_tick.get_or_insert(tick);
                    }
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
        evidence.first_oil_gather_tick.is_some(),
        "expected profile-backed economy to assign a worker to oil after expansion"
    );
    assert!(
        evidence.first_oil_gather_tick >= evidence.first_mineable_oil_tick,
        "oil gather should not precede the first mineable-oil tick"
    );
}
