//! `ai-skirmish`: a headless squad micro testbed.
//!
//! Each fight starts from a real map in Lab mode with both starts reduced to their Resource Depot
//! and a fixed rifle squad per side, then runs both controllers through the canonical AI tick
//! driver (same cadence, fog-filtered frames, and command validation as live matches). The
//! opponent is normally the full `ai_2_1` profile, whose first attack wave is exactly four
//! Riflemen. Every scenario is fought from both player slots because symmetric rifle fights are
//! otherwise decided by which side resolves its volley first.
//!
//! `evaluate` mode scores one controller on a scenario suite; `--sweep N` random-searches the
//! squad planner's parameters on the `train` suite and re-checks the leaders on `holdout`.

mod fight;
mod report;
mod scenario;
#[cfg(test)]
mod tests;

use std::fs;
use std::path::{Path, PathBuf};
use std::process;

use rand::rngs::SmallRng;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};
use rayon::prelude::*;
use serde::Serialize;

use crate::ai_core::squad_micro::{squad_shot_damage, FocusMode, RifleSquadParams};
use crate::selfplay::server_build_sha;
use fight::{run_fight, ControllerSpec, FightResult, FightSpec, ReplayPolicy};
use report::{Aggregate, BriefContext, ScenarioAggregate, SweepEntry};
use scenario::Suite;

const DEFAULT_TICKS: u32 = 1_800;
const DEFAULT_MAP: &str = "1v1 No Terrain";
const DEFAULT_SQUAD_SIZE: usize = 4;
const MAX_SQUAD_SIZE: usize = 24;
const DEFAULT_TOP: usize = 3;
const SCHEMA_VERSION: u32 = 1;

#[derive(Debug)]
struct CliConfig {
    candidate: ControllerSpec,
    opponent: ControllerSpec,
    suite: Suite,
    seeds: u32,
    seed_start: u32,
    ticks: u32,
    map_name: String,
    squad_size: usize,
    out_dir: PathBuf,
    replays: ReplayPolicy,
    replay_dir: PathBuf,
    tag: String,
    sweep: Option<u32>,
    sweep_seed: u64,
    mutate: bool,
    top: usize,
    list_scenarios: bool,
}

pub fn run_from_env() {
    let config = match parse_args(std::env::args().skip(1)) {
        Ok(Some(config)) => config,
        Ok(None) => return,
        Err(err) => {
            eprintln!("ai-skirmish: {err}\n\n{}", usage());
            process::exit(2);
        }
    };
    if config.list_scenarios {
        for scenario in config.suite.scenarios() {
            println!("{}", scenario.id);
        }
        return;
    }
    let result = match config.sweep {
        Some(trials) => run_sweep(&config, trials),
        None => run_evaluation(&config).map(|_| ()),
    };
    if let Err(err) = result {
        eprintln!("ai-skirmish failed: {err}");
        process::exit(1);
    }
}

fn usage() -> String {
    format!(
        "Usage: ai-skirmish [options]

Controllers (--candidate, --opponent):
  naive                     attack-move, simulation auto-targeting (no micro)
  micro                     squad planner defaults: {micro}
  micro:<key=value,...>     planner with overrides; start with `naive,` to override the naive preset
                            keys: {keys}
  <profile id>              a full built-in AI profile, e.g. ai_2_1 or jeffs_ai

Options:
  --candidate SPEC          controller under test (default: micro)
  --opponent SPEC           fixed opponent (default: ai_2_1)
  --suite NAME              smoke | train | holdout | all (default: train)
  --seeds N                 placement-jitter seeds per scenario and side (default: 1)
  --seed-start N            first seed (default: 0)
  --ticks N                 tick cap per fight (default: {DEFAULT_TICKS})
  --map NAME                map display name (default: {DEFAULT_MAP})
  --squad-size N            riflemen per side (default: {DEFAULT_SQUAD_SIZE})
  --out-dir DIR             report directory (default: server/target/ai-skirmish/<tag>)
  --replays POLICY          none | losses | all (default: losses; none while sweeping)
  --replay-dir DIR          replay artifact root (default: server/target/selfplay-artifacts)
  --tag NAME                run name used in artifact names (default: latest)
  --sweep N                 random-search N planner parameter sets on train, re-check top on holdout
  --sweep-seed N            sweep sampler seed (default: 1)
  --mutate                  sweep trials change 1-3 settings of --candidate instead of sampling
                            every setting at random (local refinement of a known-good set)
  --top N                   sweep entries re-run on holdout (default: {DEFAULT_TOP})
  --list-scenarios          print the suite's scenario ids and exit",
        micro = RifleSquadParams::micro(),
        keys = RifleSquadParams::KEYS.join(", "),
    )
}

fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Option<CliConfig>, String> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let server_target = manifest_dir
        .ancestors()
        .nth(2)
        .unwrap_or(manifest_dir)
        .join("target");
    let mut candidate = ControllerSpec::Micro(RifleSquadParams::micro());
    let mut opponent = ControllerSpec::parse("ai_2_1")?;
    let mut suite = Suite::Train;
    let mut seeds = 1;
    let mut seed_start = 0;
    let mut ticks = DEFAULT_TICKS;
    let mut map_name = DEFAULT_MAP.to_string();
    let mut squad_size = DEFAULT_SQUAD_SIZE;
    let mut out_dir = None;
    let mut replays = None;
    let mut replay_dir = server_target.join("selfplay-artifacts");
    let mut tag = "latest".to_string();
    let mut sweep = None;
    let mut sweep_seed = 1;
    let mut mutate = false;
    let mut top = DEFAULT_TOP;
    let mut list_scenarios = false;

    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        let mut value = || args.next().ok_or_else(|| format!("{arg} requires a value"));
        match arg.as_str() {
            "-h" | "--help" => {
                println!("{}", usage());
                return Ok(None);
            }
            "--candidate" => candidate = ControllerSpec::parse(&value()?)?,
            "--opponent" => opponent = ControllerSpec::parse(&value()?)?,
            "--suite" => suite = Suite::parse(&value()?)?,
            "--seeds" => seeds = parse_number(&arg, &value()?)?,
            "--seed-start" => seed_start = parse_number(&arg, &value()?)?,
            "--ticks" => ticks = parse_number(&arg, &value()?)?,
            "--map" => map_name = value()?,
            "--squad-size" => squad_size = parse_number(&arg, &value()?)?,
            "--out-dir" => out_dir = Some(PathBuf::from(value()?)),
            "--replays" => replays = Some(ReplayPolicy::parse(&value()?)?),
            "--replay-dir" => replay_dir = PathBuf::from(value()?),
            "--tag" => tag = value()?,
            "--sweep" => sweep = Some(parse_number(&arg, &value()?)?),
            "--sweep-seed" => sweep_seed = parse_number(&arg, &value()?)?,
            "--mutate" => mutate = true,
            "--top" => top = parse_number(&arg, &value()?)?,
            "--list-scenarios" => list_scenarios = true,
            other => return Err(format!("unknown argument {other:?}")),
        }
    }
    if seeds == 0 || ticks == 0 {
        return Err("--seeds and --ticks must be greater than zero".to_string());
    }
    if mutate && !matches!(candidate, ControllerSpec::Micro(_)) {
        return Err("--mutate needs a micro candidate to refine".to_string());
    }
    if squad_size == 0 || squad_size > MAX_SQUAD_SIZE {
        return Err(format!(
            "--squad-size must be between 1 and {MAX_SQUAD_SIZE}"
        ));
    }
    if tag.is_empty()
        || !tag
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err("--tag may only contain ASCII letters, digits, '-' and '_'".to_string());
    }
    let out_dir = out_dir.unwrap_or_else(|| server_target.join("ai-skirmish").join(&tag));
    let replays = replays.unwrap_or(if sweep.is_some() {
        ReplayPolicy::None
    } else {
        ReplayPolicy::Losses
    });
    Ok(Some(CliConfig {
        candidate,
        opponent,
        suite,
        seeds,
        seed_start,
        ticks,
        map_name,
        squad_size,
        out_dir,
        replays,
        replay_dir,
        tag,
        sweep,
        sweep_seed,
        mutate,
        top,
        list_scenarios,
    }))
}

fn parse_number<T: std::str::FromStr>(flag: &str, value: &str) -> Result<T, String> {
    value
        .parse()
        .map_err(|_| format!("{flag} expects a non-negative integer, got {value:?}"))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EvaluationReport {
    schema: u32,
    tool: &'static str,
    build: String,
    candidate: String,
    opponent: String,
    suite: Suite,
    map_name: String,
    squad_size: usize,
    max_ticks: u32,
    seed_start: u32,
    seeds: u32,
    aggregate: Aggregate,
    by_side: std::collections::BTreeMap<String, Aggregate>,
    by_scenario: Vec<ScenarioAggregate>,
    fights: Vec<FightResult>,
}

struct Evaluation {
    aggregate: Aggregate,
    report: EvaluationReport,
    scenarios: usize,
}

/// Fight every scenario of `suite` from both player slots and every seed, in parallel.
fn evaluate(
    config: &CliConfig,
    candidate: &ControllerSpec,
    suite: Suite,
    replays: ReplayPolicy,
    tag: &str,
) -> Result<Evaluation, String> {
    let scenarios = suite.scenarios();
    let mut jobs = Vec::new();
    for scenario in &scenarios {
        for candidate_player in [1, 2] {
            for seed in config.seed_start..config.seed_start.saturating_add(config.seeds) {
                jobs.push((scenario, candidate_player, seed));
            }
        }
    }
    let fights: Vec<FightResult> = jobs
        .par_iter()
        .map(|&(scenario, candidate_player, seed)| {
            run_fight(&FightSpec {
                scenario,
                candidate,
                opponent: &config.opponent,
                candidate_player,
                seed,
                max_ticks: config.ticks,
                map_name: &config.map_name,
                squad_size: config.squad_size,
                replay_policy: replays,
                replay_dir: &config.replay_dir,
                replay_tag: tag,
            })
        })
        .collect::<Result<_, _>>()?;
    let shot_damage = squad_shot_damage();
    let aggregate = Aggregate::from_results(&fights, shot_damage);
    let report = EvaluationReport {
        schema: SCHEMA_VERSION,
        tool: "ai-skirmish",
        build: server_build_sha().to_string(),
        candidate: candidate.label(),
        opponent: config.opponent.label(),
        suite,
        map_name: config.map_name.clone(),
        squad_size: config.squad_size,
        max_ticks: config.ticks,
        seed_start: config.seed_start,
        seeds: config.seeds,
        aggregate: aggregate.clone(),
        by_side: report::by_side(&fights, shot_damage),
        by_scenario: report::by_scenario(&fights, shot_damage),
        fights,
    };
    Ok(Evaluation {
        aggregate,
        report,
        scenarios: scenarios.len(),
    })
}

fn write_evaluation(dir: &Path, evaluation: &Evaluation) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|err| err.to_string())?;
    let report = &evaluation.report;
    let json = serde_json::to_vec_pretty(report).map_err(|err| err.to_string())?;
    fs::write(dir.join("summary.json"), json).map_err(|err| err.to_string())?;
    let brief = report::evaluation_brief(
        &BriefContext {
            candidate: &report.candidate,
            opponent: &report.opponent,
            suite: report.suite.as_str(),
            scenarios: evaluation.scenarios,
            seeds: report.seeds,
            map_name: &report.map_name,
            squad_size: report.squad_size,
            max_ticks: report.max_ticks,
            build: &report.build,
        },
        &report.aggregate,
        &report.by_side,
        &report.by_scenario,
        &report.fights,
    );
    fs::write(dir.join("brief.md"), brief).map_err(|err| err.to_string())
}

fn print_aggregate(label: &str, aggregate: &Aggregate) {
    println!(
        "{label}: fights={} W-L-D-nc={}-{}-{}-{} win_rate={:.1}% mean_hp_margin={:+.3} survivors={:.2}/{:.2}",
        aggregate.fights,
        aggregate.wins,
        aggregate.losses,
        aggregate.draws,
        aggregate.no_contact,
        aggregate.win_rate * 100.0,
        aggregate.mean_hp_margin,
        aggregate.mean_candidate_survivors,
        aggregate.mean_opponent_survivors,
    );
}

fn run_evaluation(config: &CliConfig) -> Result<Evaluation, String> {
    let evaluation = evaluate(
        config,
        &config.candidate,
        config.suite,
        config.replays,
        &config.tag,
    )?;
    write_evaluation(&config.out_dir, &evaluation)?;
    println!(
        "ai-skirmish: {} vs {} on suite {}",
        evaluation.report.candidate,
        evaluation.report.opponent,
        config.suite.as_str()
    );
    print_aggregate("all", &evaluation.aggregate);
    for (side, aggregate) in &evaluation.report.by_side {
        print_aggregate(&format!("as {side}"), aggregate);
    }
    if evaluation.aggregate.reinforced_fights > 0 {
        println!(
            "warning: {} fight(s) were reinforced mid-fight",
            evaluation.aggregate.reinforced_fights
        );
    }
    println!("report: {}", config.out_dir.join("brief.md").display());
    Ok(evaluation)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SweepReport {
    schema: u32,
    tool: &'static str,
    build: String,
    opponent: String,
    trials: u32,
    sweep_seed: u64,
    seeds: u32,
    entries: Vec<SweepEntry>,
}

/// Random search over planner parameters: rank on `train`, re-check the leaders on `holdout`,
/// then write a full evaluation (with loss replays) for the train champion.
fn run_sweep(config: &CliConfig, trials: u32) -> Result<(), String> {
    let mut rng = SmallRng::seed_from_u64(config.sweep_seed);
    let mut candidates: Vec<(String, RifleSquadParams)> = vec![
        ("naive".to_string(), RifleSquadParams::naive()),
        ("micro".to_string(), RifleSquadParams::micro()),
    ];
    if let ControllerSpec::Micro(params) = config.candidate {
        if !candidates.iter().any(|(_, existing)| *existing == params) {
            candidates.push(("candidate".to_string(), params));
        }
    }
    let base = match config.candidate {
        ControllerSpec::Micro(params) => params,
        ControllerSpec::Profile(_) => RifleSquadParams::micro(),
    };
    for trial in 0..trials {
        // Re-draw duplicates a few times; an occasional repeat only costs one evaluation.
        let mut params = base;
        for _ in 0..8 {
            params = if config.mutate {
                mutated_params(base, &mut rng)
            } else {
                random_params(&mut rng)
            };
            if !candidates.iter().any(|(_, existing)| *existing == params) {
                break;
            }
        }
        let prefix = if config.mutate { "mut" } else { "trial" };
        candidates.push((format!("{prefix}{trial}"), params));
    }

    let mut scored = Vec::with_capacity(candidates.len());
    for (index, (label, params)) in candidates.iter().enumerate() {
        let spec = ControllerSpec::Micro(*params);
        let evaluation = evaluate(config, &spec, Suite::Train, ReplayPolicy::None, &config.tag)?;
        println!(
            "[{}/{}] {label}: train margin {:+.3} win {:.1}%  {params}",
            index + 1,
            candidates.len(),
            evaluation.aggregate.mean_hp_margin,
            evaluation.aggregate.win_rate * 100.0
        );
        scored.push((label.clone(), *params, evaluation.aggregate));
    }
    scored.sort_by(|a, b| {
        b.2.mean_hp_margin
            .total_cmp(&a.2.mean_hp_margin)
            .then_with(|| b.2.win_rate.total_cmp(&a.2.win_rate))
    });

    let mut entries = Vec::with_capacity(scored.len());
    for (rank, (label, params, train)) in scored.into_iter().enumerate() {
        let holdout = if rank < config.top {
            let spec = ControllerSpec::Micro(params);
            let evaluation = evaluate(
                config,
                &spec,
                Suite::Holdout,
                ReplayPolicy::None,
                &config.tag,
            )?;
            Some(evaluation.aggregate)
        } else {
            None
        };
        entries.push(SweepEntry {
            rank: rank + 1,
            label,
            params: params.to_string(),
            train,
            holdout,
        });
    }

    fs::create_dir_all(&config.out_dir).map_err(|err| err.to_string())?;
    let report = SweepReport {
        schema: SCHEMA_VERSION,
        tool: "ai-skirmish",
        build: server_build_sha().to_string(),
        opponent: config.opponent.label(),
        trials,
        sweep_seed: config.sweep_seed,
        seeds: config.seeds,
        entries,
    };
    let json = serde_json::to_vec_pretty(&report).map_err(|err| err.to_string())?;
    fs::write(config.out_dir.join("sweep.json"), json).map_err(|err| err.to_string())?;
    fs::write(
        config.out_dir.join("brief.md"),
        report::sweep_brief(&report.opponent, trials, &report.entries),
    )
    .map_err(|err| err.to_string())?;

    println!("\nleaderboard (train margin | holdout margin):");
    for entry in report.entries.iter().take(config.top.max(5)) {
        println!(
            "#{} {:>9} {:+.3} | {}  {}",
            entry.rank,
            entry.label,
            entry.train.mean_hp_margin,
            entry
                .holdout
                .as_ref()
                .map(|h| format!("{:+.3}", h.mean_hp_margin))
                .unwrap_or_else(|| "   -  ".to_string()),
            entry.params
        );
    }
    if let Some(champion) = report.entries.first() {
        let params = RifleSquadParams::parse(&champion.params)?;
        let spec = ControllerSpec::Micro(params);
        let tag = format!("{}_champion", config.tag);
        let replays = if config.replays == ReplayPolicy::None {
            ReplayPolicy::Losses
        } else {
            config.replays
        };
        let evaluation = evaluate(config, &spec, Suite::All, replays, &tag)?;
        write_evaluation(&config.out_dir.join("champion"), &evaluation)?;
        print_aggregate("champion on all suites", &evaluation.aggregate);
    }
    println!("report: {}", config.out_dir.join("brief.md").display());
    Ok(())
}

/// `base` with one to three settings replaced by random draws.
fn mutated_params(base: RifleSquadParams, rng: &mut SmallRng) -> RifleSquadParams {
    let donor = random_params(rng);
    let mut keys = RifleSquadParams::KEYS.to_vec();
    keys.shuffle(rng);
    let mut params = base;
    for key in keys.into_iter().take(rng.gen_range(1..=3)) {
        params.copy_key_from(key, &donor);
    }
    params
}

fn random_params(rng: &mut SmallRng) -> RifleSquadParams {
    let pick = |rng: &mut SmallRng, values: &[f32]| *values.choose(rng).unwrap_or(&0.0);
    RifleSquadParams {
        focus: *[
            FocusMode::Weakest,
            FocusMode::Weakest,
            FocusMode::SquadNearest,
            FocusMode::Nearest,
        ]
        .choose(rng)
        .unwrap_or(&FocusMode::Weakest),
        overkill_guard: rng.gen_bool(0.7),
        retreat_hp: *[0, 0, 5, 10, 15, 20].choose(rng).unwrap_or(&0),
        retreat_tiles: pick(rng, &[1.0, 1.5, 2.0, 3.0, 4.0]),
        retreat_ticks: *[18, 30, 45, 60].choose(rng).unwrap_or(&30),
        hold: rng.gen_bool(0.5),
        contact_margin_tiles: pick(rng, &[0.0, 0.25, 0.5, 1.0, 1.5, 2.5]),
        regroup_tiles: pick(rng, &[0.0, 2.0, 3.0, 4.0, 6.0]),
        range_margin_tiles: pick(rng, &[0.0, 0.25, 0.5, 1.0]),
    }
}
