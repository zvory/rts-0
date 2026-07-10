//! Agent-oriented AI arena runner.
//!
//! The arena is intentionally a reporting layer over profile-backed self-play. It does not own
//! simulation authority or a second decision path; each run still goes through the public Game seam
//! and the existing deterministic replay verifier.
#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process;

use serde::Serialize;

use crate::ai_core::profile_manifest::{
    profile_identity_by_id, validate_profile_identity, AiProfileIdentity,
};
use crate::selfplay::{
    available_profile_ids, canonical_profile_id, run_profile_matchup_result, server_build_sha,
    ProfileMatchupEndReason, ProfileMatchupOptions, ProfileMatchupResult, ProfileMatchupTraceEntry,
};

const DEFAULT_TICKS: u32 = 25_000;
const DEFAULT_SEEDS: u32 = 3;
const DEFAULT_CANDIDATE: &str = "ai_2_1";
const DEFAULT_BASELINE: &str = "ai_turtle";
const ARENA_SCHEMA_VERSION: u32 = 1;

#[derive(Debug)]
struct CliConfig {
    candidate: String,
    baseline: String,
    seeds: u32,
    seed_start: u32,
    ticks: u32,
    out_dir: PathBuf,
    verify_replay: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ArenaReport {
    schema: u32,
    tool: &'static str,
    candidate: String,
    baseline: String,
    seed_start: u32,
    seeds: u32,
    max_ticks: u32,
    runs: Vec<ArenaRunSummary>,
    aggregate: ArenaAggregate,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ArenaRunSummary {
    manifest: ArenaRunManifest,
    result: ProfileMatchupResult,
    candidate_player_id: u32,
    baseline_player_id: u32,
    outcome: ArenaOutcome,
    artifact_dir: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ArenaRunManifest {
    schema: u32,
    tool: &'static str,
    server_build_sha: String,
    seed: u32,
    max_ticks: u32,
    side: ArenaSide,
    candidate_profile_id: String,
    baseline_profile_id: String,
    profiles: BTreeMap<String, AiProfileIdentity>,
    replay_artifact: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum ArenaSide {
    CandidatePlayerOne,
    CandidatePlayerTwo,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct ArenaOutcome {
    winner_profile: Option<String>,
    candidate_won: bool,
    baseline_won: bool,
    tick_cap: bool,
    end_reason: ProfileMatchupEndReason,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct ArenaAggregate {
    runs: u32,
    candidate_wins: u32,
    baseline_wins: u32,
    unresolved_draws: u32,
    starting_city_centre_wins: u32,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DecisionTraceRecord {
    tick: u32,
    player_id: u32,
    profile: String,
    labels: Vec<String>,
    lines: Vec<String>,
}

struct ArenaJob {
    seed: u32,
    side: ArenaSide,
}

pub fn run_from_env() {
    let Some(config) = parse_args_or_exit() else {
        return;
    };

    match run_arena(&config) {
        Ok(report) => {
            println!(
                "AI arena: {} vs {}  runs={}  candidate_wins={}  baseline_wins={}  draws={}",
                report.candidate,
                report.baseline,
                report.aggregate.runs,
                report.aggregate.candidate_wins,
                report.aggregate.baseline_wins,
                report.aggregate.unresolved_draws
            );
            println!("artifacts: {}", config.out_dir.display());
        }
        Err(err) => {
            eprintln!("ai-arena failed: {err}");
            process::exit(1);
        }
    }
}

fn parse_args_or_exit() -> Option<CliConfig> {
    match parse_args(std::env::args().skip(1)) {
        Ok(config) => config,
        Err(err) => {
            eprintln!("{err}");
            eprintln!();
            print_usage();
            process::exit(2);
        }
    }
}

fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Option<CliConfig>, String> {
    let mut candidate = DEFAULT_CANDIDATE.to_string();
    let mut baseline = DEFAULT_BASELINE.to_string();
    let mut seeds = DEFAULT_SEEDS;
    let mut seed_start = 0;
    let mut ticks = DEFAULT_TICKS;
    let mut out_dir = default_out_dir();
    let mut verify_replay = true;

    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                print_usage();
                return Ok(None);
            }
            "--candidate" => {
                candidate = resolve_profile_arg(&required_value(&arg, &mut args)?)?;
            }
            "--baseline" => {
                baseline = resolve_profile_arg(&required_value(&arg, &mut args)?)?;
            }
            "--seeds" => {
                seeds = parse_u32_flag(&arg, &mut args)?;
            }
            "--seed-start" => {
                seed_start = parse_u32_flag(&arg, &mut args)?;
            }
            "--ticks" => {
                ticks = parse_u32_flag(&arg, &mut args)?;
            }
            "--out-dir" => {
                out_dir = PathBuf::from(required_value(&arg, &mut args)?);
            }
            "--no-verify-replay" => {
                verify_replay = false;
            }
            _ => return Err(format!("unknown flag: {arg}")),
        }
    }

    if candidate == baseline {
        return Err("candidate and baseline must be different profiles".to_string());
    }
    if seeds == 0 {
        return Err("--seeds must be greater than zero".to_string());
    }
    if ticks == 0 {
        return Err("--ticks must be greater than zero".to_string());
    }

    Ok(Some(CliConfig {
        candidate,
        baseline,
        seeds,
        seed_start,
        ticks,
        out_dir,
        verify_replay,
    }))
}

fn run_arena(config: &CliConfig) -> Result<ArenaReport, String> {
    std::fs::create_dir_all(&config.out_dir).map_err(|err| err.to_string())?;
    let replay_dir = config.out_dir.join("runs");
    std::fs::create_dir_all(&replay_dir).map_err(|err| err.to_string())?;

    let jobs = side_swapped_jobs(config.seed_start, config.seeds);
    if config.candidate == config.baseline {
        return Err("candidate and baseline must be different profiles".to_string());
    }
    let mut runs = Vec::with_capacity(jobs.len());
    for job in jobs {
        let candidate_profile = config.candidate.as_str();
        let baseline_profile = config.baseline.as_str();
        let (profile_a, profile_b) = match job.side {
            ArenaSide::CandidatePlayerOne => {
                (candidate_profile.to_string(), baseline_profile.to_string())
            }
            ArenaSide::CandidatePlayerTwo => {
                (baseline_profile.to_string(), candidate_profile.to_string())
            }
        };
        let replay_name =
            run_artifact_name(candidate_profile, baseline_profile, job.seed, job.side);
        let result = run_profile_matchup_result(ProfileMatchupOptions {
            profile_a,
            profile_b,
            seed: job.seed,
            max_ticks: config.ticks,
            verify_replay: config.verify_replay,
            save_replay_name: Some(replay_name),
            replay_dir: Some(replay_dir.clone()),
        })?;
        let artifact_dir = result
            .replay_artifact
            .as_ref()
            .map(PathBuf::from)
            .ok_or_else(|| "arena run did not save a replay artifact".to_string())?;
        let run = write_run_sidecars(
            &artifact_dir,
            candidate_profile,
            baseline_profile,
            job.side,
            &result,
        )?;
        runs.push(run);
    }

    let aggregate = aggregate_runs(&runs);
    let report = ArenaReport {
        schema: ARENA_SCHEMA_VERSION,
        tool: "ai-arena",
        candidate: config.candidate.clone(),
        baseline: config.baseline.clone(),
        seed_start: config.seed_start,
        seeds: config.seeds,
        max_ticks: config.ticks,
        runs,
        aggregate,
    };
    write_json(config.out_dir.join("arena-summary.json"), &report)?;
    Ok(report)
}

fn write_run_sidecars(
    artifact_dir: &Path,
    candidate_profile: &str,
    baseline_profile: &str,
    side: ArenaSide,
    result: &ProfileMatchupResult,
) -> Result<ArenaRunSummary, String> {
    let candidate_player_id = match side {
        ArenaSide::CandidatePlayerOne => 1,
        ArenaSide::CandidatePlayerTwo => 2,
    };
    let baseline_player_id = if candidate_player_id == 1 { 2 } else { 1 };
    let candidate_identity = profile_identity_by_id(candidate_profile)
        .ok_or_else(|| format!("unknown candidate profile {candidate_profile}"))?;
    let baseline_identity = profile_identity_by_id(baseline_profile)
        .ok_or_else(|| format!("unknown baseline profile {baseline_profile}"))?;
    validate_profile_identity(&candidate_identity)?;
    validate_profile_identity(&baseline_identity)?;

    let mut profiles = BTreeMap::new();
    profiles.insert(candidate_profile.to_string(), candidate_identity);
    profiles.insert(baseline_profile.to_string(), baseline_identity);
    let manifest = ArenaRunManifest {
        schema: ARENA_SCHEMA_VERSION,
        tool: "ai-arena",
        server_build_sha: server_build_sha().to_string(),
        seed: result.seed,
        max_ticks: result.max_ticks,
        side,
        candidate_profile_id: candidate_profile.to_string(),
        baseline_profile_id: baseline_profile.to_string(),
        profiles,
        replay_artifact: result.replay_artifact.clone(),
    };
    let outcome = outcome_for(result, candidate_player_id, baseline_player_id);
    let run = ArenaRunSummary {
        manifest,
        result: result.clone(),
        candidate_player_id,
        baseline_player_id,
        outcome,
        artifact_dir: artifact_dir.display().to_string(),
    };
    write_json(artifact_dir.join("manifest.json"), &run.manifest)?;
    write_json(artifact_dir.join("summary.json"), &run)?;
    write_trace_jsonl(
        artifact_dir.join("decision-trace.jsonl"),
        &result.ai_trace_tail,
    )?;
    std::fs::write(artifact_dir.join("brief.md"), brief_markdown(&run))
        .map_err(|err| err.to_string())?;
    Ok(run)
}

fn outcome_for(
    result: &ProfileMatchupResult,
    candidate_player_id: u32,
    baseline_player_id: u32,
) -> ArenaOutcome {
    if let Some(winner) = &result.winner {
        return ArenaOutcome {
            winner_profile: Some(winner.profile.clone()),
            candidate_won: winner.player_id == candidate_player_id,
            baseline_won: winner.player_id == baseline_player_id,
            tick_cap: false,
            end_reason: result.end_reason,
        };
    }

    ArenaOutcome {
        winner_profile: None,
        candidate_won: false,
        baseline_won: false,
        tick_cap: result.end_reason == ProfileMatchupEndReason::TickCap,
        end_reason: result.end_reason,
    }
}

fn aggregate_runs(runs: &[ArenaRunSummary]) -> ArenaAggregate {
    let mut aggregate = ArenaAggregate {
        runs: runs.len() as u32,
        ..ArenaAggregate::default()
    };
    for run in runs {
        if run.outcome.candidate_won {
            aggregate.candidate_wins = aggregate.candidate_wins.saturating_add(1);
        } else if run.outcome.baseline_won {
            aggregate.baseline_wins = aggregate.baseline_wins.saturating_add(1);
        } else {
            aggregate.unresolved_draws = aggregate.unresolved_draws.saturating_add(1);
        }
        if run.outcome.candidate_won || run.outcome.baseline_won {
            aggregate.starting_city_centre_wins =
                aggregate.starting_city_centre_wins.saturating_add(1);
        }
    }
    aggregate
}

fn write_trace_jsonl(path: PathBuf, entries: &[ProfileMatchupTraceEntry]) -> Result<(), String> {
    let mut text = String::new();
    for entry in entries {
        let record = DecisionTraceRecord {
            tick: entry.tick,
            player_id: entry.player_id,
            profile: entry.profile.clone(),
            labels: trace_labels(&entry.lines),
            lines: entry.lines.clone(),
        };
        text.push_str(&serde_json::to_string(&record).map_err(|err| err.to_string())?);
        text.push('\n');
    }
    std::fs::write(path, text).map_err(|err| err.to_string())
}

fn trace_labels(lines: &[String]) -> Vec<String> {
    let mut labels = BTreeSet::new();
    for line in lines {
        if let Some(command) = line.strip_prefix("command=") {
            labels.insert(format!("command:{command}"));
            continue;
        }
        for token in line.split_whitespace() {
            if let Some(goal) = token.strip_prefix("goal=") {
                labels.insert(format!("goal:{goal}"));
            } else if let Some(status) = token.strip_prefix("status=") {
                labels.insert(format!("status:{status}"));
            } else if let Some(blockers) = token.strip_prefix("blockers=") {
                for blocker in blockers.split(',').filter(|value| *value != "-") {
                    labels.insert(format!("blocker:{blocker}"));
                }
            } else if let Some(intents) = token.strip_prefix("intents=") {
                for intent in intents.split(',').filter(|value| *value != "-") {
                    labels.insert(format!("intent:{intent}"));
                }
            }
        }
    }
    labels.into_iter().collect()
}

fn brief_markdown(run: &ArenaRunSummary) -> String {
    let result = &run.result;
    let candidate = &run.manifest.candidate_profile_id;
    let baseline = &run.manifest.baseline_profile_id;
    let candidate_player = result
        .players
        .iter()
        .find(|player| player.player_id == run.candidate_player_id);
    let baseline_player = result
        .players
        .iter()
        .find(|player| player.player_id == run.baseline_player_id);
    let mut text = String::new();
    text.push_str("# AI Arena Brief\n\n");
    text.push_str(&format!(
        "- Matchup: `{candidate}` as player {} vs `{baseline}` as player {}\n",
        run.candidate_player_id, run.baseline_player_id
    ));
    text.push_str(&format!(
        "- Seed: {}  Tick cap: {}  End tick: {}\n",
        result.seed, result.max_ticks, result.ticks
    ));
    text.push_str(&format!(
        "- Result: {}\n",
        result_text(&run.outcome, candidate, baseline)
    ));
    text.push_str(&format!(
        "- Starting City Centres: {}\n",
        starting_city_centre_text(&result.starting_city_centres)
    ));
    text.push_str(&format!(
        "- Replay: {}\n\n",
        result.replay_artifact.as_deref().unwrap_or("not saved")
    ));
    text.push_str("## Profiles\n\n");
    for identity in run.manifest.profiles.values() {
        text.push_str(&format!(
            "- `{}` `{}` modules={} overlays={} summary={}\n",
            identity.profile_id,
            identity.fingerprint,
            identity.modules.join(","),
            identity
                .overlays
                .iter()
                .map(|overlay| overlay.id.as_str())
                .collect::<Vec<_>>()
                .join(","),
            identity.summary
        ));
    }
    text.push_str("\n## Timeline\n\n");
    text.push_str(&format!(
        "- First damage: {}\n",
        tick_text(result.first_damage_tick)
    ));
    if let Some(player) = candidate_player {
        text.push_str(&player_timeline("Candidate", player));
    }
    if let Some(player) = baseline_player {
        text.push_str(&player_timeline("Baseline", player));
    }
    text.push_str(&format!(
        "- Combat events: attacks={} deaths={} totalEvents={}\n\n",
        result.attack_events, result.death_events, result.event_count
    ));
    text.push_str("## Investigation Index\n\n");
    text.push_str("- Search `decision-trace.jsonl` by `goal:*`, `status:*`, `blocker:*`, `intent:*`, and `command:*` labels.\n");
    text.push_str(&format!(
        "- Trace records saved: {} recent decisions from the bounded matchup trace tail.\n",
        result.ai_trace_tail.len()
    ));
    if let Some(tick) = result.first_damage_tick {
        text.push_str(&format!(
            "- Start combat inspection around tick {} and compare nearby attack commands.\n",
            tick.saturating_sub(180)
        ));
    }
    text
}

fn result_text(outcome: &ArenaOutcome, candidate: &str, baseline: &str) -> String {
    if outcome.candidate_won {
        return format!("candidate `{candidate}` killed the baseline starting City Centre");
    }
    if outcome.baseline_won {
        return format!("baseline `{baseline}` killed the candidate starting City Centre");
    }
    if outcome.tick_cap {
        return "draw at tick cap".to_string();
    }
    if outcome.end_reason == ProfileMatchupEndReason::StartingCityCentresDestroyed {
        return "draw: both starting City Centres were destroyed".to_string();
    }
    "draw without a starting City Centre winner".to_string()
}

fn starting_city_centre_text(
    centres: &[crate::selfplay::ProfileMatchupStartingCityCentreResult],
) -> String {
    if centres.is_empty() {
        return "-".to_string();
    }
    centres
        .iter()
        .map(|centre| {
            format!(
                "player {} `{}` id={} death={}",
                centre.player_id,
                centre.profile,
                centre.entity_id,
                tick_text(centre.death_tick)
            )
        })
        .collect::<Vec<_>>()
        .join("; ")
}

fn player_timeline(label: &str, player: &crate::selfplay::ProfileMatchupPlayerResult) -> String {
    format!(
        "- {label} player {} `{}`: firstAttack={} firstRifleAttack={} expansion={}/{} firstTank={} workers={} army={} buildings={} damage={} losses={} finalCounts={}\n",
        player.player_id,
        player.profile,
        tick_text(player.first_attack_command_tick),
        tick_text(player.first_rifleman_attack_command_tick),
        tick_text(player.first_expansion_city_centre_planned_tick),
        tick_text(player.first_expansion_city_centre_completed_tick),
        tick_text(player.first_tank_tick),
        player.worker_count,
        player.army_value,
        player.building_value,
        player.damage_dealt_events,
        player.death_count,
        format_counts(&player.final_counts),
    )
}

fn tick_text(tick: Option<u32>) -> String {
    tick.map(|tick| tick.to_string())
        .unwrap_or_else(|| "-".to_string())
}

fn format_counts(counts: &BTreeMap<String, u32>) -> String {
    if counts.is_empty() {
        return "-".to_string();
    }
    counts
        .iter()
        .map(|(kind, count)| format!("{kind}={count}"))
        .collect::<Vec<_>>()
        .join(",")
}

fn side_swapped_jobs(seed_start: u32, seeds: u32) -> Vec<ArenaJob> {
    let mut jobs = Vec::new();
    for offset in 0..seeds {
        let seed = seed_start.saturating_add(offset);
        jobs.push(ArenaJob {
            seed,
            side: ArenaSide::CandidatePlayerOne,
        });
        jobs.push(ArenaJob {
            seed,
            side: ArenaSide::CandidatePlayerTwo,
        });
    }
    jobs
}

fn run_artifact_name(
    candidate_profile: &str,
    baseline_profile: &str,
    seed: u32,
    side: ArenaSide,
) -> String {
    let side = match side {
        ArenaSide::CandidatePlayerOne => "candidate_p1",
        ArenaSide::CandidatePlayerTwo => "candidate_p2",
    };
    format!("arena__{candidate_profile}__vs__{baseline_profile}__seed_{seed}__{side}")
}

fn write_json(path: PathBuf, value: &impl Serialize) -> Result<(), String> {
    let json = serde_json::to_vec_pretty(value).map_err(|err| err.to_string())?;
    std::fs::write(path, json).map_err(|err| err.to_string())
}

fn resolve_profile_arg(value: &str) -> Result<String, String> {
    canonical_profile_id(value)
        .map(str::to_string)
        .ok_or_else(|| {
            format!(
                "unknown profile {value:?}; known profile ids: {}",
                available_profile_ids().join(", ")
            )
        })
}

fn required_value(flag: &str, args: &mut impl Iterator<Item = String>) -> Result<String, String> {
    args.next()
        .ok_or_else(|| format!("{flag} requires a value"))
}

fn parse_u32_flag(flag: &str, args: &mut impl Iterator<Item = String>) -> Result<u32, String> {
    let value = required_value(flag, args)?;
    value
        .parse()
        .map_err(|_| format!("{flag} requires a u32 value, got {value:?}"))
}

fn default_out_dir() -> PathBuf {
    std::env::temp_dir().join(format!("rts-ai-arena-{}", process::id()))
}

fn print_usage() {
    println!(
        "Usage:
  ai-arena [options]

Options:
  --candidate <id>       Candidate profile id (default: {DEFAULT_CANDIDATE})
  --baseline <id>        Baseline profile id (default: {DEFAULT_BASELINE})
  --seeds <u32>          Number of seeds to run, side-swapped (default: {DEFAULT_SEEDS})
  --seed-start <u32>     First seed to run (default: 0)
  --ticks <u32>          Tick cap per run (default: {DEFAULT_TICKS})
  --out-dir <path>       Artifact directory (default: /tmp/rts-ai-arena-<pid>)
  --no-verify-replay     Skip deterministic command-log replay verification
  -h, --help             Print this help
"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arena_jobs_are_side_swapped_per_seed() {
        let jobs = side_swapped_jobs(7, 2);

        assert_eq!(jobs.len(), 4);
        assert_eq!(jobs[0].seed, 7);
        assert_eq!(jobs[0].side, ArenaSide::CandidatePlayerOne);
        assert_eq!(jobs[1].seed, 7);
        assert_eq!(jobs[1].side, ArenaSide::CandidatePlayerTwo);
        assert_eq!(jobs[2].seed, 8);
    }

    #[test]
    fn trace_labels_are_searchable() {
        let labels = trace_labels(&[
            "goal=Economy status=Selected blockers=- intents=Train:Worker,Gather:Steel".to_string(),
            "goal=FrontalAttack status=Skipped blockers=WaitingForUnits,AttackCadence intents=-"
                .to_string(),
            "command=Train:Rifleman".to_string(),
        ]);

        assert!(labels.contains(&"goal:Economy".to_string()));
        assert!(labels.contains(&"intent:Gather:Steel".to_string()));
        assert!(labels.contains(&"blocker:WaitingForUnits".to_string()));
        assert!(labels.contains(&"command:Train:Rifleman".to_string()));
    }

    #[test]
    fn default_arena_parse_uses_canonical_profiles() {
        let config = parse_args(Vec::<String>::new())
            .expect("default args should parse")
            .expect("default args should produce config");

        assert_eq!(config.candidate, DEFAULT_CANDIDATE);
        assert_eq!(config.baseline, DEFAULT_BASELINE);
        assert_eq!(config.seeds, DEFAULT_SEEDS);
    }

    #[test]
    fn arena_rejects_the_same_profile_on_both_sides() {
        let out_dir =
            std::env::temp_dir().join(format!("rts-ai-arena-same-profile-test-{}", process::id()));
        let config = CliConfig {
            candidate: "ai_2_1".to_string(),
            baseline: "ai_2_1".to_string(),
            seeds: 1,
            seed_start: 0,
            ticks: 1,
            out_dir: out_dir.clone(),
            verify_replay: false,
        };

        let err = run_arena(&config).expect_err("same canonical profile should be rejected");

        assert!(err.contains("candidate and baseline must be different profiles"));
        let _ = std::fs::remove_dir_all(out_dir);
    }

    #[test]
    fn tick_cap_draw_is_not_scored_by_army_value() {
        let result = profile_result(
            crate::selfplay::ProfileMatchupEndReason::TickCap,
            None,
            200,
            100,
        );

        let outcome = outcome_for(&result, 1, 2);

        assert!(outcome.tick_cap);
        assert!(!outcome.candidate_won);
        assert!(!outcome.baseline_won);
        assert_eq!(
            result_text(&outcome, DEFAULT_CANDIDATE, DEFAULT_BASELINE),
            "draw at tick cap"
        );
    }

    #[test]
    fn starting_city_centre_winner_is_not_overridden_by_army_value() {
        let result = profile_result(
            crate::selfplay::ProfileMatchupEndReason::StartingCityCentreKilled,
            Some(2),
            300,
            50,
        );

        let outcome = outcome_for(&result, 1, 2);

        assert!(!outcome.tick_cap);
        assert!(!outcome.candidate_won);
        assert!(outcome.baseline_won);
        assert_eq!(
            result_text(&outcome, DEFAULT_CANDIDATE, DEFAULT_BASELINE),
            format!("baseline `{DEFAULT_BASELINE}` killed the candidate starting City Centre")
        );
    }

    fn profile_result(
        end_reason: crate::selfplay::ProfileMatchupEndReason,
        winner_player_id: Option<u32>,
        player_one_army: u32,
        player_two_army: u32,
    ) -> ProfileMatchupResult {
        ProfileMatchupResult {
            profile_a: DEFAULT_CANDIDATE.to_string(),
            profile_b: DEFAULT_BASELINE.to_string(),
            seed: 0,
            max_ticks: 120,
            ticks: 120,
            end_reason,
            winner: winner_player_id.map(|player_id| crate::selfplay::ProfileMatchupWinner {
                player_id,
                profile: profile_for_player(player_id).to_string(),
            }),
            starting_city_centres: vec![
                starting_city_centre(1, (winner_player_id == Some(2)).then_some(80)),
                starting_city_centre(2, (winner_player_id == Some(1)).then_some(80)),
            ],
            players: vec![
                player_result(1, player_one_army),
                player_result(2, player_two_army),
            ],
            first_damage_tick: None,
            attack_events: 0,
            death_events: 0,
            event_count: 0,
            replay_verified: false,
            replay_artifact: None,
            ai_trace_tail: Vec::new(),
        }
    }

    fn player_result(
        player_id: u32,
        army_value: u32,
    ) -> crate::selfplay::ProfileMatchupPlayerResult {
        crate::selfplay::ProfileMatchupPlayerResult {
            player_id,
            profile: profile_for_player(player_id).to_string(),
            alive: true,
            army_value,
            building_value: 0,
            worker_count: 0,
            command_count: 0,
            attack_command_count: 0,
            damage_dealt_events: 0,
            death_count: 0,
            first_attack_command_tick: None,
            first_rifleman_attack_command_tick: None,
            first_scout_car_tick: None,
            first_scout_car_harass_command_tick: None,
            first_expansion_city_centre_planned_tick: None,
            first_expansion_city_centre_completed_tick: None,
            first_tank_tick: None,
            final_counts: BTreeMap::new(),
        }
    }

    fn starting_city_centre(
        player_id: u32,
        death_tick: Option<u32>,
    ) -> crate::selfplay::ProfileMatchupStartingCityCentreResult {
        crate::selfplay::ProfileMatchupStartingCityCentreResult {
            player_id,
            profile: profile_for_player(player_id).to_string(),
            entity_id: player_id * 100,
            alive: death_tick.is_none(),
            death_tick,
        }
    }

    fn profile_for_player(player_id: u32) -> &'static str {
        if player_id == 1 {
            DEFAULT_CANDIDATE
        } else {
            DEFAULT_BASELINE
        }
    }

    #[test]
    fn tiny_arena_run_writes_agent_artifacts() {
        let out_dir = std::env::temp_dir().join(format!(
            "rts-ai-arena-test-{}-{}",
            process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let config = CliConfig {
            candidate: DEFAULT_CANDIDATE.to_string(),
            baseline: DEFAULT_BASELINE.to_string(),
            seeds: 1,
            seed_start: 3,
            ticks: 12,
            out_dir: out_dir.clone(),
            verify_replay: false,
        };

        let report = run_arena(&config).expect("tiny arena should run");

        assert_eq!(report.runs.len(), 2);
        assert!(out_dir.join("arena-summary.json").exists());
        for run in &report.runs {
            let dir = PathBuf::from(&run.artifact_dir);
            assert!(dir.join("replay.json").exists());
            assert!(dir.join("manifest.json").exists());
            assert!(dir.join("summary.json").exists());
            assert!(dir.join("decision-trace.jsonl").exists());
            assert!(dir.join("brief.md").exists());
        }
        let _ = std::fs::remove_dir_all(out_dir);
    }
}
