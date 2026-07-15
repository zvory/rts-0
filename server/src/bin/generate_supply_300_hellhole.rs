use std::collections::BTreeMap;
use std::path::PathBuf;

use rts_protocol::{
    Command, Event, InitialCamera, LabCheckpointScenarioV1 as ProtocolLabCheckpointScenarioV1,
    LabScenarioLabMetadata, LabScenarioPayload, LabVisionMode, DEFAULT_FACTION_ID,
};
use rts_rules::economy::supply_cost;
use rts_rules::faction::catalog_for;
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
const MAP_NAME: &str = "1v1";
const SCENARIO_NAME: &str = "Supply 300 Hellhole";
const SEED: u32 = 0x5a00_0300;
const TILE: f32 = 32.0;
const TARGET_SUPPLY: u32 = 300;
const BUILD_SHA: &str = "bundled-lab-scenario-asset-v1";
const CENTER: (f32, f32) = (96.0 * TILE, 63.0 * TILE);
const LATTICE_COLUMNS: usize = 16;
const LATTICE_SPACING_X: f32 = 54.0;
const LATTICE_SPACING_Y: f32 = 33.0;
const PARTIAL_HP_MIN: usize = 1;
const PARTIAL_HP_WARMUP_TICKS: u32 = 180;

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
    let mut game = blank_authored_lab()?;
    clear_default_entities(&mut game)?;
    grant_lab_state(&mut game)?;

    let composition = composition_300_supply()?;
    let mut units_by_player = BTreeMap::<u32, Vec<(u32, EntityKind)>>::new();
    let positions = dense_interleaved_positions(composition.len() * 2);
    let mut player_one = composition.clone();
    let mut player_two = composition.clone();
    remove_one(&mut player_one, EntityKind::Worker)?;
    remove_one(&mut player_two, EntityKind::Rifleman)?;
    let worker = spawn(
        &mut game,
        1,
        EntityKind::Worker,
        positions[0].0,
        positions[0].1,
        true,
    )?;
    let rifleman = spawn(
        &mut game,
        2,
        EntityKind::Rifleman,
        positions[1].0,
        positions[1].1,
        true,
    )?;
    units_by_player
        .entry(1)
        .or_default()
        .push((worker, EntityKind::Worker));
    units_by_player
        .entry(2)
        .or_default()
        .push((rifleman, EntityKind::Rifleman));
    issue(
        &mut game,
        2,
        Command::Attack {
            units: vec![rifleman],
            target: worker,
            queued: false,
        },
    )?;
    let warmup_entity_count = game.perf_entity_counts().entities;
    warm_until_partial_hp(&mut game, warmup_entity_count)?;
    enable_god_mode(&mut game)?;

    let mut position_index = 2;
    for index in 0..player_one.len() {
        for (player_id, kind) in [(1, player_one[index]), (2, player_two[index])] {
            let (x, y) = positions[position_index];
            position_index += 1;
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

fn blank_authored_lab() -> Result<Game, String> {
    let players = vec![
        PlayerInit {
            id: 1,
            team_id: 1,
            faction_id: DEFAULT_FACTION_ID.to_string(),
            name: "Hellhole Kriegsia".to_string(),
            color: "#0072b2".to_string(),
            is_ai: false,
        },
        PlayerInit {
            id: 2,
            team_id: 2,
            faction_id: DEFAULT_FACTION_ID.to_string(),
            name: "Hellhole Bravo".to_string(),
            color: "#d55e00".to_string(),
            is_ai: false,
        },
    ];
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
        .filter(|entity| entity.kind != EntityKind::CityCentre.to_string())
        .map(|entity| entity.id)
        .collect();
    for chunk in ids.chunks(400) {
        apply(game, LabOp::DeleteEntities(chunk.to_vec()))?;
    }
    Ok(())
}

fn grant_lab_state(game: &mut Game) -> Result<(), String> {
    for player_id in 1..=2 {
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
    }
    Ok(())
}

fn composition_300_supply() -> Result<Vec<EntityKind>, String> {
    let required = [
        EntityKind::Worker,
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
    let mut out = required.to_vec();
    let mut supply = supply_for(&out)?;
    let mut index = 0;
    while supply < TARGET_SUPPLY {
        let kind = required[index % required.len()];
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

fn remove_one(units: &mut Vec<EntityKind>, kind: EntityKind) -> Result<(), String> {
    let index = units
        .iter()
        .position(|candidate| *candidate == kind)
        .ok_or_else(|| format!("composition is missing required {kind}"))?;
    units.remove(index);
    Ok(())
}

fn supply_of(kind: EntityKind) -> Result<u32, String> {
    let raw_supply = supply_cost(kind);
    if !kind.is_unit() || raw_supply == 0 {
        return Err(format!("{kind} is not an ordinary supply-bearing unit"));
    }
    let catalog = catalog_for(DEFAULT_FACTION_ID)
        .ok_or_else(|| format!("missing faction catalog {DEFAULT_FACTION_ID}"))?;
    if !catalog.allows_unit(kind) {
        return Err(format!(
            "{kind} is not available to faction {DEFAULT_FACTION_ID}"
        ));
    }
    Ok(raw_supply)
}

fn dense_interleaved_positions(count: usize) -> Vec<(f32, f32)> {
    let rows = count.div_ceil(LATTICE_COLUMNS);
    let width = (LATTICE_COLUMNS.saturating_sub(1)) as f32 * LATTICE_SPACING_X;
    let height = (rows.saturating_sub(1)) as f32 * LATTICE_SPACING_Y;
    (0..count)
        .map(|index| {
            let col = index % LATTICE_COLUMNS;
            let row = index / LATTICE_COLUMNS;
            (
                CENTER.0 - width * 0.5 + col as f32 * LATTICE_SPACING_X,
                CENTER.1 - height * 0.5 + row as f32 * LATTICE_SPACING_Y,
            )
        })
        .collect()
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

fn warm_until_partial_hp(game: &mut Game, expected_entities: usize) -> Result<(), String> {
    let mut attack_events = 0usize;
    for _ in 0..PARTIAL_HP_WARMUP_TICKS {
        let events = game.tick();
        attack_events += events
            .iter()
            .flat_map(|(_, events)| events)
            .filter(|event| matches!(event, Event::Attack { .. }))
            .count();
        let snapshot = game.snapshot_full_for(1);
        if snapshot.entities.len() != expected_entities {
            return Err(format!(
                "warmup changed entity count: {} != {expected_entities}",
                snapshot.entities.len()
            ));
        }
        let partial_hp = snapshot
            .entities
            .iter()
            .filter(|entity| entity.hp > 0 && entity.hp < entity.max_hp)
            .count();
        if attack_events > 0 && partial_hp >= PARTIAL_HP_MIN {
            return Ok(());
        }
    }
    Err(format!(
        "warmup did not produce partial HP and attack feedback within {PARTIAL_HP_WARMUP_TICKS} ticks"
    ))
}

fn enable_god_mode(game: &mut Game) -> Result<(), String> {
    for player_id in 1..=2 {
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
            god_mode_players: vec![1, 2],
            initial_camera: Some(InitialCamera {
                center_x: CENTER.0 as u32,
                center_y: CENTER.1 as u32,
            }),
        })
        .map_err(|err| format!("failed to encode lab metadata: {err}"))?,
    );
    serde_json::from_value(value).map_err(|err| format!("failed to build protocol scenario: {err}"))
}
