//! One headless squad fight: build a Lab game on a real map, strip both starts down to their
//! Resource Depot, spawn the two squads, and run both controllers through the canonical AI tick
//! driver until one squad is destroyed or the tick cap.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use super::scenario::{place_squads, PlacementInput, Scenario};
use crate::ai_core::squad_micro::{
    squad_member_max_hp, RifleSquadParams, SquadMicroStrategy, SQUAD_KIND,
};
use crate::live::{AiAlivePolicy, AiController, CanonicalAiTickDriver};
use crate::selfplay::{canonical_profile_id, is_safe_artifact_name, server_build_sha};
use rts_sim::game::lab::{LabOp, LabSetPlayerResources, LabSpawnEntity};
use rts_sim::game::map::Map;
use rts_sim::game::replay::ReplayStartComposition;
use rts_sim::game::{Game, PlayerInit};
use rts_sim::protocol::{kinds, Event, Snapshot};

/// Start assignment seed. Fixed so every fight uses the same corners; side swapping, not the map
/// seed, is what cancels positional and tick-order bias.
const MAP_SEED: u32 = 1;
const TILE_PX: f32 = rts_rules::balance::TILE_SIZE as f32;

/// Who controls a squad.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ControllerSpec {
    /// A full built-in AI profile, exactly as it runs in live matches.
    Profile(&'static str),
    /// The rifle squad planner with explicit parameters.
    Micro(RifleSquadParams),
}

impl ControllerSpec {
    /// `naive`, `micro`, `micro:<params>`, `profile:<id>`, or a bare profile id such as `ai_2_1`.
    pub(crate) fn parse(input: &str) -> Result<Self, String> {
        let input = input.trim();
        match input {
            "naive" => return Ok(Self::Micro(RifleSquadParams::naive())),
            "micro" => return Ok(Self::Micro(RifleSquadParams::micro())),
            _ => {}
        }
        if let Some(params) = input.strip_prefix("micro:") {
            return RifleSquadParams::parse(params).map(Self::Micro);
        }
        let profile = input.strip_prefix("profile:").unwrap_or(input);
        canonical_profile_id(profile).map(Self::Profile).ok_or_else(|| {
            format!(
                "unknown controller {input:?}; expected naive, micro, micro:<key=value,...>, or a profile id"
            )
        })
    }

    pub(crate) fn label(&self) -> String {
        match self {
            Self::Profile(id) => (*id).to_string(),
            Self::Micro(params) => format!("micro:{params}"),
        }
    }

    fn short_label(&self) -> &'static str {
        match self {
            Self::Profile(id) => id,
            Self::Micro(_) => "squad_micro",
        }
    }

    fn controller(&self, player: u32) -> AiController {
        match self {
            Self::Profile(id) => AiController::with_profile_id(player, id),
            Self::Micro(params) => {
                AiController::with_strategy(player, Box::new(SquadMicroStrategy::new(*params)))
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ReplayPolicy {
    None,
    Losses,
    All,
}

impl ReplayPolicy {
    pub(crate) fn parse(value: &str) -> Result<Self, String> {
        match value {
            "none" => Ok(Self::None),
            "losses" => Ok(Self::Losses),
            "all" => Ok(Self::All),
            other => Err(format!(
                "unknown replay policy {other:?}; expected none, losses, or all"
            )),
        }
    }
}

pub(crate) struct FightSpec<'a> {
    pub(crate) scenario: &'a Scenario,
    pub(crate) candidate: &'a ControllerSpec,
    pub(crate) opponent: &'a ControllerSpec,
    pub(crate) candidate_player: u32,
    pub(crate) seed: u32,
    pub(crate) max_ticks: u32,
    pub(crate) map_name: &'a str,
    pub(crate) squad_size: usize,
    pub(crate) replay_policy: ReplayPolicy,
    pub(crate) replay_dir: &'a Path,
    pub(crate) replay_tag: &'a str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum FightOutcome {
    Win,
    Loss,
    Draw,
    NoContact,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FightResult {
    pub(crate) scenario: String,
    pub(crate) candidate_player: u32,
    pub(crate) seed: u32,
    pub(crate) outcome: FightOutcome,
    pub(crate) ticks: u32,
    pub(crate) first_damage_tick: Option<u32>,
    pub(crate) candidate_survivors: u32,
    pub(crate) opponent_survivors: u32,
    pub(crate) candidate_hp: u32,
    pub(crate) opponent_hp: u32,
    /// `(candidate HP - opponent HP) / full squad HP`, in `[-1, 1]`. The tuning objective.
    pub(crate) hp_margin: f32,
    pub(crate) damage_dealt: u32,
    pub(crate) damage_taken: u32,
    pub(crate) candidate_shots: u32,
    pub(crate) opponent_shots: u32,
    /// A combat unit outside the starting squads appeared (for example, trained mid-fight).
    pub(crate) reinforced: bool,
    pub(crate) replay_artifact: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct SquadState {
    alive: u32,
    hp: u32,
}

pub(crate) fn run_fight(spec: &FightSpec<'_>) -> Result<FightResult, String> {
    let candidate = spec.candidate_player;
    let opponent = if candidate == 1 { 2 } else { 1 };
    if candidate != 1 && candidate != 2 {
        return Err(format!("candidate player must be 1 or 2, got {candidate}"));
    }
    let player = |id: u32, spec: &ControllerSpec, color: &str| PlayerInit {
        id,
        team_id: id,
        faction_id: "kriegsia".to_string(),
        name: spec.short_label().to_string(),
        color: color.to_string(),
        is_ai: true,
    };
    let (first, second) = if candidate == 1 {
        (spec.candidate, spec.opponent)
    } else {
        (spec.opponent, spec.candidate)
    };
    let players = vec![player(1, first, "#4cc9f0"), player(2, second, "#f72585")];
    let map = Map::load_for_players(spec.map_name, &[(1, 1), (2, 2)], MAP_SEED)?;
    let world_px = (map.world_width_px(), map.world_height_px());
    let metadata = Map::metadata_for_name(spec.map_name)?;
    let mut game = Game::new_lab(&players, spec.seed, map, metadata);

    let bases = depot_positions(&game.snapshot_full_for(1));
    let base = |id: u32| {
        bases.get(&id).copied().ok_or_else(|| {
            format!(
                "player {id} has no starting Resource Depot on {}",
                spec.map_name
            )
        })
    };
    let placement = place_squads(
        spec.scenario,
        &PlacementInput {
            squad_size: spec.squad_size,
            candidate_base: base(candidate)?,
            opponent_base: base(opponent)?,
            tile_px: TILE_PX,
            world_px,
            seed: spec.seed,
        },
    );
    for id in [1, 2] {
        let starting_units: Vec<u32> = game
            .lab_owned_units(id)
            .map_err(|err| lab_error(spec, err))?
            .into_iter()
            .map(|(unit, _)| unit)
            .collect();
        game.apply_lab_op(LabOp::DeleteEntities(starting_units))
            .map_err(|err| lab_error(spec, err))?;
        game.apply_lab_op(LabOp::SetPlayerResources(LabSetPlayerResources {
            player_id: id,
            steel: 0,
            oil: 0,
        }))
        .map_err(|err| lab_error(spec, err))?;
    }
    let spawns = [
        (candidate, &placement.candidate),
        (opponent, &placement.opponent),
    ]
    .into_iter()
    .flat_map(|(owner, points)| {
        points.iter().map(move |&(x, y)| LabSpawnEntity {
            owner,
            kind: SQUAD_KIND,
            x,
            y,
            completed: true,
        })
    })
    .collect();
    game.apply_lab_op(LabOp::SpawnEntities(spawns))
        .map_err(|err| lab_error(spec, err))?;

    let mut squads: BTreeMap<u32, BTreeSet<u32>> = BTreeMap::new();
    for id in [1, 2] {
        let squad: BTreeSet<u32> = game
            .lab_owned_units(id)
            .map_err(|err| lab_error(spec, err))?
            .into_iter()
            .filter(|(_, kind)| *kind == SQUAD_KIND)
            .map(|(unit, _)| unit)
            .collect();
        if squad.len() != spec.squad_size {
            return Err(format!(
                "player {id} spawned {} of {} squad members in {}",
                squad.len(),
                spec.squad_size,
                spec.scenario.id
            ));
        }
        squads.insert(id, squad);
    }
    let replay_start = (spec.replay_policy != ReplayPolicy::None)
        .then(|| ReplayStartComposition::capture(&game, server_build_sha()))
        .transpose()?;

    let mut controllers = vec![first.controller(1), second.controller(2)];
    let full_hp = squad_member_max_hp().saturating_mul(spec.squad_size as u32);
    let mut shots: BTreeMap<u32, u32> = BTreeMap::new();
    let mut first_damage_tick = None;
    let mut reinforced = false;
    let mut states = squad_states(&game.snapshot_full_for(1), &squads, &mut reinforced);
    while game.tick_count() < spec.max_ticks {
        CanonicalAiTickDriver::run(&mut game, &mut controllers, AiAlivePolicy::Normal);
        let events = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| game.tick()))
            .map_err(|_| format!("Game::tick panicked in skirmish {}", spec.scenario.id))?;
        for (recipient, events) in events {
            let Some(squad) = squads.get(&recipient) else {
                continue;
            };
            for event in events {
                if let Event::Attack { from, .. } = event {
                    if squad.contains(&from) {
                        *shots.entry(recipient).or_default() += 1;
                    }
                }
            }
        }
        states = squad_states(&game.snapshot_full_for(1), &squads, &mut reinforced);
        if first_damage_tick.is_none() && states.values().any(|state| state.hp < full_hp) {
            first_damage_tick = Some(game.tick_count());
        }
        if states.values().any(|state| state.alive == 0) {
            break;
        }
    }

    let mine = states.get(&candidate).copied().unwrap_or_default();
    let theirs = states.get(&opponent).copied().unwrap_or_default();
    let outcome = match (mine.alive, theirs.alive) {
        (0, 0) => FightOutcome::Draw,
        (_, 0) => FightOutcome::Win,
        (0, _) => FightOutcome::Loss,
        _ if first_damage_tick.is_none() => FightOutcome::NoContact,
        _ => FightOutcome::Draw,
    };
    let keep_replay = match spec.replay_policy {
        ReplayPolicy::None => false,
        ReplayPolicy::Losses => outcome != FightOutcome::Win,
        ReplayPolicy::All => true,
    };
    let replay_artifact = match (&replay_start, keep_replay) {
        (Some(start), true) => {
            let name = replay_name(spec);
            let winner = match outcome {
                FightOutcome::Win => Some(candidate),
                FightOutcome::Loss => Some(opponent),
                FightOutcome::Draw | FightOutcome::NoContact => None,
            };
            write_replay(&spec.replay_dir.join(&name), start, &game, winner)?;
            Some(name)
        }
        _ => None,
    };
    Ok(FightResult {
        scenario: spec.scenario.id.clone(),
        candidate_player: candidate,
        seed: spec.seed,
        outcome,
        ticks: game.tick_count(),
        first_damage_tick,
        candidate_survivors: mine.alive,
        opponent_survivors: theirs.alive,
        candidate_hp: mine.hp,
        opponent_hp: theirs.hp,
        hp_margin: (mine.hp as f32 - theirs.hp as f32) / full_hp.max(1) as f32,
        damage_dealt: full_hp.saturating_sub(theirs.hp),
        damage_taken: full_hp.saturating_sub(mine.hp),
        candidate_shots: shots.get(&candidate).copied().unwrap_or(0),
        opponent_shots: shots.get(&opponent).copied().unwrap_or(0),
        reinforced,
        replay_artifact,
    })
}

fn lab_error(spec: &FightSpec<'_>, err: rts_sim::game::lab::LabError) -> String {
    format!(
        "lab setup failed for scenario {} (candidate player {}, seed {}) on {}: {err:?}",
        spec.scenario.id, spec.candidate_player, spec.seed, spec.map_name
    )
}

fn depot_positions(snapshot: &Snapshot) -> BTreeMap<u32, (f32, f32)> {
    let mut bases = BTreeMap::new();
    for entity in &snapshot.entities {
        if entity.owner != 0 && entity.kind == kinds::RESOURCE_DEPOT {
            bases.entry(entity.owner).or_insert((entity.x, entity.y));
        }
    }
    bases
}

fn squad_states(
    snapshot: &Snapshot,
    squads: &BTreeMap<u32, BTreeSet<u32>>,
    reinforced: &mut bool,
) -> BTreeMap<u32, SquadState> {
    let mut states: BTreeMap<u32, SquadState> = squads
        .keys()
        .map(|id| (*id, SquadState::default()))
        .collect();
    for entity in &snapshot.entities {
        let Some(squad) = squads.get(&entity.owner) else {
            continue;
        };
        if squad.contains(&entity.id) {
            if entity.hp > 0 {
                let state = states.entry(entity.owner).or_default();
                state.alive += 1;
                state.hp += entity.hp;
            }
        } else if is_combat_unit(&entity.kind) {
            *reinforced = true;
        }
    }
    states
}

/// Armed units other than workers; a new one mid-fight means a squad was reinforced.
fn is_combat_unit(kind: &str) -> bool {
    kind != kinds::WORKER
        && kind
            .parse::<rts_sim::game::entity::EntityKind>()
            .is_ok_and(|kind| {
                kind.is_unit() && rts_rules::combat::default_weapon_profile(kind).is_some()
            })
}

fn replay_name(spec: &FightSpec<'_>) -> String {
    let name = format!(
        "skirmish_{}_{}_p{}_s{}",
        spec.replay_tag, spec.scenario.id, spec.candidate_player, spec.seed
    );
    if is_safe_artifact_name(&name) {
        name
    } else {
        format!(
            "skirmish_{}_p{}_s{}",
            spec.scenario.id, spec.candidate_player, spec.seed
        )
    }
}

fn write_replay(
    dir: &PathBuf,
    start: &ReplayStartComposition,
    game: &Game,
    winner: Option<u32>,
) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|err| err.to_string())?;
    let artifact = start.finalize(game, winner, game.scores());
    let json = serde_json::to_vec_pretty(&artifact).map_err(|err| err.to_string())?;
    fs::write(dir.join("replay.json"), json).map_err(|err| err.to_string())
}
