use std::collections::BTreeMap;
use std::path::PathBuf;

use rts_protocol::{
    Command, InitialCamera, LabCheckpointScenarioV1 as ProtocolLabCheckpointScenarioV1,
    LabScenarioLabMetadata, LabScenarioPayload, LabVisionMode, DEFAULT_FACTION_ID,
};
use rts_rules::balance::{building_stats, unit_stats};
use rts_sim::game::entity::EntityKind;
use rts_sim::game::lab::{
    LabCommandOptions, LabOp, LabOpOutcome, LabSetCompletedResearch, LabSetPlayerResources,
    LabSpawnEntity,
};
use rts_sim::game::map::Map;
use rts_sim::game::upgrade;
use rts_sim::game::{Game, PlayerInit};

const OUT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/lab-scenarios/supply-300-hellhole.json"
);
const MAP_NAME: &str = "No Terrain";
const SCENARIO_NAME: &str = "Supply 300 Hellhole";
const SEED: u32 = 0x5a00_0300;
const TILE: f32 = 32.0;
const TARGET_SUPPLY: u32 = 300;
const BUILD_SHA: &str = "bundled-lab-scenario-asset-v1";
const CENTER: (f32, f32) = (63.0 * TILE, 63.0 * TILE);
const SHUTTLE_OFFSET_TILES: f32 = 18.0;

fn main() {
    let out = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(OUT));
    if let Err(err) = run(out) {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

fn run(out: PathBuf) -> Result<(), String> {
    let mut game = blank_no_terrain_lab()?;
    clear_default_entities(&mut game)?;
    grant_lab_state(&mut game)?;

    let composition = composition_300_supply()?;
    let mut units_by_player = BTreeMap::<u32, Vec<(u32, EntityKind)>>::new();
    spawn_building_rings(&mut game)?;
    for player_id in 1..=4 {
        let positions = positions_for_player(player_id, composition.len());
        for (kind, (x, y)) in composition.iter().copied().zip(positions) {
            let id = spawn(&mut game, player_id, kind, x, y, true)?;
            units_by_player
                .entry(player_id)
                .or_default()
                .push((id, kind));
        }
    }
    seed_initial_orders(&mut game, &units_by_player)?;

    let sim_scenario = game
        .export_lab_checkpoint_scenario(SCENARIO_NAME.to_string(), BUILD_SHA)
        .map_err(|err| format!("failed to export checkpoint scenario: {err:?}"))?;
    let protocol_scenario = add_protocol_lab_metadata(sim_scenario)?;
    let payload = LabScenarioPayload::Checkpoint(protocol_scenario);
    let json = serde_json::to_string(&payload)
        .map_err(|err| format!("failed to serialize scenario JSON: {err}"))?
        + "\n";
    if json.len() > 1_000_000 {
        return Err(format!(
            "scenario JSON is too large: {} bytes > 1,000,000",
            json.len()
        ));
    }
    std::fs::write(&out, json)
        .map_err(|err| format!("failed to write {}: {err}", out.display()))?;

    let restored = rts_server::lab_scenarios::load_lab_scenario_by_id("supply-300-hellhole")
        .and_then(|loaded| loaded.build_game())?;
    let supply: Vec<_> = restored
        .snapshot_full_for(1)
        .player_resources
        .iter()
        .map(|p| (p.id, p.supply_used, p.supply_cap))
        .collect();
    println!(
        "wrote {} ({} bytes), supply {:?}",
        out.display(),
        std::fs::metadata(&out).map(|m| m.len()).unwrap_or(0),
        supply
    );
    Ok(())
}

fn blank_no_terrain_lab() -> Result<Game, String> {
    let players: Vec<_> = (1..=4)
        .map(|id| PlayerInit {
            id,
            team_id: id,
            faction_id: DEFAULT_FACTION_ID.to_string(),
            name: format!("Hellhole {id}"),
            color: match id {
                1 => "#0072b2",
                2 => "#d55e00",
                3 => "#009e73",
                _ => "#cc79a7",
            }
            .to_string(),
            is_ai: false,
        })
        .collect();
    let start_players: Vec<_> = players.iter().map(|p| (p.id, p.team_id)).collect();
    let map_metadata = Map::metadata_for_name(MAP_NAME)
        .map_err(|err| format!("cannot load map metadata {MAP_NAME:?}: {err}"))?;
    let map = Map::load_for_players(MAP_NAME, &start_players, SEED)
        .map_err(|err| format!("cannot load map {MAP_NAME:?}: {err}"))?;
    Ok(Game::new_lab(&players, SEED, map, map_metadata))
}

fn clear_default_entities(game: &mut Game) -> Result<(), String> {
    let ids: Vec<_> = game
        .snapshot_full_for(1)
        .entities
        .iter()
        .map(|entity| entity.id)
        .collect();
    for chunk in ids.chunks(400) {
        apply(game, LabOp::DeleteEntities(chunk.to_vec()))?;
    }
    Ok(())
}

fn grant_lab_state(game: &mut Game) -> Result<(), String> {
    for player_id in 1..=4 {
        apply(
            game,
            LabOp::SetPlayerResources(LabSetPlayerResources {
                player_id,
                steel: 99_999,
                oil: 99_999,
            }),
        )?;
        for &upgrade in upgrade::ALL {
            apply(
                game,
                LabOp::SetCompletedResearch(LabSetCompletedResearch {
                    player_id,
                    upgrade,
                    completed: true,
                }),
            )?;
        }
        apply(
            game,
            LabOp::SetPlayerGodMode {
                player_id,
                enabled: true,
            },
        )?;
    }
    Ok(())
}

fn composition_300_supply() -> Result<Vec<EntityKind>, String> {
    let required = [
        EntityKind::Worker,
        EntityKind::Golem,
        EntityKind::Rifleman,
        EntityKind::MachineGunner,
        EntityKind::Panzerfaust,
        EntityKind::AntiTankGun,
        EntityKind::MortarTeam,
        EntityKind::Artillery,
        EntityKind::ScoutCar,
        EntityKind::Tank,
        EntityKind::CommandCar,
    ];
    let filler = [
        EntityKind::Tank,
        EntityKind::Tank,
        EntityKind::ScoutCar,
        EntityKind::CommandCar,
        EntityKind::MachineGunner,
        EntityKind::MortarTeam,
        EntityKind::AntiTankGun,
        EntityKind::Rifleman,
        EntityKind::Panzerfaust,
    ];
    let mut out = required.to_vec();
    let mut supply = supply_for(&out)?;
    let mut index = 0;
    while supply < TARGET_SUPPLY {
        let kind = filler[index % filler.len()];
        index += 1;
        let cost = supply_of(kind)?;
        if supply + cost > TARGET_SUPPLY {
            continue;
        }
        out.push(kind);
        supply += cost;
    }
    Ok(out)
}

fn supply_for(units: &[EntityKind]) -> Result<u32, String> {
    units.iter().copied().map(supply_of).sum()
}

fn supply_of(kind: EntityKind) -> Result<u32, String> {
    if kind == EntityKind::Golem {
        return Ok(0);
    }
    unit_stats(kind)
        .map(|stats| stats.supply)
        .ok_or_else(|| format!("{kind} is not a unit"))
}

fn positions_for_player(player_id: u32, count: usize) -> Vec<(f32, f32)> {
    match player_id {
        1 | 2 => central_positions(player_id, count),
        3 => grid_positions(shuttle_endpoint(1.0, -1.0), count, 10, 64.0),
        4 => grid_positions(shuttle_endpoint(-1.0, -1.0), count, 10, 64.0),
        _ => unreachable!("only four hellhole players are generated"),
    }
}

fn shuttle_endpoint(x_dir: f32, y_dir: f32) -> (f32, f32) {
    (
        CENTER.0 + x_dir * SHUTTLE_OFFSET_TILES * TILE,
        CENTER.1 + y_dir * SHUTTLE_OFFSET_TILES * TILE,
    )
}

fn central_positions(player_id: u32, count: usize) -> Vec<(f32, f32)> {
    let parity = if player_id == 1 { 0 } else { 1 };
    grid_positions(CENTER, count * 2, 24, 64.0)
        .into_iter()
        .enumerate()
        .filter_map(|(index, pos)| (index % 2 == parity).then_some(pos))
        .take(count)
        .collect()
}

fn grid_positions(
    center: (f32, f32),
    count: usize,
    columns: usize,
    spacing: f32,
) -> Vec<(f32, f32)> {
    let rows = count.div_ceil(columns);
    let width = (columns.saturating_sub(1)) as f32 * spacing;
    let height = (rows.saturating_sub(1)) as f32 * spacing;
    (0..count)
        .map(|index| {
            let col = index % columns;
            let row = index / columns;
            (
                center.0 - width * 0.5 + col as f32 * spacing,
                center.1 - height * 0.5 + row as f32 * spacing,
            )
        })
        .collect()
}

fn spawn_building_rings(game: &mut Game) -> Result<(), String> {
    let clusters = [(1, 4, 54), (2, 94, 54), (3, 54, 4), (4, 54, 104)];
    for (player_id, origin_x, origin_y) in clusters {
        for (kind, dx, dy) in [
            (EntityKind::CityCentre, 0, 0),
            (EntityKind::Barracks, 4, 0),
            (EntityKind::Factory, 8, 0),
            (EntityKind::Steelworks, 12, 0),
            (EntityKind::ResearchComplex, 16, 0),
            (EntityKind::Depot, 0, 4),
            (EntityKind::Depot, 3, 4),
            (EntityKind::Depot, 6, 4),
            (EntityKind::Depot, 9, 4),
            (EntityKind::Depot, 12, 4),
        ] {
            let (x, y) = building_center(kind, origin_x + dx, origin_y + dy)?;
            spawn(game, player_id, kind, x, y, true)?;
        }
    }
    Ok(())
}

fn building_center(kind: EntityKind, tile_x: u32, tile_y: u32) -> Result<(f32, f32), String> {
    let stats = building_stats(kind).ok_or_else(|| format!("{kind} is not a building"))?;
    Ok((
        (tile_x as f32 + stats.foot_w as f32 * 0.5) * TILE,
        (tile_y as f32 + stats.foot_h as f32 * 0.5) * TILE,
    ))
}

fn spawn(
    game: &mut Game,
    owner: u32,
    kind: EntityKind,
    x: f32,
    y: f32,
    completed: bool,
) -> Result<u32, String> {
    match apply(
        game,
        LabOp::SpawnEntity(LabSpawnEntity {
            owner,
            kind,
            x,
            y,
            completed,
        }),
    )? {
        LabOpOutcome::Spawned { entity_id } => Ok(entity_id),
        other => Err(format!("unexpected spawn outcome for {kind}: {other:?}")),
    }
}

fn seed_initial_orders(
    game: &mut Game,
    units_by_player: &BTreeMap<u32, Vec<(u32, EntityKind)>>,
) -> Result<(), String> {
    issue(
        game,
        1,
        Command::AttackMove {
            units: unit_ids(units_by_player, 1),
            x: CENTER.0 + 170.0,
            y: CENTER.1,
            queued: false,
        },
    )?;
    issue(
        game,
        2,
        Command::AttackMove {
            units: unit_ids(units_by_player, 2),
            x: CENTER.0 - 170.0,
            y: CENTER.1,
            queued: false,
        },
    )?;
    for player_id in [1, 2] {
        let supports: Vec<u32> = units_by_player
            .get(&player_id)
            .into_iter()
            .flatten()
            .filter(|(_, kind)| matches!(kind, EntityKind::AntiTankGun | EntityKind::Artillery))
            .map(|(id, _)| *id)
            .collect();
        if !supports.is_empty() {
            issue(
                game,
                player_id,
                Command::SetupAntiTankGuns {
                    units: supports,
                    x: if player_id == 1 {
                        CENTER.0 + 200.0
                    } else {
                        CENTER.0 - 200.0
                    },
                    y: CENTER.1,
                    queued: false,
                },
            )?;
        }
    }
    Ok(())
}

fn unit_ids(units_by_player: &BTreeMap<u32, Vec<(u32, EntityKind)>>, player_id: u32) -> Vec<u32> {
    units_by_player
        .get(&player_id)
        .into_iter()
        .flatten()
        .map(|(id, _)| *id)
        .collect()
}

fn issue(game: &mut Game, player_id: u32, command: Command) -> Result<(), String> {
    game.issue_lab_command_as(
        player_id,
        command,
        LabCommandOptions {
            ignore_command_limits: true,
        },
    )
    .map_err(|err| format!("command for player {player_id} failed: {err:?}"))
}

fn apply(game: &mut Game, op: LabOp) -> Result<LabOpOutcome, String> {
    game.apply_lab_op(op)
        .map_err(|err| format!("lab operation failed: {err:?}"))
}

fn add_protocol_lab_metadata(
    scenario: rts_sim::game::lab::LabCheckpointScenarioV1,
) -> Result<ProtocolLabCheckpointScenarioV1, String> {
    let mut value = serde_json::to_value(scenario)
        .map_err(|err| format!("failed to encode scenario: {err}"))?;
    let metadata = value
        .get_mut("metadata")
        .and_then(|metadata| metadata.as_object_mut())
        .ok_or_else(|| "scenario metadata is not an object".to_string())?;
    metadata.insert(
        "lab".to_string(),
        serde_json::to_value(LabScenarioLabMetadata {
            vision: LabVisionMode::All,
            god_mode_players: vec![1, 2, 3, 4],
            initial_camera: Some(InitialCamera {
                center_x: CENTER.0 as u32,
                center_y: CENTER.1 as u32,
            }),
        })
        .map_err(|err| format!("failed to encode lab metadata: {err}"))?,
    );
    serde_json::from_value(value).map_err(|err| format!("failed to build protocol scenario: {err}"))
}
