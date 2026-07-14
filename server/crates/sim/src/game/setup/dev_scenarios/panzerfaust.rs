use super::*;
use crate::game::entity::Order;

enum PanzerfaustInspectionScenario {
    Duel,
    WindupCancel,
    TargetDeath,
    EntrenchedRange,
    Methamphetamines,
}

impl PanzerfaustInspectionScenario {
    fn from_id(id: &str) -> Option<Self> {
        match id {
            "panzerfaust_duel" => Some(Self::Duel),
            "panzerfaust_windup_cancel" => Some(Self::WindupCancel),
            "panzerfaust_target_death" => Some(Self::TargetDeath),
            "panzerfaust_entrenched_range" => Some(Self::EntrenchedRange),
            "panzerfaust_methamphetamines" => Some(Self::Methamphetamines),
            _ => None,
        }
    }

    fn label(&self) -> &'static str {
        match self {
            Self::Duel => "Panzerfaust duel",
            Self::WindupCancel => "Panzerfaust windup cancel",
            Self::TargetDeath => "Panzerfaust target death",
            Self::EntrenchedRange => "Panzerfaust entrenched range",
            Self::Methamphetamines => "Panzerfaust Methamphetamines",
        }
    }
}

impl Game {
    pub fn new_panzerfaust_inspection_scenario(
        scenario_id: &str,
        unit: EntityKind,
        unit_count: usize,
        seed: u32,
    ) -> Result<DevScenarioSetup, String> {
        let scenario = PanzerfaustInspectionScenario::from_id(scenario_id)
            .ok_or_else(|| format!("unsupported Panzerfaust inspection scenario {scenario_id}"))?;
        validate_panzerfaust_inspection_args(scenario.label(), unit, unit_count)?;
        match scenario {
            PanzerfaustInspectionScenario::Duel => panzerfaust_duel_scenario(seed),
            PanzerfaustInspectionScenario::WindupCancel => panzerfaust_windup_cancel_scenario(seed),
            PanzerfaustInspectionScenario::TargetDeath => panzerfaust_target_death_scenario(seed),
            PanzerfaustInspectionScenario::EntrenchedRange => {
                panzerfaust_entrenched_range_scenario(seed)
            }
            PanzerfaustInspectionScenario::Methamphetamines => {
                panzerfaust_methamphetamines_scenario(seed)
            }
        }
    }
}

fn validate_panzerfaust_inspection_args(
    scenario: &str,
    unit: EntityKind,
    unit_count: usize,
) -> Result<(), String> {
    if unit != EntityKind::Panzerfaust {
        return Err(format!("unsupported {scenario} unit {unit}"));
    }
    if unit_count != 1 {
        return Err(format!("unsupported {scenario} unit count {unit_count}"));
    }
    Ok(())
}

fn set_attack_order(entities: &mut EntityStore, attacker: u32, target: u32) -> Result<(), String> {
    entities
        .get_mut(attacker)
        .ok_or_else(|| format!("missing attacker {attacker}"))?
        .set_order(Order::attack(target));
    Ok(())
}

fn panzerfaust_duel_scenario(seed: u32) -> Result<DevScenarioSetup, String> {
    let mut map = flat_dev_map(2);
    let center = (map.size / 2, map.size / 2);
    set_panzerfaust_duel_starts(&mut map, center);

    let panzerfaust_pos = map.tile_center(center.0 - 2, center.1);
    let tank_pos = map.tile_center(center.0 + 1, center.1);
    let mut entities = EntityStore::new();
    let panzerfaust = spawn_panzerfaust(&mut entities, 1, panzerfaust_pos, "")?;
    let tank = spawn_tank(&mut entities, 2, tank_pos, "target ")?;
    set_attack_order(&mut entities, panzerfaust, tank)?;

    build_panzerfaust_setup(PanzerfaustSetupSpec {
        map,
        entities,
        teams: [(1, 1), (2, 2)],
        player_id: 1,
        start_tile: (center.0 - 8, center.1),
        seed,
        metadata_name: "dev:panzerfaust_duel",
        units: vec![panzerfaust],
        goal: panzerfaust_pos,
        issue_after_ticks: u32::MAX,
    })
    .checkpoint_backed("dev:panzerfaust_duel")
}

fn panzerfaust_windup_cancel_scenario(seed: u32) -> Result<DevScenarioSetup, String> {
    let mut map = flat_dev_map(2);
    let center = (map.size / 2, map.size / 2);
    set_panzerfaust_duel_starts(&mut map, center);

    let panzerfaust_pos = map.tile_center(center.0 - 2, center.1);
    let tank_pos = map.tile_center(center.0 + 1, center.1);
    let cancel_goal = map.tile_center(center.0 - 8, center.1 + 1);
    let mut entities = EntityStore::new();
    let panzerfaust = spawn_panzerfaust(&mut entities, 1, panzerfaust_pos, "")?;
    let tank = spawn_tank(&mut entities, 2, tank_pos, "target ")?;
    set_attack_order(&mut entities, panzerfaust, tank)?;

    build_panzerfaust_setup(PanzerfaustSetupSpec {
        map,
        entities,
        teams: [(1, 1), (2, 2)],
        player_id: 1,
        start_tile: (center.0 - 8, center.1),
        seed,
        metadata_name: "dev:panzerfaust_windup_cancel",
        units: vec![panzerfaust],
        goal: cancel_goal,
        issue_after_ticks: config::TICK_HZ / 6,
    })
    .checkpoint_backed("dev:panzerfaust_windup_cancel")
}

fn panzerfaust_target_death_scenario(seed: u32) -> Result<DevScenarioSetup, String> {
    let mut map = flat_dev_map(3);
    let center = (map.size / 2, map.size / 2);
    set_player_start(&mut map, 0, (center.0 - 8, center.1 - 1));
    set_player_start(&mut map, 1, (center.0 + 8, center.1));
    set_player_start(&mut map, 2, (center.0 - 8, center.1 + 1));

    let normal_pos = map.tile_center(center.0 - 2, center.1 - 1);
    let boosted_pos = map.tile_center(center.0 - 2, center.1 + 1);
    let tank_pos = map.tile_center(center.0 + 1, center.1);
    let mut entities = EntityStore::new();
    let normal_panzerfaust = spawn_panzerfaust(&mut entities, 1, normal_pos, "normal ")?;
    let boosted_panzerfaust = spawn_panzerfaust(&mut entities, 3, boosted_pos, "boosted ")?;
    let tank = spawn_tank(&mut entities, 2, tank_pos, "low-health target ")?;
    if let Some(tank_entity) = entities.get_mut(tank) {
        let panzerfaust_tank_damage = crate::rules::combat::panzerfaust_loaded_shot_damage(
            EntityKind::Tank,
            Some(crate::rules::terrain::TerrainKind::Open),
        );
        let damage = tank_entity.hp.saturating_sub(panzerfaust_tank_damage);
        tank_entity.apply_damage(damage, None);
    }
    set_attack_order(&mut entities, normal_panzerfaust, tank)?;
    set_attack_order(&mut entities, boosted_panzerfaust, tank)?;

    let mut setup = build_panzerfaust_setup(PanzerfaustSetupSpec {
        map,
        entities,
        teams: [(1, 1), (2, 2), (3, 1)],
        player_id: 1,
        start_tile: (center.0 - 8, center.1 - 1),
        seed,
        metadata_name: "dev:panzerfaust_target_death",
        units: vec![normal_panzerfaust, boosted_panzerfaust],
        goal: normal_pos,
        issue_after_ticks: u32::MAX,
    });
    grant_methamphetamines(&mut setup.game, 3);
    setup.checkpoint_backed("dev:panzerfaust_target_death")
}

fn panzerfaust_entrenched_range_scenario(seed: u32) -> Result<DevScenarioSetup, String> {
    let mut map = flat_dev_map(2);
    let center = (map.size / 2, map.size / 2);
    set_panzerfaust_duel_starts(&mut map, center);

    let entrenched_pos = map.tile_center(center.0 - 2, center.1 - 3);
    let exposed_pos = map.tile_center(center.0 - 2, center.1 + 3);
    let entrenched_tank_pos = map.tile_center(center.0 + 2, center.1 - 3);
    let exposed_tank_pos = map.tile_center(center.0 + 2, center.1 + 3);
    let mut entities = EntityStore::new();
    let entrenched_panzerfaust =
        spawn_panzerfaust(&mut entities, 1, entrenched_pos, "entrenched ")?;
    let exposed_panzerfaust = spawn_panzerfaust(&mut entities, 1, exposed_pos, "exposed ")?;
    spawn_tank(
        &mut entities,
        2,
        entrenched_tank_pos,
        "entrenched-range target ",
    )?;
    spawn_tank(&mut entities, 2, exposed_tank_pos, "exposed-range target ")?;
    for id in [entrenched_panzerfaust, exposed_panzerfaust] {
        entities
            .get_mut(id)
            .ok_or_else(|| format!("missing Panzerfaust {id}"))?
            .hold_position();
    }

    let mut setup = build_panzerfaust_setup(PanzerfaustSetupSpec {
        map,
        entities,
        teams: [(1, 1), (2, 2)],
        player_id: 1,
        start_tile: (center.0 - 8, center.1),
        seed,
        metadata_name: "dev:panzerfaust_entrenched_range",
        units: vec![entrenched_panzerfaust, exposed_panzerfaust],
        goal: entrenched_pos,
        issue_after_ticks: u32::MAX,
    });
    grant_entrenchment(&mut setup.game, setup.player_id);
    let trench = setup
        .game
        .state
        .trenches
        .create(&setup.game.state.map, entrenched_pos.0, entrenched_pos.1)
        .ok_or_else(|| "failed to seed Panzerfaust trench".to_string())?;
    if let Some(entity) = setup.game.state.entities.get_mut(entrenched_panzerfaust) {
        if let Some(movement) = entity.movement.as_mut() {
            movement.occupied_trench_id = Some(trench);
        }
    }
    refresh_trench_memory(&mut setup.game);
    setup.checkpoint_backed("dev:panzerfaust_entrenched_range")
}

fn panzerfaust_methamphetamines_scenario(seed: u32) -> Result<DevScenarioSetup, String> {
    let mut map = flat_dev_map(3);
    let center = (map.size / 2, map.size / 2);
    set_player_start(&mut map, 0, (center.0 - 8, center.1 - 2));
    set_player_start(&mut map, 1, (center.0 + 8, center.1));
    set_player_start(&mut map, 2, (center.0 - 8, center.1 + 2));

    let normal_pos = map.tile_center(center.0 - 2, center.1 - 2);
    let boosted_pos = map.tile_center(center.0 - 2, center.1 + 2);
    let normal_tank_pos = map.tile_center(center.0 + 1, center.1 - 2);
    let boosted_tank_pos = map.tile_center(center.0 + 1, center.1 + 2);
    let mut entities = EntityStore::new();
    let normal_panzerfaust = spawn_panzerfaust(&mut entities, 1, normal_pos, "normal ")?;
    let boosted_panzerfaust = spawn_panzerfaust(&mut entities, 3, boosted_pos, "boosted ")?;
    let normal_tank = spawn_tank(&mut entities, 2, normal_tank_pos, "normal target ")?;
    let boosted_tank = spawn_tank(&mut entities, 2, boosted_tank_pos, "boosted target ")?;
    set_attack_order(&mut entities, normal_panzerfaust, normal_tank)?;
    set_attack_order(&mut entities, boosted_panzerfaust, boosted_tank)?;

    let mut setup = build_panzerfaust_setup(PanzerfaustSetupSpec {
        map,
        entities,
        teams: [(1, 1), (2, 2), (3, 1)],
        player_id: 1,
        start_tile: (center.0 - 8, center.1 - 2),
        seed,
        metadata_name: "dev:panzerfaust_methamphetamines",
        units: vec![normal_panzerfaust, boosted_panzerfaust],
        goal: normal_pos,
        issue_after_ticks: u32::MAX,
    });
    grant_methamphetamines(&mut setup.game, 3);
    setup.checkpoint_backed("dev:panzerfaust_methamphetamines")
}

struct PanzerfaustSetupSpec<const N: usize> {
    map: Map,
    entities: EntityStore,
    teams: [(u32, u32); N],
    player_id: u32,
    start_tile: (u32, u32),
    seed: u32,
    metadata_name: &'static str,
    units: Vec<u32>,
    goal: (f32, f32),
    issue_after_ticks: u32,
}

fn build_panzerfaust_setup<const N: usize>(spec: PanzerfaustSetupSpec<N>) -> DevScenarioSetup {
    DevScenarioSetup {
        game: build_dev_scenario_game_with_teams(
            spec.map,
            spec.entities,
            spec.teams,
            spec.player_id,
            spec.start_tile,
            spec.seed,
            spec.metadata_name,
        ),
        player_id: spec.player_id,
        units: spec.units,
        goal: spec.goal,
        issue_after_ticks: spec.issue_after_ticks,
    }
}

fn set_panzerfaust_duel_starts(map: &mut Map, center: (u32, u32)) {
    set_player_start(map, 0, (center.0 - 8, center.1));
    set_player_start(map, 1, (center.0 + 8, center.1));
}

fn set_player_start(map: &mut Map, index: usize, tile: (u32, u32)) {
    if let Some(slot) = map.starts.get_mut(index) {
        *slot = tile;
    }
}

fn spawn_panzerfaust(
    entities: &mut EntityStore,
    owner: u32,
    pos: (f32, f32),
    label: &str,
) -> Result<u32, String> {
    entities
        .spawn_unit(owner, EntityKind::Panzerfaust, pos.0, pos.1)
        .ok_or_else(|| format!("failed to spawn {label}Panzerfaust"))
}

fn spawn_tank(
    entities: &mut EntityStore,
    owner: u32,
    pos: (f32, f32),
    label: &str,
) -> Result<u32, String> {
    entities
        .spawn_unit(owner, EntityKind::Tank, pos.0, pos.1)
        .ok_or_else(|| format!("failed to spawn {label}Tank"))
}

fn grant_entrenchment(game: &mut Game, player_id: u32) {
    if let Some(player) = game
        .state
        .players
        .iter_mut()
        .find(|player| player.id == player_id)
    {
        player.upgrades.insert(upgrade::UpgradeKind::Entrenchment);
    }
}

fn grant_methamphetamines(game: &mut Game, player_id: u32) {
    if let Some(player) = game
        .state
        .players
        .iter_mut()
        .find(|player| player.id == player_id)
    {
        player
            .upgrades
            .insert(upgrade::UpgradeKind::Methamphetamines);
    }
}

fn refresh_trench_memory(game: &mut Game) {
    let player_ids: Vec<u32> = game.state.players.iter().map(|player| player.id).collect();
    game.refresh_trench_memory(&player_ids);
}
