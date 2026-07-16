use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use rts_protocol::{
    Command, InitialCamera, LabCheckpointScenarioV1 as ProtocolLabCheckpointScenarioV1,
    LabScenarioLabMetadata, LabScenarioPayload, LabVisionMode, DEFAULT_FACTION_ID,
};
use rts_rules::balance::building_stats;
use rts_rules::terrain::MAP_TERRAIN_ROCK;
use rts_server::tools::hellhole_spec::{
    composition_300_supply, respawn_candidates, CENTER, SEED, SHUTTLE_OFFSET_TILES, TILE,
};
use rts_sim::game::entity::EntityKind;
use rts_sim::game::lab::{
    LabCommandOptions, LabError, LabOp, LabOpOutcome, LabSetCompletedResearch,
    LabSetPlayerResources, LabSpawnEntity,
};
use rts_sim::game::map::Map;
use rts_sim::game::upgrade;
use rts_sim::game::{Game, PlayerInit};

const OUT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/lab-scenarios/supply-300-hellhole.json"
);
const MAP_NAME: &str = "No Terrain";
const SCENARIO_NAME: &str = "Supply 300 2v2 Hellhole";
const BUILD_SHA: &str = "bundled-lab-scenario-asset-v2";
const ROCK_CELL_TILES: u32 = 5;
const TARGET_ROCK_TILES: usize = 470;
const UNIT_FOOTPRINT_CLEARANCE_TILES: i32 = 1;
const BUILDING_CLUSTERS: [(u32, u32, u32); 4] = [(1, 4, 54), (2, 94, 54), (3, 54, 4), (4, 54, 104)];
const BUILDING_LAYOUT: [(EntityKind, u32, u32); 10] = [
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
];

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
    let composition = composition_300_supply()?;
    let mut game = blank_hellhole_lab(&composition)?;
    clear_default_entities(&mut game)?;
    grant_lab_state(&mut game)?;

    let mut units_by_player = BTreeMap::<u32, Vec<(u32, EntityKind)>>::new();
    spawn_building_rings(&mut game)?;
    spawn_dense_central_scrum(&mut game, &composition, &mut units_by_player)?;
    for player_id in [3, 4] {
        for (kind, (x, y)) in composition
            .iter()
            .copied()
            .zip(shuttle_positions_for_player(player_id, composition.len()))
        {
            let id = spawn(&mut game, player_id, kind, x, y, true).map_err(|err| {
                format!("failed to spawn player {player_id} {kind} at ({x}, {y}): {err}")
            })?;
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
    let restored = Game::restore_lab_checkpoint_scenario(sim_scenario.clone())
        .map_err(|err| format!("generated checkpoint did not restore: {err:?}"))?;
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

fn blank_hellhole_lab(composition: &[EntityKind]) -> Result<Game, String> {
    let players: Vec<_> = (1..=4)
        .map(|id| PlayerInit {
            id,
            team_id: if id == 1 || id == 3 { 1 } else { 2 },
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
    let mut map = Map::load_for_players(MAP_NAME, &start_players, SEED)
        .map_err(|err| format!("cannot load map {MAP_NAME:?}: {err}"))?;
    let rock_count = populate_sparse_rock_occluders(&mut map, composition)?;
    if rock_count < 128 {
        return Err(format!(
            "sparse Hellhole terrain produced only {rock_count} stone tiles"
        ));
    }
    Ok(Game::new_lab(&players, SEED, map, map_metadata))
}

fn populate_sparse_rock_occluders(
    map: &mut Map,
    composition: &[EntityKind],
) -> Result<usize, String> {
    let occupied = occupied_spawn_tiles(map.size, composition)?;
    let mut candidates = Vec::new();
    for cell_y in (0..map.size).step_by(ROCK_CELL_TILES as usize) {
        for cell_x in (0..map.size).step_by(ROCK_CELL_TILES as usize) {
            let hash = SEED ^ cell_x.wrapping_mul(0x9e37_79b9) ^ cell_y.wrapping_mul(0x85eb_ca6b);
            let tile_x = cell_x + 1 + hash % (ROCK_CELL_TILES - 2);
            let tile_y = cell_y + 1 + hash.rotate_left(13) % (ROCK_CELL_TILES - 2);
            if tile_x >= map.size - 1
                || tile_y >= map.size - 1
                || occupied.contains(&(tile_x, tile_y))
            {
                continue;
            }
            candidates.push((hash, tile_x, tile_y));
        }
    }
    candidates.sort_unstable();
    if candidates.len() < TARGET_ROCK_TILES {
        return Err(format!(
            "Hellhole terrain has only {} valid stone candidates for target {TARGET_ROCK_TILES}",
            candidates.len()
        ));
    }
    for &(_, tile_x, tile_y) in candidates.iter().take(TARGET_ROCK_TILES) {
        map.terrain[(tile_y * map.size + tile_x) as usize] = MAP_TERRAIN_ROCK;
    }
    Ok(TARGET_ROCK_TILES)
}

fn occupied_spawn_tiles(
    map_size: u32,
    composition: &[EntityKind],
) -> Result<BTreeSet<(u32, u32)>, String> {
    let mut occupied = BTreeSet::new();
    for player_id in [3, 4] {
        for position in shuttle_positions_for_player(player_id, composition.len()) {
            reserve_world_point(&mut occupied, map_size, position);
        }
    }

    for (_, origin_x, origin_y) in BUILDING_CLUSTERS {
        for (kind, dx, dy) in BUILDING_LAYOUT {
            let stats = building_stats(kind).ok_or_else(|| format!("{kind} is not a building"))?;
            reserve_rect(
                &mut occupied,
                map_size,
                origin_x as i32 + dx as i32,
                origin_y as i32 + dy as i32,
                origin_x as i32 + dx as i32 + stats.foot_w as i32 - 1,
                origin_y as i32 + dy as i32 + stats.foot_h as i32 - 1,
            );
        }
    }
    Ok(occupied)
}

fn reserve_world_point(reserved: &mut BTreeSet<(u32, u32)>, map_size: u32, position: (f32, f32)) {
    let tile_x = (position.0 / TILE).floor() as i32;
    let tile_y = (position.1 / TILE).floor() as i32;
    reserve_rect(
        reserved,
        map_size,
        tile_x - UNIT_FOOTPRINT_CLEARANCE_TILES,
        tile_y - UNIT_FOOTPRINT_CLEARANCE_TILES,
        tile_x + UNIT_FOOTPRINT_CLEARANCE_TILES,
        tile_y + UNIT_FOOTPRINT_CLEARANCE_TILES,
    );
}

fn reserve_rect(
    reserved: &mut BTreeSet<(u32, u32)>,
    map_size: u32,
    min_x: i32,
    min_y: i32,
    max_x: i32,
    max_y: i32,
) {
    for tile_y in min_y.max(0)..=max_y.min(map_size as i32 - 1) {
        for tile_x in min_x.max(0)..=max_x.min(map_size as i32 - 1) {
            reserved.insert((tile_x as u32, tile_y as u32));
        }
    }
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
        if player_id >= 3 {
            apply(
                game,
                LabOp::SetPlayerGodMode {
                    player_id,
                    enabled: true,
                },
            )?;
        }
    }
    Ok(())
}

fn spawn_dense_central_scrum(
    game: &mut Game,
    composition: &[EntityKind],
    units_by_player: &mut BTreeMap<u32, Vec<(u32, EntityKind)>>,
) -> Result<(), String> {
    let candidates = respawn_candidates();
    let mut next_candidate = 0usize;
    for &kind in composition {
        for player_id in [1, 2] {
            let mut spawned = None;
            while let Some(&(x, y)) = candidates.get(next_candidate) {
                next_candidate += 1;
                match try_spawn(game, player_id, kind, x, y, true)? {
                    Some(id) => {
                        spawned = Some(id);
                        break;
                    }
                    None => continue,
                }
            }
            let id = spawned.ok_or_else(|| {
                format!(
                    "dense Hellhole scrum exhausted {} candidates while placing player {player_id} {kind}",
                    candidates.len()
                )
            })?;
            units_by_player
                .entry(player_id)
                .or_default()
                .push((id, kind));
        }
    }
    Ok(())
}

fn try_spawn(
    game: &mut Game,
    owner: u32,
    kind: EntityKind,
    x: f32,
    y: f32,
    completed: bool,
) -> Result<Option<u32>, String> {
    match game.apply_lab_op(LabOp::SpawnEntity(LabSpawnEntity {
        owner,
        kind,
        x,
        y,
        completed,
    })) {
        Ok(LabOpOutcome::Spawned { entity_id }) => Ok(Some(entity_id)),
        Err(LabError::Placement { .. }) => Ok(None),
        Ok(other) => Err(format!("unexpected spawn outcome for {kind}: {other:?}")),
        Err(err) => Err(format!("lab operation failed: {err:?}")),
    }
}

fn shuttle_positions_for_player(player_id: u32, count: usize) -> Vec<(f32, f32)> {
    let center = match player_id {
        3 => shuttle_endpoint(1.0, -1.0),
        4 => shuttle_endpoint(-1.0, -1.0),
        _ => unreachable!("only shuttle players have endpoint formations"),
    };
    grid_positions(center, count, 10, 64.0)
}

fn shuttle_endpoint(x_dir: f32, y_dir: f32) -> (f32, f32) {
    (
        CENTER.0 + x_dir * SHUTTLE_OFFSET_TILES as f32 * TILE,
        CENTER.1 + y_dir * SHUTTLE_OFFSET_TILES as f32 * TILE,
    )
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
    for (player_id, origin_x, origin_y) in BUILDING_CLUSTERS {
        for (kind, dx, dy) in BUILDING_LAYOUT {
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
            god_mode_players: vec![3, 4],
            initial_camera: Some(InitialCamera {
                center_x: CENTER.0 as u32,
                center_y: CENTER.1 as u32,
            }),
        })
        .map_err(|err| format!("failed to encode lab metadata: {err}"))?,
    );
    serde_json::from_value(value).map_err(|err| format!("failed to build protocol scenario: {err}"))
}
