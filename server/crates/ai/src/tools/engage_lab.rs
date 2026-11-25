//! `ai-engage-lab`: scores the matchup micro (`jeff_v1_1::matchup`) one engagement at a time.
//!
//! Each fight is a Lab game on an open map: both Resource Depots stay, every other unit goes, and
//! two groups spawn a gap apart between the depots. The opponent is scripted: `attack` attack-moves
//! at the candidate the way the AI opponents' waves do (re-aimed at what it sees), `hold` stands
//! where it spawned. The candidate attack-moves the same way, and with `mm:<params>` the matchup
//! micro steers its units in contact. Every scenario is fought from both player slots over
//! several placement seeds; the score is value lost (steel plus oil, wounded units counted by the
//! HP share lost) on each side.
//!
//! Usage: ai-engage-lab --scenarios FILE|"name ours theirs opp[;...]" --cands "naive;mm;mm:avoid=0"
//!        [--seeds 6] [--map "1v1 No Terrain"] [--ticks 2700] [--gap 20] [--threads 8]
//!        [--meth-us 1] [--meth-them 1] [--fights FILE]
//! A side is `kind:count+kind:count`, e.g. `tank:3+rifleman:10`; opp is `attack` or `hold`.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write as _;
use std::process;

use rayon::prelude::*;
use rts_rules::math::GameplayMath as _;
use rts_sim::game::entity::EntityKind;
use rts_sim::game::lab::{LabOp, LabSetCompletedResearch, LabSetPlayerResources, LabSpawnEntity};
use rts_sim::game::map::Map;
use rts_sim::game::upgrade::UpgradeKind;
use rts_sim::game::{Game, PlayerInit};

use crate::ai_core::decision::matchup::{self, MatchupMemory, Order, Params};
use crate::ai_core::observation::AiObservation;
use crate::live::{AiAlivePolicy, AiController, CanonicalAiTickDriver};
use crate::sdk::{AiActions, AiFrame, AiStrategy, UnitGroup};

const TILE_PX: f32 = rts_rules::balance::TILE_SIZE as f32;
const MAP_SEED: u32 = 1;

#[derive(Clone, Debug)]
struct Scenario {
    name: String,
    ours: Vec<(EntityKind, usize)>,
    theirs: Vec<(EntityKind, usize)>,
    hold: bool,
    gap: Option<f32>,
    /// A real-map replay (`map=Name_With_Underscores abs`): units spawn at their real tiles on that map.
    map: Option<String>,
    abs: bool,
    /// The player slot the candidate had in the real game (`slot=N`): both runs use it, with two seeds.
    slot: Option<u32>,
    /// The candidate holds too (defends) instead of attacking.
    us_hold: bool,
    /// Snapshot scenarios (a side written as `kind@along,across/...`): every unit's place in tiles,
    /// origin at the middle of the field, +along from the candidate toward the opponent.
    ours_at: Vec<(EntityKind, f32, f32)>,
    theirs_at: Vec<(EntityKind, f32, f32)>,
}

#[derive(Clone, Debug)]
struct Cand {
    label: String,
    params: Option<Params>,
}

#[derive(Clone, Debug, Default)]
struct FightResult {
    our_lost: f32,
    their_lost: f32,
    our_dead: usize,
    their_dead: usize,
    our_alive: usize,
    their_alive: usize,
    ticks: u32,
    orders: String,
}

fn parse_side(text: &str) -> Result<Vec<(EntityKind, usize)>, String> {
    text.split('+')
        .map(|part| {
            let (kind, count) = part.split_once(':').unwrap_or((part, "1"));
            let kind: EntityKind = kind
                .parse()
                .map_err(|_| format!("unknown unit kind {kind:?}"))?;
            let count: usize = count.parse().map_err(|_| format!("bad count in {part:?}"))?;
            Ok((kind, count))
        })
        .collect()
}

fn parse_at(text: &str) -> Result<Vec<(EntityKind, f32, f32)>, String> {
    text.split('/')
        .map(|part| {
            let (kind, at) = part.split_once('@').ok_or_else(|| format!("bad unit {part:?}"))?;
            let (along, across) = at.split_once(',').ok_or_else(|| format!("bad place {part:?}"))?;
            let kind: EntityKind = kind
                .parse()
                .map_err(|_| format!("unknown unit kind {kind:?}"))?;
            let along: f32 = along.parse().map_err(|_| format!("bad place {part:?}"))?;
            let across: f32 = across.parse().map_err(|_| format!("bad place {part:?}"))?;
            Ok((kind, along, across))
        })
        .collect()
}

fn counts(units: &[(EntityKind, f32, f32)]) -> Vec<(EntityKind, usize)> {
    let mut out: Vec<(EntityKind, usize)> = Vec::new();
    for (kind, _, _) in units {
        match out.iter_mut().find(|(k, _)| k == kind) {
            Some(entry) => entry.1 += 1,
            None => out.push((*kind, 1)),
        }
    }
    out
}

fn parse_scenarios(text: &str) -> Result<Vec<Scenario>, String> {
    let body = if std::path::Path::new(text).is_file() {
        std::fs::read_to_string(text).map_err(|err| err.to_string())?
    } else {
        text.replace(';', "\n")
    };
    body.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| {
            let fields: Vec<&str> = line.split_whitespace().collect();
            if fields.len() < 4 {
                return Err(format!("scenario needs name ours theirs opp: {line:?}"));
            }
            let mut gap = None;
            let mut us_hold = false;
            let mut map = None;
            let mut abs = false;
            let mut slot = None;
            for extra in &fields[4..] {
                if let Some(value) = extra.strip_prefix("slot=") {
                    slot = value.parse().ok();
                }
                if *extra == "abs" {
                    abs = true;
                }
                if let Some(name) = extra.strip_prefix("map=") {
                    map = Some(name.replace('_', " "));
                }
                if *extra == "us=hold" {
                    us_hold = true;
                }
                if let Some(value) = extra.strip_prefix("gap=") {
                    gap = Some(value.parse().map_err(|_| format!("bad gap {extra:?}"))?);
                }
            }
            let snap = fields[1].contains('@');
            let ours_at = if snap { parse_at(fields[1])? } else { Vec::new() };
            let theirs_at = if snap { parse_at(fields[2])? } else { Vec::new() };
            Ok(Scenario {
                name: fields[0].to_string(),
                ours: if snap { counts(&ours_at) } else { parse_side(fields[1])? },
                theirs: if snap { counts(&theirs_at) } else { parse_side(fields[2])? },
                ours_at,
                theirs_at,
                hold: match fields[3] {
                    "hold" => true,
                    "attack" => false,
                    other => return Err(format!("opp must be attack or hold, got {other:?}")),
                },
                gap,
                map,
                abs,
                slot,
                us_hold,
            })
        })
        .collect()
}

fn parse_cands(text: &str) -> Result<Vec<Cand>, String> {
    text.split(';')
        .map(str::trim)
        .filter(|c| !c.is_empty())
        .map(|c| {
            let (label, spec) = c.split_once('=').map_or((c, c), |(l, s)| (l, s));
            let params = match spec {
                "naive" => None,
                "mm" => Some(Params::default()),
                other => Some(Params::parse(other.strip_prefix("mm:").unwrap_or(other))?),
            };
            Ok(Cand {
                label: label.to_string(),
                params,
            })
        })
        .collect()
}

/// One side's controller: attack-move at the enemy (or hold), with optional matchup micro.
struct LabSide {
    params: Option<Params>,
    hold: bool,
    memory: MatchupMemory,
    goal: (f32, f32),
    home: (f32, f32),
    started: bool,
    last_check: u32,
}

fn emit(actions: &mut AiActions, id: u32, order: Order) {
    let Ok(group) = UnitGroup::new([id]) else {
        return;
    };
    let _ = match order {
        Order::Move(x, y) => actions.move_group(&group, x, y, false),
        Order::Attack(target) => actions.attack(&group, target, false),
        Order::Hold => actions.hold_position(&group, false),
        Order::Point(x, y) => actions.point_tanks(&group, x, y, false),
    };
}

impl AiStrategy for LabSide {
    fn step(&mut self, frame: &AiFrame, actions: &mut AiActions) {
        let tick = frame.tick();
        if tick < 3 {
            return;
        }
        let Some(observation) = AiObservation::from_frame(frame) else {
            return;
        };
        let units: Vec<&crate::ai_core::observation::AiEntitySummary> = observation
            .owned
            .iter()
            .filter(|unit| {
                unit.hp > 0 && unit.kind.is_unit() && unit.kind != EntityKind::Worker
            })
            .collect();
        if self.hold {
            if !self.started {
                if let Ok(group) = UnitGroup::new(units.iter().map(|unit| unit.id)) {
                    let _ = actions.hold_position(&group, false);
                }
                self.started = true;
            }
            // A holding side with micro: the micro steers, released units hold again.
            if let Some(params) = &self.params {
                let plan = matchup::plan(&observation, params, &mut self.memory, &|_| true, self.home);
                for (id, order) in &plan.orders {
                    emit(actions, *id, *order);
                }
                if let Ok(group) = UnitGroup::new(plan.released.iter().copied()) {
                    let _ = actions.hold_position(&group, false);
                }
            }
            return;
        }
        let mut steered = BTreeSet::new();
        let mut released = Vec::new();
        if let Some(params) = &self.params {
            let plan = matchup::plan(&observation, params, &mut self.memory, &|_| true, self.home);
            for (id, order) in &plan.orders {
                emit(actions, *id, *order);
            }
            steered = plan.held;
            released = plan.released;
        }
        let seen: Vec<(f32, f32)> = observation
            .visible_enemies
            .iter()
            .filter(|enemy| enemy.kind.is_unit() && enemy.kind != EntityKind::Worker)
            .map(|enemy| (enemy.x, enemy.y))
            .collect();
        let goal = if seen.is_empty() {
            self.goal
        } else {
            let n = seen.len() as f32;
            (
                seen.iter().map(|p| p.0).sum::<f32>() / n,
                seen.iter().map(|p| p.1).sum::<f32>() / n,
            )
        };
        let send: Vec<u32> = if !self.started {
            self.started = true;
            self.last_check = tick;
            units.iter().map(|unit| unit.id).collect()
        } else if tick.saturating_sub(self.last_check) >= 30 {
            self.last_check = tick;
            units
                .iter()
                .filter(|unit| {
                    !steered.contains(&unit.id)
                        && (unit.state == crate::ai_core::observation::AiEntityState::Idle
                            || released.contains(&unit.id))
                })
                .map(|unit| unit.id)
                .collect()
        } else {
            released
        };
        if let Ok(group) = UnitGroup::new(send) {
            let _ = actions.attack_move(&group, goal.0, goal.1, false);
        }
    }
}

fn spawn_first(game: &mut Game, owner: u32, kind: EntityKind, points: &[(f32, f32)]) -> bool {
    points.iter().any(|point| {
        game.apply_lab_op(LabOp::SpawnEntity(LabSpawnEntity {
            owner,
            kind,
            x: point.0,
            y: point.1,
            completed: true,
            dao: None,
            faction_id: None,
        }))
        .is_ok()
    })
}

/// Small deterministic jitter from the seed.
fn jitter(seed: u32, index: u32) -> f32 {
    let mut x = seed
        .wrapping_mul(0x9E37_79B9)
        .wrapping_add(index.wrapping_mul(0x85EB_CA6B))
        .wrapping_add(0x27D4_EB2F);
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    (x % 10_000) as f32 / 10_000.0 - 0.5
}

struct Setup<'a> {
    scenario: &'a Scenario,
    cand: &'a Cand,
    candidate: u32,
    seed: u32,
    map: &'a str,
    ticks: u32,
    gap: f32,
    meth_us: bool,
    meth_them: bool,
}

fn run_fight(setup: &Setup<'_>) -> Result<FightResult, String> {
    let candidate = setup.candidate;
    let opponent = 3 - candidate;
    let player = |id: u32| PlayerInit {
        id,
        team_id: id,
        faction_id: "kriegsia".to_string(),
        name: format!("p{id}"),
        color: if id == 1 { "#4cc9f0" } else { "#f72585" }.to_string(),
        is_ai: true,
    };
    let players = vec![player(1), player(2)];
    let map_name = setup.scenario.map.as_deref().unwrap_or(setup.map);
    let map = Map::load_for_players(map_name, &[(1, 1), (2, 2)], MAP_SEED)?;
    let metadata = Map::metadata_for_name(map_name)?;
    let mut game = Game::new_lab(&players, setup.seed.wrapping_add(11), map, metadata);
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
        let meth = if id == candidate {
            setup.meth_us
        } else {
            setup.meth_them
        };
        let mut upgrades = vec![UpgradeKind::TankUnlock, UpgradeKind::Panzerfausts];
        if meth {
            upgrades.push(UpgradeKind::Methamphetamines);
        }
        for upgrade in upgrades {
            game.apply_lab_op(LabOp::SetCompletedResearch(LabSetCompletedResearch {
                player_id: id,
                upgrade,
                completed: true,
            }))
            .map_err(|err| format!("{err:?}"))?;
        }
    }
    let (dx, dy) = (enemy.0 - home.0, enemy.1 - home.1);
    let length = (dx * dx + dy * dy).det_sqrt().max(1.0);
    let u = (dx / length, dy / length);
    let perp = (-u.1, u.0);
    let mid = ((home.0 + enemy.0) / 2.0, (home.1 + enemy.1) / 2.0);
    let shift = jitter(setup.seed, 999) * 6.0;
    let mut place = |owner: u32, side: &[(EntityKind, usize)], toward: f32, salt: u32| {
        // Vehicles in the front row, infantry in rows behind them.
        let center = (
            mid.0 - u.0 * toward * setup.gap / 2.0 * TILE_PX + perp.0 * shift * TILE_PX,
            mid.1 - u.1 * toward * setup.gap / 2.0 * TILE_PX + perp.1 * shift * TILE_PX,
        );
        let at = |along: f32, across: f32| {
            (
                center.0 + (u.0 * along * toward + perp.0 * across) * TILE_PX,
                center.1 + (u.1 * along * toward + perp.1 * across) * TILE_PX,
            )
        };
        let is_vehicle = |kind: &EntityKind| {
            matches!(
                kind,
                EntityKind::Tank | EntityKind::ScoutCar | EntityKind::CommandCar
            )
        };
        let vehicles: usize = side
            .iter()
            .filter(|(kind, _)| is_vehicle(kind))
            .map(|(_, n)| n)
            .sum();
        let mut index = 0u32;
        let mut row_vehicle = 0usize;
        let mut infantry = 0usize;
        let mut spawned = 0usize;
        for (kind, count) in side {
            for _ in 0..*count {
                index += 1;
                let (along, across) = if is_vehicle(kind) {
                    let slot = row_vehicle;
                    row_vehicle += 1;
                    (1.5, (slot as f32 - (vehicles as f32 - 1.0) / 2.0) * 3.0)
                } else {
                    let slot = infantry;
                    infantry += 1;
                    let row = slot / 8;
                    let col = slot % 8;
                    (-(row as f32) * 1.4, (col as f32 - 3.5) * 1.3)
                };
                let jx = jitter(setup.seed.wrapping_add(salt), index) * 0.6;
                let jy = jitter(setup.seed.wrapping_add(salt + 7), index) * 0.6;
                let base = at(along + jx, across + jy);
                let points: Vec<(f32, f32)> = [(0.0, 0.0), (0.7, 0.0), (-0.7, 0.0), (0.0, 0.7), (0.0, -0.7), (1.4, 1.4), (-1.4, -1.4)]
                    .iter()
                    .map(|(ox, oy)| (base.0 + ox * TILE_PX, base.1 + oy * TILE_PX))
                    .collect();
                if spawn_first(&mut game, owner, *kind, &points) {
                    spawned += 1;
                }
            }
        }
        spawned
    };
    let want_ours: usize = setup.scenario.ours.iter().map(|(_, n)| n).sum();
    let want_theirs: usize = setup.scenario.theirs.iter().map(|(_, n)| n).sum();
    let snap = !setup.scenario.ours_at.is_empty();
    let abs = setup.scenario.abs;
    let world = |along: f32, across: f32| {
        if abs {
            // Real-map replay: the two numbers are the unit's tile x and y.
            return ((along + 0.0) * TILE_PX, (across + 0.0) * TILE_PX);
        }
        (
            mid.0 + (u.0 * along + perp.0 * across) * TILE_PX,
            mid.1 + (u.1 * along + perp.1 * across) * TILE_PX,
        )
    };
    if snap {
        for (owner, side, salt) in [
            (candidate, &setup.scenario.ours_at, 101u32),
            (opponent, &setup.scenario.theirs_at, 202u32),
        ] {
            let mut spawned = 0usize;
            for (index, (kind, along, across)) in side.iter().enumerate() {
                let jx = jitter(setup.seed.wrapping_add(salt), index as u32 + 1) * 0.5;
                let jy = jitter(setup.seed.wrapping_add(salt + 7), index as u32 + 1) * 0.5;
                let base = world(along + jx, across + jy);
                let points: Vec<(f32, f32)> = [(0.0, 0.0), (0.7, 0.0), (-0.7, 0.0), (0.0, 0.7), (0.0, -0.7), (1.4, 1.4), (-1.4, -1.4), (2.1, 0.0), (-2.1, 0.0)]
                    .iter()
                    .map(|(ox, oy)| (base.0 + ox * TILE_PX, base.1 + oy * TILE_PX))
                    .collect();
                if spawn_first(&mut game, owner, *kind, &points) {
                    spawned += 1;
                }
            }
            if spawned * 10 < side.len() * if abs { 8 } else { 9 } {
                return Err(format!("spawn failed in {} ({spawned}/{})", setup.scenario.name, side.len()));
            }
        }
    } else if place(candidate, &setup.scenario.ours, 1.0, 101) != want_ours
        || place(opponent, &setup.scenario.theirs, -1.0, 202) != want_theirs
    {
        return Err(format!("spawn failed in {}", setup.scenario.name));
    }
    let start_units = |game: &Game, owner: u32| -> BTreeMap<u32, (EntityKind, u32)> {
        game.snapshot_full_for(owner)
            .entities
            .iter()
            .filter(|e| e.owner == owner)
            .filter_map(|e| {
                let kind: EntityKind = e.kind.parse().ok()?;
                (kind.is_unit() && kind != EntityKind::Worker).then_some((e.id, (kind, e.max_hp)))
            })
            .collect()
    };
    let ours = start_units(&game, candidate);
    let theirs = start_units(&game, opponent);
    let centroid = |side: &[(EntityKind, f32, f32)]| {
        let n = side.len().max(1) as f32;
        world(
            side.iter().map(|unit| unit.1).sum::<f32>() / n,
            side.iter().map(|unit| unit.2).sum::<f32>() / n,
        )
    };
    let cand_center = if snap {
        centroid(&setup.scenario.ours_at)
    } else {
        (
            mid.0 - u.0 * setup.gap / 2.0 * TILE_PX,
            mid.1 - u.1 * setup.gap / 2.0 * TILE_PX,
        )
    };
    let opp_center = if snap {
        centroid(&setup.scenario.theirs_at)
    } else {
        (
            mid.0 + u.0 * setup.gap / 2.0 * TILE_PX,
            mid.1 + u.1 * setup.gap / 2.0 * TILE_PX,
        )
    };
    let cand_side = LabSide {
        params: setup.cand.params,
        hold: setup.scenario.us_hold,
        memory: MatchupMemory::default(),
        goal: opp_center,
        home,
        started: false,
        last_check: 0,
    };
    let opp_side = LabSide {
        params: None,
        hold: setup.scenario.hold,
        memory: MatchupMemory::default(),
        goal: cand_center,
        home: enemy,
        started: false,
        last_check: 0,
    };
    let (first, second): (Box<dyn AiStrategy>, Box<dyn AiStrategy>) = if candidate == 1 {
        (Box::new(cand_side), Box::new(opp_side))
    } else {
        (Box::new(opp_side), Box::new(cand_side))
    };
    let mut controllers = vec![
        AiController::with_strategy(1, first),
        AiController::with_strategy(2, second),
    ];
    let hp_of = |game: &Game, owner: u32, units: &BTreeMap<u32, (EntityKind, u32)>| {
        let snapshot = game.snapshot_full_for(owner);
        units
            .iter()
            .map(|(id, (kind, max))| {
                let hp = snapshot
                    .entities
                    .iter()
                    .find(|e| e.id == *id)
                    .map_or(0, |e| e.hp);
                (*kind, hp, *max)
            })
            .collect::<Vec<_>>()
    };
    let alive = |rows: &[(EntityKind, u32, u32)]| rows.iter().filter(|r| r.1 > 0).count();
    // The fight is over when one side is dead, or nobody has been hurt for 20 seconds.
    let mut last_total = u32::MAX;
    let mut last_change = 0u32;
    while game.tick_count() < setup.ticks {
        CanonicalAiTickDriver::run(&mut game, &mut controllers, AiAlivePolicy::Normal);
        let _ = game.tick();
        if game.tick_count() % 15 == 0 {
            let a = hp_of(&game, candidate, &ours);
            let b = hp_of(&game, opponent, &theirs);
            if alive(&a) == 0 || alive(&b) == 0 {
                break;
            }
            let total: u32 = a.iter().chain(&b).map(|row| row.1).sum();
            if total != last_total {
                last_total = total;
                last_change = game.tick_count();
            } else if game.tick_count() - last_change >= 600 {
                break;
            }
        }
    }
    let a = hp_of(&game, candidate, &ours);
    let b = hp_of(&game, opponent, &theirs);
    let lost = |rows: &[(EntityKind, u32, u32)]| -> f32 {
        rows.iter()
            .map(|(kind, hp, max)| matchup::cost(*kind) * (1.0 - *hp as f32 / (*max).max(1) as f32))
            .sum()
    };
    Ok(FightResult {
        our_lost: lost(&a),
        their_lost: lost(&b),
        our_dead: a.len() - alive(&a),
        their_dead: b.len() - alive(&b),
        our_alive: alive(&a),
        their_alive: alive(&b),
        ticks: game.tick_count(),
        orders: String::new(),
    })
}

pub fn run_from_env() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let get = |flag: &str| -> Option<String> {
        args.iter()
            .position(|a| a == flag)
            .and_then(|i| args.get(i + 1).cloned())
    };
    let scenarios = match get("--scenarios").map(|s| parse_scenarios(&s)) {
        Some(Ok(s)) => s,
        Some(Err(err)) => {
            eprintln!("ai-engage-lab: {err}");
            process::exit(2);
        }
        None => {
            eprintln!("ai-engage-lab: --scenarios is required");
            process::exit(2);
        }
    };
    let cands = match parse_cands(&get("--cands").unwrap_or_else(|| "naive;mm".to_string())) {
        Ok(c) => c,
        Err(err) => {
            eprintln!("ai-engage-lab: {err}");
            process::exit(2);
        }
    };
    let seeds: u32 = get("--seeds").and_then(|s| s.parse().ok()).unwrap_or(6);
    let map = get("--map").unwrap_or_else(|| "1v1 No Terrain".to_string());
    let ticks: u32 = get("--ticks").and_then(|s| s.parse().ok()).unwrap_or(2700);
    let gap: f32 = get("--gap").and_then(|s| s.parse().ok()).unwrap_or(20.0);
    let threads: usize = get("--threads").and_then(|s| s.parse().ok()).unwrap_or(8).min(8);
    let meth_us = get("--meth-us").is_none_or(|s| s != "0");
    let meth_them = get("--meth-them").is_none_or(|s| s != "0");
    let fights_out = get("--fights");
    let mut jobs = Vec::new();
    for (si, scenario) in scenarios.iter().enumerate() {
        for (ci, _) in cands.iter().enumerate() {
            for seed in 0..seeds {
                for candidate in [1u32, 2] {
                    jobs.push((si, ci, seed, candidate));
                }
            }
        }
    }
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .expect("thread pool");
    let results: Vec<((usize, usize, u32, u32), Result<FightResult, String>)> = pool.install(|| {
        jobs.par_iter()
            .map(|&(si, ci, seed, candidate)| {
                let scenario = &scenarios[si];
                let (candidate, seed) = match scenario.slot {
                    Some(slot) if slot == 1 || slot == 2 => (slot, seed * 2 + candidate - 1),
                    _ => (candidate, seed),
                };
                let setup = Setup {
                    scenario,
                    cand: &cands[ci],
                    candidate,
                    seed,
                    map: &map,
                    ticks,
                    gap: scenario.gap.unwrap_or(gap),
                    meth_us,
                    meth_them,
                };
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run_fight(&setup)))
                    .unwrap_or_else(|_| Err("panic".to_string()));
                ((si, ci, seed, candidate), result)
            })
            .collect()
    });
    if let Some(path) = fights_out {
        if let Ok(mut file) = std::fs::File::create(&path) {
            for ((si, ci, seed, candidate), result) in &results {
                if let Ok(r) = result {
                    let _ = writeln!(
                        file,
                        "{}\t{}\t{}\tp{}\t{:.0}\t{:.0}\t{}\t{}\t{}\t{}\t{}{}",
                        scenarios[*si].name, cands[*ci].label, seed, candidate, r.our_lost, r.their_lost,
                        r.our_dead, r.their_dead, r.our_alive, r.their_alive, r.ticks, r.orders
                    );
                }
            }
        }
    }
    println!(
        "{:<18} {:<14} {:>3} {:>5} {:>5} {:>7} {:>7} {:>6} {:>7} {:>9}",
        "scenario", "cand", "n", "win", "loss", "ourLost", "thrLost", "trade", "net", "dead o/t"
    );
    for (si, scenario) in scenarios.iter().enumerate() {
        for (ci, cand) in cands.iter().enumerate() {
            let rows: Vec<&FightResult> = results
                .iter()
                .filter(|((s, c, _, _), _)| *s == si && *c == ci)
                .filter_map(|(_, r)| r.as_ref().ok())
                .collect();
            let errors = results
                .iter()
                .filter(|((s, c, _, _), r)| *s == si && *c == ci && r.is_err())
                .count();
            if rows.is_empty() {
                println!("{:<18} {:<14} errors={errors}", scenario.name, cand.label);
                continue;
            }
            let n = rows.len() as f32;
            let wins = rows.iter().filter(|r| r.their_alive == 0 && r.our_alive > 0).count();
            let losses = rows.iter().filter(|r| r.our_alive == 0 && r.their_alive > 0).count();
            let our: f32 = rows.iter().map(|r| r.our_lost).sum();
            let their: f32 = rows.iter().map(|r| r.their_lost).sum();
            let od: f32 = rows.iter().map(|r| r.our_dead as f32).sum::<f32>() / n;
            let td: f32 = rows.iter().map(|r| r.their_dead as f32).sum::<f32>() / n;
            println!(
                "{:<18} {:<14} {:>3} {:>5} {:>5} {:>7.0} {:>7.0} {:>6.2} {:>7.0} {:>4.1}/{:<4.1}{}",
                scenario.name,
                cand.label,
                rows.len(),
                wins,
                losses,
                our / n,
                their / n,
                their / our.max(1.0),
                (their - our) / n,
                od,
                td,
                if errors > 0 { format!(" err={errors}") } else { String::new() }
            );
        }
    }
}
