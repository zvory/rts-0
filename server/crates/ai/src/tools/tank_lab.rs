//! `ai-tank-lab`: a headless Tank micro testbed.
//!
//! AI 2.1 rarely parks its Tanks, so the 120 game test barely exercises fights against a parked
//! Tank line, the way human players fight. Each lab fight starts a real map in Lab mode, keeps
//! both Resource Depots, gives the candidate a finished Tank tech path (Barracks, Training Centre,
//! Engineering Complex, Factory, Tank Production) with N Tanks and a Scout Car, and gives the
//! opponent K Tanks run by a scripted controller:
//! - `parked_line`: the opponent's Tanks park 20 tiles in front of its base, with a Rifleman
//!   spotting ahead of them. The candidate has to get through them.
//! - `siege`: the opponent's Tanks park 13 tiles in front of the candidate's base and shell it,
//!   with a Rifleman spotting.
//! - `attackers`: the opponent's Tanks attack-move from mid-map into the candidate's base, the
//!   way AI 2.1 attacks. A regression check: parked defenders should keep winning this.
//! - `trap_wall`: a human-style defense. A Tank Trap wall stands 26 tiles in front of the
//!   opponent's base, its Tanks park 6 tiles behind it, and a Rifleman spots in front of it. The
//!   candidate has to take the wall down and get through.
//!
//! Usage: ai-tank-lab [--candidate jeffs_ai] [--baseline jeffs_ai_pre_tank_micro]
//!        [--map "1v1 No Terrain"] [--ticks 4500] [--scenarios parked_line,siege,attackers]
//!        [--sizes 3v2,4v3,5v3,6v4,3v3] [--replays <dir>] [--trace] [--smoke-plus]

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::process;

use rts_sim::game::entity::EntityKind;
use rts_sim::game::lab::{LabOp, LabSetCompletedResearch, LabSetPlayerResources, LabSpawnEntity};
use rts_sim::game::map::Map;
use rts_sim::game::upgrade::UpgradeKind;
use rts_sim::game::{Game, PlayerInit};
use rts_sim::protocol::Event;

use crate::live::{AiAlivePolicy, AiController, CanonicalAiTickDriver};
use crate::sdk::{AiActions, AiFrame, AiStrategy, UnitGroup};
use crate::selfplay::{canonical_profile_id, server_build_sha};
use rts_sim::game::replay::ReplayStartComposition;

const TILE_PX: f32 = rts_rules::balance::TILE_SIZE as f32;
const MAP_SEED: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Scenario {
    ParkedLine,
    Siege,
    Attackers,
    TrapWall,
}

impl Scenario {
    fn parse(name: &str) -> Result<Self, String> {
        match name {
            "parked_line" => Ok(Self::ParkedLine),
            "siege" => Ok(Self::Siege),
            "attackers" => Ok(Self::Attackers),
            "trap_wall" => Ok(Self::TrapWall),
            other => Err(format!("unknown scenario {other:?}")),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::ParkedLine => "parked_line",
            Self::Siege => "siege",
            Self::Attackers => "attackers",
            Self::TrapWall => "trap_wall",
        }
    }
}

/// The opponent: faces the candidate, then either holds or attack-moves at it.
struct ScriptedTanks {
    face: (f32, f32),
    attack: Option<(f32, f32)>,
    faced: bool,
    attacked: bool,
}

impl AiStrategy for ScriptedTanks {
    fn step(&mut self, frame: &AiFrame, actions: &mut AiActions) {
        let tanks: Vec<u32> = frame
            .owned()
            .iter()
            .filter(|unit| unit.kind == EntityKind::Tank)
            .map(|unit| unit.id)
            .collect();
        let Ok(group) = UnitGroup::new(tanks.iter().copied()) else {
            return;
        };
        if !self.faced && frame.tick() >= 2 {
            // A short step toward the candidate turns the hulls to face it.
            for unit in frame.owned().iter().filter(|unit| tanks.contains(&unit.id)) {
                let (x, y) = unit.position;
                let dx = self.face.0 - x;
                let dy = self.face.1 - y;
                let length = (dx * dx + dy * dy).sqrt().max(1.0);
                if let Ok(one) = UnitGroup::new([unit.id]) {
                    let _ = actions.move_group(
                        &one,
                        x + dx / length * 0.6 * TILE_PX,
                        y + dy / length * 0.6 * TILE_PX,
                        false,
                    );
                }
            }
            self.faced = true;
            return;
        }
        if let Some((x, y)) = self.attack {
            if !self.attacked && frame.tick() >= 90 {
                let _ = actions.attack_move(&group, x, y, false);
                self.attacked = true;
            }
        }
    }
}

#[derive(Clone, Debug, Default)]
struct Outcome {
    candidate_tanks: usize,
    candidate_alive: usize,
    opponent_tanks: usize,
    opponent_alive: usize,
    smoke_casts: usize,
    ticks: u32,
    candidate_hp_lost: u32,
}

struct Fight<'a> {
    scenario: Scenario,
    profile: &'static str,
    candidate_player: u32,
    tanks: usize,
    enemy_tanks: usize,
    map_name: &'a str,
    max_ticks: u32,
    replay_dir: Option<&'a PathBuf>,
    trace: bool,
    smoke_plus: bool,
}

fn spawn(game: &mut Game, owner: u32, kind: EntityKind, point: (f32, f32)) -> Result<(), String> {
    game.apply_lab_op(LabOp::SpawnEntity(LabSpawnEntity {
        owner,
        kind,
        x: point.0,
        y: point.1,
        completed: true,
    }))
    .map(|_| ())
    .map_err(|err| format!("spawn {kind:?} at {point:?}: {err:?}"))
}

/// Spawns `kind` at the first of `points` that accepts it.
fn spawn_first(
    game: &mut Game,
    owner: u32,
    kind: EntityKind,
    points: &[(f32, f32)],
) -> Result<(), String> {
    let mut last = String::new();
    for point in points {
        match spawn(game, owner, kind, *point) {
            Ok(()) => return Ok(()),
            Err(err) => last = err,
        }
    }
    Err(last)
}

fn run_fight(fight: &Fight<'_>) -> Result<Outcome, String> {
    let candidate = fight.candidate_player;
    let opponent = 3 - candidate;
    let player = |id: u32, name: &str| PlayerInit {
        id,
        team_id: id,
        faction_id: "kriegsia".to_string(),
        name: name.to_string(),
        color: if id == 1 { "#4cc9f0" } else { "#f72585" }.to_string(),
        is_ai: true,
    };
    let players = vec![player(1, "p1"), player(2, "p2")];
    let map = Map::load_for_players(fight.map_name, &[(1, 1), (2, 2)], MAP_SEED)?;
    let metadata = Map::metadata_for_name(fight.map_name)?;
    let mut game = Game::new_lab(&players, 7, map, metadata);
    let depots: BTreeMap<u32, (f32, f32)> = game
        .snapshot_full_for(1)
        .entities
        .iter()
        .filter(|entity| entity.kind == "resource_depot")
        .map(|entity| (entity.owner, (entity.x, entity.y)))
        .collect();
    let home = *depots.get(&candidate).ok_or("candidate has no depot")?;
    let enemy = *depots.get(&opponent).ok_or("opponent has no depot")?;
    for id in [1, 2] {
        let units: Vec<u32> = game
            .lab_owned_units(id)
            .map_err(|err| format!("{err:?}"))?
            .into_iter()
            .map(|(unit, _)| unit)
            .collect();
        game.apply_lab_op(LabOp::DeleteEntities(units))
            .map_err(|err| format!("{err:?}"))?;
        game.apply_lab_op(LabOp::SetPlayerResources(LabSetPlayerResources {
            player_id: id,
            steel: 0,
            oil: 0,
        }))
        .map_err(|err| format!("{err:?}"))?;
    }
    let (dx, dy) = (enemy.0 - home.0, enemy.1 - home.1);
    let length = (dx * dx + dy * dy).sqrt().max(1.0);
    let u = (dx / length, dy / length);
    let perp = (-u.1, u.0);
    let at = |origin: (f32, f32), along: f32, side: f32| {
        (
            origin.0 + (u.0 * along + perp.0 * side) * TILE_PX,
            origin.1 + (u.1 * along + perp.1 * side) * TILE_PX,
        )
    };

    // The candidate's finished Tank tech, behind its depot.
    for (index, kind) in [
        EntityKind::Barracks,
        EntityKind::TrainingCentre,
        EntityKind::EngineeringComplex,
        EntityKind::Factory,
    ]
    .into_iter()
    .enumerate()
    {
        let side = (index as f32 - 1.5) * 5.0;
        let points: Vec<(f32, f32)> = [-6.0_f32, -9.0, -12.0]
            .into_iter()
            .flat_map(|along| [side, side + 2.5, side - 2.5].map(|s| at(home, along, s)))
            .collect();
        spawn_first(&mut game, candidate, kind, &points)?;
    }
    let mut upgrades = vec![UpgradeKind::TankUnlock, UpgradeKind::Entrenchment];
    if fight.smoke_plus {
        upgrades.push(UpgradeKind::SmokePlus);
    }
    for upgrade in upgrades {
        game.apply_lab_op(LabOp::SetCompletedResearch(LabSetCompletedResearch {
            player_id: candidate,
            upgrade,
            completed: true,
        }))
        .map_err(|err| format!("{err:?}"))?;
    }
    // Under siege the defenders start behind the depot, out of the attackers' reach.
    let tank_row = if fight.scenario == Scenario::Siege {
        -4.0
    } else {
        5.0
    };
    for index in 0..fight.tanks {
        let side = (index as f32 - (fight.tanks as f32 - 1.0) / 2.0) * 2.0;
        spawn_first(
            &mut game,
            candidate,
            EntityKind::Tank,
            &[
                at(home, tank_row, side),
                at(home, tank_row - 2.0, side),
                at(home, tank_row + 2.0, side),
            ],
        )?;
    }
    spawn_first(
        &mut game,
        candidate,
        EntityKind::ScoutCar,
        &[at(home, 3.0, 0.0), at(home, 2.0, 3.0), at(home, 2.0, -3.0)],
    )?;

    // The opponent's Tanks and spotter.
    let (line, spotter, attack) = match fight.scenario {
        Scenario::ParkedLine => (at(enemy, -20.0, 0.0), Some(at(enemy, -24.0, 0.0)), None),
        Scenario::Siege => (at(home, 12.0, 0.0), Some(at(home, 7.5, 2.0)), None),
        Scenario::Attackers => (at(home, length / TILE_PX * 0.5, 0.0), None, Some(home)),
        Scenario::TrapWall => (at(enemy, -20.0, 0.0), Some(at(enemy, -28.0, 0.0)), None),
    };
    if fight.scenario == Scenario::TrapWall {
        // Two tiles apart, so the wall closes for vehicles, from map edge to map edge (spots off
        // the map fail); completed traps turn neutral.
        for step in -60..=60 {
            let point = at(enemy, -26.0, step as f32 * 2.0);
            let _ = spawn(&mut game, opponent, EntityKind::TankTrap, point);
        }
    }
    for index in 0..fight.enemy_tanks {
        let side = (index as f32 - (fight.enemy_tanks as f32 - 1.0) / 2.0) * 2.0;
        let point = (
            line.0 + perp.0 * side * TILE_PX,
            line.1 + perp.1 * side * TILE_PX,
        );
        spawn_first(&mut game, opponent, EntityKind::Tank, &[point])?;
    }
    if let Some(spotter) = spotter {
        spawn_first(&mut game, opponent, EntityKind::Rifleman, &[spotter])?;
    }

    let tank_ids = |game: &Game, owner: u32| -> BTreeSet<u32> {
        game.lab_owned_units(owner)
            .unwrap_or_default()
            .into_iter()
            .filter(|(_, kind)| *kind == EntityKind::Tank)
            .map(|(id, _)| id)
            .collect()
    };
    let start_candidate = tank_ids(&game, candidate);
    let start_opponent = tank_ids(&game, opponent);
    // HP the starting Tanks lost, a dead one counting its full 292.
    let starting_hp_lost = |game: &Game| -> u32 {
        let snapshot = game.snapshot_full_for(candidate);
        start_candidate
            .iter()
            .map(|id| {
                snapshot
                    .entities
                    .iter()
                    .find(|e| e.id == *id)
                    .map_or(292, |e| e.max_hp.saturating_sub(e.hp))
            })
            .sum()
    };
    let replay_start = fight
        .replay_dir
        .map(|_| ReplayStartComposition::capture(&game, server_build_sha()))
        .transpose()?;

    let scripted = ScriptedTanks {
        face: home,
        attack,
        faced: false,
        attacked: false,
    };
    let mut controllers = if candidate == 1 {
        vec![
            AiController::with_profile_id(1, fight.profile),
            AiController::with_strategy(2, Box::new(scripted)),
        ]
    } else {
        vec![
            AiController::with_strategy(1, Box::new(scripted)),
            AiController::with_profile_id(2, fight.profile),
        ]
    };
    let mut smoke_casts = 0;
    while game.tick_count() < fight.max_ticks {
        CanonicalAiTickDriver::run(&mut game, &mut controllers, AiAlivePolicy::Normal);
        if fight.trace {
            let index = usize::from(candidate == 2);
            if let Some(trace) = controllers[index]
                .latest_decision_trace()
                .filter(|trace| trace.trace_tick == game.tick_count())
            {
                for line in trace.lines.iter().filter(|line| {
                    line.starts_with("tank_micro") && !line.starts_with("tank_micro memory")
                }) {
                    println!("    t{} {}", game.tick_count(), line);
                }
            }
        }
        if fight.trace && game.tick_count().is_multiple_of(300) {
            let index = usize::from(candidate == 2);
            if let Some(trace) = controllers[index].latest_decision_trace() {
                let tanks = tank_ids(&game, candidate).len();
                let snapshot = game.snapshot_full_for(candidate);
                let spots: Vec<String> = snapshot
                    .entities
                    .iter()
                    .filter(|e| e.kind == "tank" || e.kind == "scout_car")
                    .map(|e| {
                        format!(
                            "{}{}@{:.0},{:.0}:{}",
                            if e.owner == candidate { "J" } else { "E" },
                            &e.kind[..1],
                            e.x / TILE_PX,
                            e.y / TILE_PX,
                            e.state
                        )
                    })
                    .collect();
                println!("    t{} units {}", game.tick_count(), spots.join(" "));
                for line in trace.lines.iter().filter(|line| {
                    line.contains("FrontalAttack") || line.starts_with("tank_micro memory")
                }) {
                    println!("    t{} tanks={} {}", game.tick_count(), tanks, line);
                }
            }
        }
        for (recipient, events) in game.tick() {
            if recipient != candidate {
                continue;
            }
            for event in events {
                if matches!(event, Event::SmokeLaunch { .. }) {
                    smoke_casts += 1;
                }
            }
        }
        if tank_ids(&game, candidate).is_empty() || tank_ids(&game, opponent).is_empty() {
            break;
        }
    }
    let alive_candidate: BTreeSet<u32> = tank_ids(&game, candidate)
        .intersection(&start_candidate)
        .copied()
        .collect();
    let alive_opponent: BTreeSet<u32> = tank_ids(&game, opponent)
        .intersection(&start_opponent)
        .copied()
        .collect();
    if let (Some(dir), Some(start)) = (fight.replay_dir, replay_start) {
        let name = format!(
            "tank-lab-{}-{}-{}v{}-p{}",
            fight.scenario.name(),
            fight.profile,
            fight.tanks,
            fight.enemy_tanks,
            candidate
        );
        let artifact = start.finalize(&game, None, game.scores());
        let json = serde_json::to_vec(&artifact).map_err(|err| err.to_string())?;
        std::fs::write(dir.join(format!("{name}.json")), json).map_err(|err| err.to_string())?;
    }
    Ok(Outcome {
        candidate_tanks: start_candidate.len(),
        candidate_alive: alive_candidate.len(),
        opponent_tanks: start_opponent.len(),
        opponent_alive: alive_opponent.len(),
        smoke_casts,
        ticks: game.tick_count(),
        candidate_hp_lost: starting_hp_lost(&game),
    })
}

/// Fights, Tanks lost, enemy Tanks killed, wins, wiped.
type Totals = (usize, usize, usize, usize, usize);

struct Config {
    candidate: &'static str,
    baseline: &'static str,
    map: String,
    ticks: u32,
    scenarios: Vec<Scenario>,
    sizes: Vec<(usize, usize)>,
    replays: Option<PathBuf>,
    trace: bool,
    smoke_plus: bool,
}

fn parse_args() -> Result<Config, String> {
    let mut config = Config {
        candidate: "jeffs_ai",
        baseline: "jeffs_ai_pre_tank_micro",
        map: "1v1 No Terrain".to_string(),
        ticks: 4500,
        scenarios: vec![Scenario::ParkedLine, Scenario::Siege, Scenario::Attackers],
        sizes: vec![(3, 2), (4, 3), (5, 3), (6, 4), (3, 3)],
        replays: None,
        trace: false,
        smoke_plus: false,
    };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = || args.next().ok_or_else(|| format!("{arg} needs a value"));
        match arg.as_str() {
            "--candidate" => {
                let id = value()?;
                config.candidate =
                    canonical_profile_id(&id).ok_or(format!("unknown profile {id}"))?;
            }
            "--baseline" => {
                let id = value()?;
                config.baseline =
                    canonical_profile_id(&id).ok_or(format!("unknown profile {id}"))?;
            }
            "--map" => config.map = value()?,
            "--ticks" => {
                config.ticks = value()?.parse().map_err(|err| format!("--ticks: {err}"))?
            }
            "--scenarios" => {
                config.scenarios = value()?
                    .split(',')
                    .map(|name| Scenario::parse(name.trim()))
                    .collect::<Result<_, _>>()?
            }
            "--sizes" => {
                config.sizes = value()?
                    .split(',')
                    .map(|size| {
                        let (n, k) = size
                            .trim()
                            .split_once('v')
                            .ok_or(format!("bad size {size}"))?;
                        Ok((
                            n.parse().map_err(|_| format!("bad size {size}"))?,
                            k.parse().map_err(|_| format!("bad size {size}"))?,
                        ))
                    })
                    .collect::<Result<_, String>>()?
            }
            "--replays" => config.replays = Some(PathBuf::from(value()?)),
            "--trace" => config.trace = true,
            "--smoke-plus" => config.smoke_plus = true,
            other => return Err(format!("unknown argument {other}")),
        }
    }
    Ok(config)
}

pub fn run_from_env() {
    let config = match parse_args() {
        Ok(config) => config,
        Err(err) => {
            eprintln!("ai-tank-lab: {err}");
            process::exit(2);
        }
    };
    if let Some(dir) = &config.replays {
        let _ = std::fs::create_dir_all(dir);
    }
    println!(
        "scenario      size  profile                    side  tanks left  enemy left  smoke  ticks  hp lost"
    );
    let mut totals: BTreeMap<(Scenario, &str), Totals> = BTreeMap::new();
    for scenario in &config.scenarios {
        for (tanks, enemy_tanks) in &config.sizes {
            for profile in [config.baseline, config.candidate] {
                for side in [1, 2] {
                    let fight = Fight {
                        scenario: *scenario,
                        profile,
                        candidate_player: side,
                        tanks: *tanks,
                        enemy_tanks: *enemy_tanks,
                        map_name: &config.map,
                        max_ticks: config.ticks,
                        replay_dir: config.replays.as_ref(),
                        trace: config.trace,
                        smoke_plus: config.smoke_plus,
                    };
                    match run_fight(&fight) {
                        Ok(outcome) => {
                            println!(
                                "{:<13} {}v{}  {:<26} p{}    {:>2}/{:<2}       {:>2}/{:<2}       {:>3}  {:>5}  {:>6}",
                                scenario.name(),
                                tanks,
                                enemy_tanks,
                                profile,
                                side,
                                outcome.candidate_alive,
                                outcome.candidate_tanks,
                                outcome.opponent_alive,
                                outcome.opponent_tanks,
                                outcome.smoke_casts,
                                outcome.ticks,
                                outcome.candidate_hp_lost
                            );
                            let entry = totals.entry((*scenario, profile)).or_default();
                            entry.0 += 1;
                            entry.1 += outcome.candidate_tanks - outcome.candidate_alive;
                            entry.2 += outcome.opponent_tanks - outcome.opponent_alive;
                            entry.3 += usize::from(
                                outcome.opponent_alive == 0 && outcome.candidate_alive > 0,
                            );
                            entry.4 += usize::from(outcome.candidate_alive == 0);
                        }
                        Err(err) => println!(
                            "{} {}v{} {} p{}: error {err}",
                            scenario.name(),
                            tanks,
                            enemy_tanks,
                            profile,
                            side
                        ),
                    }
                }
            }
        }
    }
    println!("\nscenario      profile                    fights  tanks lost  enemy tanks killed  wins  wiped");
    for ((scenario, profile), (fights, lost, killed, wins, wiped)) in totals {
        println!(
            "{:<13} {:<26} {:>6}  {:>10}  {:>18}  {:>4}  {:>5}",
            scenario.name(),
            profile,
            fights,
            lost,
            killed,
            wins,
            wiped
        );
    }
}
